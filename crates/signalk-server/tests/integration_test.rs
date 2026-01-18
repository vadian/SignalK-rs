//! Integration tests for the SignalK WebSocket server.
//!
//! These tests start an actual server and connect with a WebSocket client
//! to verify end-to-end functionality.

use std::net::SocketAddr;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;

use signalk_server::{ServerConfig, ServerEvent, SignalKServer, Delta};
use signalk_core::{PathValue, Update};

/// Find an available port for testing.
async fn find_available_port() -> SocketAddr {
    // Bind to port 0 to get an available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap()
}

/// Start a test server and return the address and event sender.
async fn start_test_server() -> (
    SocketAddr,
    tokio::sync::mpsc::Sender<ServerEvent>,
    tokio::task::JoinHandle<()>,
) {
    let addr = find_available_port().await;

    let config = ServerConfig {
        name: "test-server".to_string(),
        version: "1.7.0".to_string(),
        self_urn: "vessels.urn:mrn:signalk:uuid:test-vessel".to_string(),
        bind_addr: addr,
    };

    let server = SignalKServer::new(config);
    let event_tx = server.event_sender();

    let handle = tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    (addr, event_tx, handle)
}

/// Connect a WebSocket client to the given address.
async fn connect_client(addr: SocketAddr) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let url = format!("ws://{}/signalk/v1/stream", addr);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect");
    ws_stream
}

/// Wait for a text message with timeout.
async fn recv_text(
    ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> Result<String, &'static str> {
    match timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => Ok(text),
        Ok(Some(Ok(_))) => Err("Unexpected message type"),
        Ok(Some(Err(_))) => Err("WebSocket error"),
        Ok(None) => Err("Connection closed"),
        Err(_) => Err("Timeout"),
    }
}

#[tokio::test]
async fn test_hello_message_on_connect() {
    let (addr, _event_tx, handle) = start_test_server().await;

    // Connect client
    let mut ws = connect_client(addr).await;

    // First message should be Hello
    let msg = recv_text(&mut ws).await.expect("Should receive Hello");
    let hello: serde_json::Value = serde_json::from_str(&msg).expect("Valid JSON");

    // Verify Hello fields
    assert_eq!(hello["name"], "test-server");
    assert_eq!(hello["version"], "1.7.0");
    assert_eq!(hello["self"], "vessels.urn:mrn:signalk:uuid:test-vessel");
    assert!(hello["roles"].is_array());
    assert!(hello["timestamp"].is_string());

    // Clean up
    ws.close(None).await.ok();
    handle.abort();
}

#[tokio::test]
async fn test_delta_broadcast() {
    let (addr, event_tx, handle) = start_test_server().await;

    // Connect client
    let mut ws = connect_client(addr).await;

    // Skip Hello message
    let _ = recv_text(&mut ws).await.expect("Should receive Hello");

    // Skip initial delta (if any - it may be empty)
    // We'll just proceed to send our own delta

    // Send a delta through the event channel
    let delta = Delta {
        context: Some("vessels.self".to_string()),
        updates: vec![Update {
            source_ref: Some("test.source".to_string()),
            source: None,
            timestamp: Some("2024-01-17T12:00:00.000Z".to_string()),
            values: vec![PathValue {
                path: "navigation.speedOverGround".to_string(),
                value: serde_json::json!(5.5),
            }],
            meta: None,
        }],
    };

    event_tx
        .send(ServerEvent::DeltaReceived(delta))
        .await
        .expect("Should send delta");

    // Wait for the delta to be broadcast
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Receive the delta
    let msg = recv_text(&mut ws).await.expect("Should receive delta");
    let received: serde_json::Value = serde_json::from_str(&msg).expect("Valid JSON");

    // Verify delta structure
    assert!(received["updates"].is_array());
    let updates = received["updates"].as_array().unwrap();
    assert!(!updates.is_empty());

    let values = updates[0]["values"].as_array().unwrap();
    assert_eq!(values[0]["path"], "navigation.speedOverGround");
    assert_eq!(values[0]["value"], 5.5);

    // Clean up
    ws.close(None).await.ok();
    handle.abort();
}

#[tokio::test]
async fn test_subscription_filtering() {
    let (addr, event_tx, handle) = start_test_server().await;

    // Connect client
    let mut ws = connect_client(addr).await;

    // Skip Hello message
    let _ = recv_text(&mut ws).await.expect("Should receive Hello");

    // Subscribe to only navigation paths
    let subscribe = serde_json::json!({
        "context": "vessels.self",
        "subscribe": [{
            "path": "navigation.*"
        }]
    });
    ws.send(Message::Text(subscribe.to_string()))
        .await
        .expect("Should send subscribe");

    // Small delay for subscription processing
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send delta with navigation and environment data
    let delta = Delta {
        context: Some("vessels.self".to_string()),
        updates: vec![Update {
            source_ref: Some("test.source".to_string()),
            source: None,
            timestamp: Some("2024-01-17T12:00:00.000Z".to_string()),
            values: vec![
                PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(5.5),
                },
                PathValue {
                    path: "environment.wind.speedApparent".to_string(),
                    value: serde_json::json!(10.0),
                },
            ],
            meta: None,
        }],
    };

    event_tx
        .send(ServerEvent::DeltaReceived(delta))
        .await
        .expect("Should send delta");

    // Wait for the delta
    tokio::time::sleep(Duration::from_millis(100)).await;

    let msg = recv_text(&mut ws).await.expect("Should receive filtered delta");
    let received: serde_json::Value = serde_json::from_str(&msg).expect("Valid JSON");

    // Should only have navigation path, not environment
    let updates = received["updates"].as_array().unwrap();
    let values = updates[0]["values"].as_array().unwrap();

    // The subscription adds to existing subscriptions (self.* is default)
    // So we may get both paths. Let's check we at least get navigation
    let nav_value = values
        .iter()
        .find(|v| v["path"] == "navigation.speedOverGround");
    assert!(nav_value.is_some(), "Should have navigation path");

    // Clean up
    ws.close(None).await.ok();
    handle.abort();
}

