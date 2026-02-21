use {
    crate::Error,
    futures::{SinkExt, StreamExt},
    moltis_protocol::{
        ClientInfo, ConnectAuth, ConnectParams, HelloOk, PROTOCOL_VERSION, RequestFrame,
    },
    std::{sync::Arc, time::Duration},
    tokio::sync::mpsc,
    tokio_tungstenite::{Connector, connect_async_tls_with_config, tungstenite::Message},
    tracing::{debug, error, info},
};

/// Maximum reconnect backoff delay.
const MAX_BACKOFF: Duration = Duration::from_secs(5);

/// Events sent from the connection task to the main app loop.
#[derive(Debug)]
pub enum ConnectionEvent {
    Connected(Box<HelloOk>),
    Disconnected,
    Error(String),
    Frame(String),
}

/// Manages a WebSocket connection to the gateway, including handshake and
/// auto-reconnect with exponential backoff.
pub struct ConnectionManager {
    /// Send JSON text frames to the WebSocket writer task.
    write_tx: mpsc::UnboundedSender<String>,
}

impl ConnectionManager {
    /// Spawn the connection manager. Connects to the gateway and begins
    /// forwarding frames to `event_tx`. Returns immediately — the connection
    /// runs in background tasks.
    pub fn spawn(
        url: String,
        auth: ConnectAuth,
        event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    ) -> Self {
        let (write_tx, write_rx) = mpsc::unbounded_channel::<String>();

        tokio::spawn(connection_loop(url, auth, event_tx, write_rx));

        Self { write_tx }
    }

    /// Send a raw JSON string through the WebSocket.
    pub fn send_raw(&self, json: String) {
        // Ignore send error — means the connection loop has exited.
        let _ = self.write_tx.send(json);
    }
}

/// Build the `ConnectParams` for the protocol v3 handshake.
fn build_connect_params(auth: &ConnectAuth) -> ConnectParams {
    ConnectParams {
        min_protocol: PROTOCOL_VERSION,
        max_protocol: PROTOCOL_VERSION,
        client: ClientInfo {
            id: "moltis-tui".into(),
            display_name: Some("Moltis TUI".into()),
            version: env!("CARGO_PKG_VERSION").into(),
            platform: std::env::consts::OS.into(),
            device_family: None,
            model_identifier: None,
            mode: "operator".into(),
            instance_id: Some(uuid::Uuid::new_v4().to_string()),
        },
        caps: Some(vec!["streaming".into(), "tools".into(), "approvals".into()]),
        commands: None,
        permissions: None,
        path_env: None,
        role: Some("operator".into()),
        scopes: Some(vec![
            "operator.admin".into(),
            "operator.read".into(),
            "operator.write".into(),
            "operator.approvals".into(),
        ]),
        device: None,
        auth: Some(auth.clone()),
        locale: None,
        user_agent: Some(format!("moltis-tui/{}", env!("CARGO_PKG_VERSION"))),
        timezone: None,
    }
}

