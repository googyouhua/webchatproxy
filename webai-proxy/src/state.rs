use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, oneshot};
use tokio_tungstenite::tungstenite::Message;
use futures_util::stream::SplitSink;
use tokio_tungstenite::WebSocketStream;

pub type PendingRequests = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

pub enum ExtSocket {
    Plain(SplitSink<WebSocketStream<tokio::net::TcpStream>, Message>),
}

pub type ExtensionSocket = Arc<Mutex<Option<(u64, ExtSocket)>>>;

pub fn next_conn_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct AppState {
    pub pending: PendingRequests,
    pub extension_socket: ExtensionSocket,
    pub active_session: Arc<Mutex<Option<String>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_conn_id_increments() {
        let a = next_conn_id();
        let b = next_conn_id();
        assert!(b > a, "connection IDs should increment");
    }

    #[test]
    fn test_app_state_creation() {
        let state = AppState {
            pending: Arc::new(Mutex::new(HashMap::new())),
            extension_socket: Arc::new(Mutex::new(None)),
            active_session: Arc::new(Mutex::new(None)),
        };
        let _ = state;
    }
}
