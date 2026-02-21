use {
    crate::{Error, connection::ConnectionManager},
    moltis_protocol::{RequestFrame, ResponseFrame},
    std::{collections::HashMap, sync::Arc, time::Duration},
    tokio::sync::{Mutex, oneshot},
};

/// Timeout for individual RPC calls.
const RPC_TIMEOUT: Duration = Duration::from_secs(10);

/// Correlates RPC request/response pairs by ID.
pub struct RpcClient {
    connection: Arc<ConnectionManager>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<ResponseFrame>>>>,
}

impl RpcClient {
    pub fn new(connection: Arc<ConnectionManager>) -> Self {
        Self {
            connection,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Send an RPC request and wait for the matching response.
    pub async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let frame = RequestFrame {
            r#type: "req".into(),
            id: id.clone(),
            method: method.into(),
            params: Some(params),
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id.clone(), tx);
        }

        let json = serde_json::to_string(&frame).map_err(Error::Json)?;
        self.connection.send_raw(json);

        // Wait for response with timeout
        let result = tokio::time::timeout(RPC_TIMEOUT, rx).await;

        // Clean up pending entry on timeout or error
        match result {
            Ok(Ok(response)) => {
                if response.ok {
                    Ok(response.payload.unwrap_or(serde_json::Value::Null))
                } else {
                    let msg = response
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "unknown RPC error".into());
                    Err(Error::Protocol(msg))
                }
            },
            Ok(Err(_)) => {
                // oneshot sender dropped — connection closed
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                Err(Error::Connection(
                    "connection closed during RPC call".into(),
                ))
            },
            Err(_) => {
                // Timeout
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                Err(Error::Connection(format!(
                    "RPC call '{method}' timed out after {}s",
                    RPC_TIMEOUT.as_secs()
                )))
            },
        }
    }

    /// Send an RPC request without waiting for a response.
    pub fn fire_and_forget(&self, method: &str, params: serde_json::Value) {
        let id = uuid::Uuid::new_v4().to_string();
        let frame = RequestFrame {
            r#type: "req".into(),
            id,
            method: method.into(),
            params: Some(params),
        };

        if let Ok(json) = serde_json::to_string(&frame) {
            self.connection.send_raw(json);
        }
    }

    /// Called by the event loop when a response frame arrives.
    /// Routes it to the waiting `call()` if one exists.
    pub async fn resolve_response(&self, frame: ResponseFrame) {
        let mut pending = self.pending.lock().await;
        if let Some(tx) = pending.remove(&frame.id) {
            // Ignore send error — the caller may have timed out and dropped the receiver.
            let _ = tx.send(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolve_response_routes_to_caller() {
        let (event_tx, _event_rx) =
            tokio::sync::mpsc::unbounded_channel::<crate::connection::ConnectionEvent>();
        let conn = Arc::new(ConnectionManager::spawn(
            // Use a dummy URL — we won't actually connect in this test.
            // The connection will fail, but we only test the pending map.
            "ws://127.0.0.1:1/ws/chat".into(),
            moltis_protocol::ConnectAuth {
                api_key: None,
                password: None,
                token: None,
            },
            event_tx,
        ));

        let rpc = RpcClient::new(conn);

        // Manually insert a pending entry
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = rpc.pending.lock().await;
            pending.insert("test-id".into(), tx);
        }

        // Resolve it
        let response = ResponseFrame {
            r#type: "res".into(),
            id: "test-id".into(),
            ok: true,
            payload: Some(serde_json::json!({"result": "ok"})),
            error: None,
        };
        rpc.resolve_response(response).await;

        let result = rx.await;
        assert!(matches!(result, Ok(frame) if frame.ok));
    }
}