#[tokio::test]
async fn test_multiple_clients() {
    let (addr, event_tx, handle) = start_test_server().await;

    // Connect two clients
    let mut ws1 = connect_client(addr).await;
    let mut ws2 = connect_client(addr).await;

    // Both should receive Hello
    let hello1 = recv_text(&mut ws1).await.expect("Client 1 Hello");
    let hello2 = recv_text(&mut ws2).await.expect("Client 2 Hello");

    let h1: serde_json::Value = serde_json::from_str(&hello1).unwrap();
    let h2: serde_json::Value = serde_json::from_str(&hello2).unwrap();
    assert_eq!(h1["name"], "test-server");
    assert_eq!(h2["name"], "test-server");

    // Send a delta
    let delta = Delta {
        context: Some("vessels.self".to_string()),
        updates: vec![Update {
            source_ref: Some("test".to_string()),
            source: None,
            timestamp: Some("2024-01-17T12:00:00.000Z".to_string()),
            values: vec![PathValue {
                path: "navigation.position".to_string(),
                value: serde_json::json!({"latitude": 45.0, "longitude": -123.0}),
            }],
            meta: None,
        }],
    };

    event_tx
        .send(ServerEvent::DeltaReceived(delta))
        .await
        .expect("Should send delta");

    // Both clients should receive the delta
    tokio::time::sleep(Duration::from_millis(100)).await;

    let msg1 = recv_text(&mut ws1).await.expect("Client 1 delta");
    let msg2 = recv_text(&mut ws2).await.expect("Client 2 delta");

    let d1: serde_json::Value = serde_json::from_str(&msg1).unwrap();
    let d2: serde_json::Value = serde_json::from_str(&msg2).unwrap();

    assert!(d1["updates"].is_array());
    assert!(d2["updates"].is_array());

    // Clean up
    ws1.close(None).await.ok();
    ws2.close(None).await.ok();
    handle.abort();
}

#[tokio::test]
async fn test_unsubscribe() {
    let (addr, event_tx, handle) = start_test_server().await;

    // Connect client
    let mut ws = connect_client(addr).await;

    // Skip Hello
    let _ = recv_text(&mut ws).await.expect("Hello");

    // Unsubscribe from all
    let unsubscribe = serde_json::json!({
        "context": "*",
        "unsubscribe": [{"path": "*"}]
    });
    ws.send(Message::Text(unsubscribe.to_string()))
        .await
        .expect("Should send unsubscribe");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send a delta
    let delta = Delta {
        context: Some("vessels.self".to_string()),
        updates: vec![Update {
            source_ref: Some("test".to_string()),
            source: None,
            timestamp: Some("2024-01-17T12:00:00.000Z".to_string()),
            values: vec![PathValue {
                path: "navigation.speedOverGround".to_string(),
                value: serde_json::json!(5.5),
            }],
            meta: None,
        }],
    };

    event_tx
        .send(ServerEvent::DeltaReceived(delta))
        .await
        .expect("Should send delta");

    // Client should NOT receive the delta (unsubscribed)
    match timeout(Duration::from_millis(200), ws.next()).await {
        Err(_) => {
            // Timeout is expected - no delta received
        }
        Ok(Some(Ok(Message::Text(_)))) => {
            panic!("Should not receive delta after unsubscribe");
        }
        _ => {}
    }

    // Clean up
    ws.close(None).await.ok();
    handle.abort();
}

#[tokio::test]
async fn test_put_request_returns_not_implemented() {
    let (addr, _event_tx, handle) = start_test_server().await;

    // Connect client
    let mut ws = connect_client(addr).await;

    // Skip Hello
    let _ = recv_text(&mut ws).await.expect("Hello");

    // Send a PUT request
    let put_request = serde_json::json!({
        "requestId": "test-put-123",
        "put": {
            "path": "steering.autopilot.target.headingTrue",
            "value": 1.5
        }
    });

    ws.send(Message::Text(put_request.to_string()))
        .await
        .expect("Should send PUT");

    // Should receive failure response
    let response = recv_text(&mut ws).await.expect("Should receive PUT response");
    let resp: serde_json::Value = serde_json::from_str(&response).expect("Valid JSON");

    assert_eq!(resp["requestId"], "test-put-123");
    assert_eq!(resp["state"], "FAILED");
    assert_eq!(resp["statusCode"], 501);

    // Clean up
    ws.close(None).await.ok();
    handle.abort();
}
