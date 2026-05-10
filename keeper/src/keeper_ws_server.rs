// ============================================================
// ws_server.rs – WebSocket Server for Keeper Bot
// ============================================================

use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::protocol::Message;
use serde_json::json;
use tracing::{info, error};

/// Types of updates sent via WebSocket
pub enum UpdateType {
    PriceUpdate,
    FibLevelUpdate,
    TransactionLog,
}

pub struct WsServer {
    tx: broadcast::Sender<String>,
}

impl WsServer {
    pub fn new() -> (Self, broadcast::Receiver<String>) {
        let (tx, rx) = broadcast::channel(100);
        (Self { tx }, rx)
    }

    /// Broadcast a JSON message to all connected clients
    pub fn broadcast(&self, payload: serde_json::Value) {
        if let Ok(msg) = serde_json::to_string(&payload) {
            let _ = self.tx.send(msg);
        }
    }

    /// Start the TCP listener and handle connections
    pub async fn run(self: Arc<Self>, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!("📡 WebSocket Server running on: ws://{}", addr);

        while let Ok((stream, _)) = listener.accept().await {
            let self_clone = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = self_clone.handle_connection(stream).await {
                    error!("❌ WebSocket connection error: {}", e);
                }
            });
        }
        Ok(())
    }

    async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let (mut ws_sender, mut _ws_receiver) = ws_stream.split();
        let mut rx = self.tx.subscribe();

        info!("✅ New client connected to Keeper WebSocket");

        while let Ok(msg) = rx.recv().await {
            ws_sender.send(Message::Text(msg)).await?;
        }

        Ok(())
    }
}

/// Helper to format bot data for the dashboard
pub fn create_update_payload(msg_type: &str, data: serde_json::Value) -> serde_json::Value {
    json!({
        "type": msg_type,
        "data": data,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}
