use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, oneshot};
use tokio_tungstenite::tungstenite::Message;
use futures_util::stream::SplitSink;
use tokio_tungstenite::WebSocketStream;

pub type PendingRequests = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

pub enum ExtSocket {
    Plain(SplitSink<WebSocketStream<tokio::net::TcpStream>, Message>),
    Tls(SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>),
}

pub type ExtensionSocket = Arc<Mutex<Option<(u64, ExtSocket)>>>;
pub type Results = Arc<Mutex<HashMap<String, String>>>;

pub fn next_conn_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct AppState {
    pub pending: PendingRequests,
    pub auth: crate::auth::Auth,
    pub extension_socket: ExtensionSocket,
    pub sessions: Arc<Mutex<HashSet<String>>>,
    pub results: Results,
    pub ticket_counter: Arc<Mutex<u64>>,
    pub busy_sessions: Arc<Mutex<HashSet<String>>>,
}
