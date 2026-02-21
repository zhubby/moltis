mod app;
mod auth;
mod connection;
pub mod error;
mod events;
mod onboarding;
mod rpc;
mod state;
mod ui;

pub use {app::App, error::Error};

/// Build the default gateway WebSocket URL from `moltis.toml` config.
fn resolve_gateway_url() -> String {
    let config = moltis_config::discover_and_load();
    let scheme = if config.tls.enabled {
        "wss"
    } else {
        "ws"
    };
    let bind = &config.server.bind;
    let port = config.server.port;

    // Use `localhost` for loopback binds to avoid TLS/SNI warnings when an IP
    // literal is used as the hostname.
    let host = if matches!(bind.as_str(), "0.0.0.0" | "::" | "127.0.0.1" | "::1") {
        "localhost"
    } else {
        bind
    };

    format!("{scheme}://{host}:{port}/ws/chat")
}

/// Entry point for the TUI client.
///
/// When `url` is `None`, the gateway address is derived from the local
/// `moltis.toml` config (server bind/port + TLS setting).
pub async fn run_tui(url: Option<&str>, api_key: Option<&str>) -> Result<(), Error> {
    // Install the rustls ring crypto provider for TLS connections.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let url = match url {
        Some(u) => u.to_owned(),
        None => resolve_gateway_url(),
    };

    let connect_auth = auth::resolve_auth(api_key);

    // Enable focus-change reporting so we can redraw on tab-switch.
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableFocusChange)
        .map_err(Error::Terminal)?;

    let terminal = ratatui::init();
    let result = App::new(url, connect_auth).run(terminal).await;
    ratatui::restore();

    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableFocusChange);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_initial_state() {
        let auth = moltis_protocol::ConnectAuth {
            api_key: None,
            password: None,
            token: None,
        };
        let app = App::new("ws://localhost:9433/ws/chat".into(), auth);
        drop(app);
    }

    #[test]
    fn resolve_url_from_config() {
        let url = resolve_gateway_url();
        assert!(
            url.starts_with("ws://") || url.starts_with("wss://"),
            "URL must start with ws:// or wss://, got: {url}"
        );
        assert!(
            url.ends_with("/ws/chat"),
            "URL must end with /ws/chat, got: {url}"
        );
    }
}
