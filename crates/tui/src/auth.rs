use moltis_protocol::ConnectAuth;

/// Resolve authentication credentials from CLI arguments.
///
/// Priority: explicit API key > environment variable > no auth (local instances).
pub fn resolve_auth(api_key: Option<&str>) -> ConnectAuth {
    ConnectAuth {
        api_key: api_key.map(String::from),
        password: None,
        token: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_api_key() {
        let auth = resolve_auth(Some("sk-test-123"));
        assert_eq!(auth.api_key.as_deref(), Some("sk-test-123"));
        assert!(auth.password.is_none());
    }

    #[test]
    fn without_credentials() {
        let auth = resolve_auth(None);
        assert!(auth.api_key.is_none());
        assert!(auth.password.is_none());
        assert!(auth.token.is_none());
    }
}
