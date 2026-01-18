//! SignalK WebSocket server implementation.
//!
//! This module provides the core WebSocket server that handles:
//! - Client connections
//! - Hello message on connect
//! - Delta broadcasting
//! - Subscription management

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::{debug, error, info, warn};

use signalk_core::{Delta, MemoryStore, SignalKStore};
use signalk_protocol::{
    ClientMessage, HelloMessage, ServerMessage, Subscription, SubscribeRequest,
    encode_server_message,
};

use crate::subscription::{ClientSubscription, SubscriptionManager};

/// Configuration for the SignalK server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server name sent in Hello message.
    pub name: String,
    /// SignalK version.
    pub version: String,
    /// Self vessel URN.
    pub self_urn: String,
    /// Address to bind to.
    pub bind_addr: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "signalk-server-rust".to_string(),
            version: "1.7.0".to_string(),
            self_urn: "vessels.urn:mrn:signalk:uuid:00000000-0000-0000-0000-000000000000".to_string(),
            bind_addr: "0.0.0.0:3000".parse().unwrap(),
        }
    }
}

/// Events that can be sent to the server.
#[derive(Debug, Clone)]
pub enum ServerEvent {
    /// A delta was received from a provider.
    DeltaReceived(Delta),
}

/// The SignalK WebSocket server.
pub struct SignalKServer {
    config: ServerConfig,
    store: Arc<RwLock<MemoryStore>>,
    /// Channel for broadcasting deltas to all connection handlers.
    delta_tx: broadcast::Sender<Delta>,
    /// Channel for receiving events from providers.
    event_tx: mpsc::Sender<ServerEvent>,
    event_rx: mpsc::Receiver<ServerEvent>,
}

impl SignalKServer {
    /// Create a new SignalK server with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        let store = MemoryStore::new(&config.self_urn);
        let (delta_tx, _) = broadcast::channel(1024);
        let (event_tx, event_rx) = mpsc::channel(1024);

        Self {
            config,
            store: Arc::new(RwLock::new(store)),
            delta_tx,
            event_tx,
            event_rx,
        }
    }

    /// Get a sender for submitting events to the server.
    pub fn event_sender(&self) -> mpsc::Sender<ServerEvent> {
        self.event_tx.clone()
    }

    /// Get the current self URN.
    pub fn self_urn(&self) -> &str {
        &self.config.self_urn
    }

    /// Run the server, listening for WebSocket connections.
    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        info!("SignalK server listening on {}", self.config.bind_addr);

        // Spawn the event processor
        let store = self.store.clone();
        let delta_tx = self.delta_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = self.event_rx.recv().await {
                match event {
                    ServerEvent::DeltaReceived(delta) => {
                        // Apply delta to store
                        {
                            let mut store = store.write().await;
                            store.apply_delta(&delta);
                        }
                        // Broadcast to all clients
                        let _ = delta_tx.send(delta);
                    }
                }
            }
        });

        // Accept connections
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let config = self.config.clone();
                    let store = self.store.clone();
                    let delta_rx = self.delta_tx.subscribe();

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, addr, config, store, delta_rx).await {
                            error!("Connection error from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}

/// Handle a single WebSocket connection.
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    config: ServerConfig,
    store: Arc<RwLock<MemoryStore>>,
    mut delta_rx: broadcast::Receiver<Delta>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("New connection from {}", addr);

    // Parse query parameters from WebSocket handshake
    let subscribe_mode = Arc::new(RwLock::new(String::from("self")));
    let send_cached = Arc::new(RwLock::new(true));

    let subscribe_mode_clone = subscribe_mode.clone();
    let send_cached_clone = send_cached.clone();

    // Perform WebSocket handshake with callback to extract query params
    let ws_stream = tokio_tungstenite::accept_hdr_async(stream, move |req: &Request, resp: Response| {
        // Extract query parameters from the URI
        if let Some(query) = req.uri().query() {
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    match key {
                        "subscribe" => {
                            if let Ok(mut mode) = subscribe_mode_clone.try_write() {
                                *mode = value.to_string();
                            }
                        }
                        "sendCachedValues" => {
                            if let Ok(mut cached) = send_cached_clone.try_write() {
                                *cached = value == "true";
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(resp)
    })
    .await?;
    
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Send Hello message
    let hello = HelloMessage::new(&config.name, &config.version, &config.self_urn);
    let hello_msg = encode_server_message(&ServerMessage::Hello(hello))?;
    ws_tx.send(Message::Text(hello_msg)).await?;
    debug!("Sent Hello to {}", addr);

    // Initialize subscription manager for this client
    let mut subscriptions = SubscriptionManager::new(&config.self_urn);

    // Apply initial subscription based on query parameter
    let subscribe_mode_value = subscribe_mode.read().await.clone();
    match subscribe_mode_value.as_str() {
        "all" => subscriptions.subscribe_all(),
        "none" => {}, // No default subscriptions
        _ => subscriptions.subscribe_self_all(), // "self" or default
    }

    // Send cached values for initial subscription if requested
    let send_cached_value = *send_cached.read().await;
    if send_cached_value {
        let store = store.read().await;
        if let Some(delta) = subscriptions.get_initial_delta(&store) {
            let msg = encode_server_message(&ServerMessage::Delta(delta))?;
            ws_tx.send(Message::Text(msg)).await?;
        }
    }

    loop {
        tokio::select! {
            // Handle incoming messages from client
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_client_message(&text, &mut subscriptions, &mut ws_tx).await {
                            warn!("Error handling message from {}: {}", addr, e);
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("Client {} closed connection", addr);
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        ws_tx.send(Message::Pong(data)).await?;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error from {}: {}", addr, e);
                        break;
                    }
                    None => {
                        info!("Client {} disconnected", addr);
                        break;
                    }
                    _ => {} // Ignore other message types
                }
            }

            // Handle deltas broadcast from server
            delta = delta_rx.recv() => {
                match delta {
                    Ok(delta) => {
                        // Filter delta based on client subscriptions
                        if let Some(filtered) = subscriptions.filter_delta(&delta) {
                            let msg = encode_server_message(&ServerMessage::Delta(filtered))?;
                            if let Err(e) = ws_tx.send(Message::Text(msg)).await {
                                error!("Failed to send delta to {}: {}", addr, e);
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Client {} lagged {} messages", addr, n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Delta channel closed");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handle a message received from a client.
async fn handle_client_message(
    text: &str,
    subscriptions: &mut SubscriptionManager,
    ws_tx: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let msg: ClientMessage = serde_json::from_str(text)?;

    match msg {
        ClientMessage::Subscribe(req) => {
            debug!("Client subscribed to {:?}", req.subscribe);
            subscriptions.add_subscriptions(&req.context, &req.subscribe);
        }
        ClientMessage::Unsubscribe(req) => {
            debug!("Client unsubscribed from {:?}", req.unsubscribe);
            for spec in &req.unsubscribe {
                subscriptions.remove_subscription(&req.context, &spec.path);
            }
        }
        ClientMessage::Put(req) => {
            // PUT requests are not yet implemented
            warn!("PUT request not implemented: {:?}", req);
            let response = signalk_protocol::PutResponse {
                request_id: req.request_id,
                state: signalk_protocol::PutState::Failed,
                status_code: 501,
                message: Some("PUT not implemented".to_string()),
            };
            let msg = serde_json::to_string(&response)?;
            ws_tx.send(Message::Text(msg)).await?;
        }
    }

    Ok(())
}