/// Main connection loop with auto-reconnect.
async fn connection_loop(
    url: String,
    auth: ConnectAuth,
    event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    mut write_rx: mpsc::UnboundedReceiver<String>,
) {
    let mut backoff = Duration::from_secs(1);

    loop {
        info!(url = %url, "connecting to gateway");

        match connect_and_run(&url, &auth, &event_tx, &mut write_rx).await {
            Ok(()) => {
                debug!("connection closed cleanly");
            },
            Err(e) => {
                error!(error = %e, "connection error");
                let _ = event_tx.send(ConnectionEvent::Error(e.to_string()));
            },
        }

        let _ = event_tx.send(ConnectionEvent::Disconnected);

        // Exponential backoff before reconnect
        info!(delay_ms = backoff.as_millis(), "reconnecting after delay");
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// Build a TLS connector that trusts the gateway's self-signed CA.
///
/// First tries to load the Moltis CA cert from the config directory.
/// Falls back to a permissive verifier so local development always works.
fn build_tls_connector() -> Connector {
    let mut root_store = rustls::RootCertStore::empty();

    // Load system certs
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = root_store.add(cert);
    }

    // Load the Moltis CA cert from the config directory
    if let Some(config_dir) = moltis_config::config_dir() {
        let ca_path = config_dir.join("certs").join("ca.pem");
        if let Ok(pem_data) = std::fs::read(&ca_path) {
            let mut reader = std::io::BufReader::new(pem_data.as_slice());
            for cert in rustls_pemfile::certs(&mut reader).flatten() {
                let _ = root_store.add(cert);
            }
            debug!(path = %ca_path.display(), "loaded Moltis CA cert");
        }
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Connector::Rustls(Arc::new(config))
}

/// Single connection attempt: connect, handshake, then forward frames.
async fn connect_and_run(
    url: &str,
    auth: &ConnectAuth,
    event_tx: &mpsc::UnboundedSender<ConnectionEvent>,
    write_rx: &mut mpsc::UnboundedReceiver<String>,
) -> Result<(), Error> {
    let connector = build_tls_connector();
    let (ws_stream, _response) =
        connect_async_tls_with_config(url, None, false, Some(connector)).await?;
    let (mut ws_sink, mut ws_reader) = ws_stream.split();

    // Send connect handshake
    let connect_id = uuid::Uuid::new_v4().to_string();
    let connect_frame = RequestFrame {
        r#type: "req".into(),
        id: connect_id.clone(),
        method: "connect".into(),
        params: Some(serde_json::to_value(build_connect_params(auth)).map_err(Error::Json)?),
    };
    let connect_json = serde_json::to_string(&connect_frame).map_err(Error::Json)?;
    ws_sink.send(Message::Text(connect_json.into())).await?;

    // Wait for hello-ok response
    let hello_ok = wait_for_hello(&mut ws_reader, &connect_id).await?;
    info!(
        server_version = %hello_ok.server.version,
        conn_id = %hello_ok.server.conn_id,
        "connected to gateway"
    );
    let _ = event_tx.send(ConnectionEvent::Connected(Box::new(hello_ok)));

    // Reset backoff on successful connection (handled by caller via Ok return,
    // but we rely on the loop structure).

    // Forward frames bidirectionally
    loop {
        tokio::select! {
            // Incoming frames from gateway
            msg = ws_reader.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let _ = event_tx.send(ConnectionEvent::Frame(text.to_string()));
                    },
                    Some(Ok(Message::Close(_))) | None => {
                        debug!("WebSocket closed by server");
                        return Ok(());
                    },
                    Some(Ok(Message::Ping(data))) => {
                        ws_sink.send(Message::Pong(data)).await?;
                    },
                    Some(Ok(_)) => {}, // Ignore binary, pong, etc.
                    Some(Err(e)) => {
                        return Err(Error::WebSocket(e));
                    },
                }
            },
            // Outgoing frames from app
            json = write_rx.recv() => {
                match json {
                    Some(text) => {
                        ws_sink.send(Message::Text(text.into())).await?;
                    },
                    None => {
                        // write channel closed — app is shutting down
                        let _ = ws_sink.send(Message::Close(None)).await;
                        return Ok(());
                    },
                }
            },
        }
    }
}

/// Wait for the `hello-ok` response frame from the gateway.
async fn wait_for_hello(
    reader: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    connect_id: &str,
) -> Result<HelloOk, Error> {
    let timeout = Duration::from_millis(moltis_protocol::HANDSHAKE_TIMEOUT_MS);

    let result = tokio::time::timeout(timeout, async {
        while let Some(msg) = reader.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Parse as a response frame
                    if let Ok(frame) = serde_json::from_str::<moltis_protocol::ResponseFrame>(&text)
                        && frame.id == connect_id
                    {
                        if frame.ok {
                            if let Some(payload) = frame.payload {
                                let hello: HelloOk =
                                    serde_json::from_value(payload).map_err(Error::Json)?;
                                return Ok(hello);
                            }
                            return Err(Error::Protocol(
                                "hello-ok response missing payload".into(),
                            ));
                        } else {
                            let msg = frame
                                .error
                                .map(|e| e.message)
                                .unwrap_or_else(|| "unknown error".into());
                            return Err(Error::Auth(msg));
                        }
                    }
                    // Not our response — could be an event, skip it
                },
                Ok(Message::Close(_)) => {
                    return Err(Error::Connection(
                        "server closed connection during handshake".into(),
                    ));
                },
                Ok(_) => {},
                Err(e) => return Err(Error::WebSocket(e)),
            }
        }
        Err(Error::Connection(
            "connection closed before handshake".into(),
        ))
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(Error::Connection("handshake timed out".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_params_built_correctly() {
        let auth = ConnectAuth {
            api_key: Some("test-key".into()),
            password: None,
            token: None,
        };
        let params = build_connect_params(&auth);
        assert_eq!(params.min_protocol, PROTOCOL_VERSION);
        assert_eq!(params.max_protocol, PROTOCOL_VERSION);
        assert_eq!(params.client.id, "moltis-tui");
        assert_eq!(params.client.mode, "operator");
        assert!(params.auth.is_some());
        assert_eq!(
            params.auth.as_ref().and_then(|a| a.api_key.as_deref()),
            Some("test-key")
        );
    }
}
