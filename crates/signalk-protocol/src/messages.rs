//! Protocol message types for WebSocket communication.
//!
//! This module defines all message types exchanged over the SignalK WebSocket protocol:
//! - Server → Client: Hello, Delta, PutResponse
//! - Client → Server: Subscribe, Unsubscribe, Put
//!
//! Messages are serialized as JSON over WebSocket text frames.

use serde::{Deserialize, Serialize};
use signalk_core::Delta;

/// Subscription request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub context: String,
    pub subscribe: Vec<Subscription>,
}

/// A single subscription specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<SubscriptionFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<SubscriptionPolicy>,
    #[serde(rename = "minPeriod", skip_serializing_if = "Option::is_none")]
    pub min_period: Option<u64>,
}

/// Subscription format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionFormat {
    Delta,
    Full,
}

/// Subscription policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionPolicy {
    Instant,
    Ideal,
    Fixed,
}

/// Unsubscribe request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeRequest {
    pub context: String,
    pub unsubscribe: Vec<UnsubscribeSpec>,
}

/// Unsubscribe specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeSpec {
    pub path: String,
}

/// PUT request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub put: PutSpec,
}

/// PUT specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutSpec {
    pub path: String,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// PUT response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutResponse {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub state: PutState,
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// PUT request state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PutState {
    Completed,
    Pending,
    Failed,
}

// ============================================================================
// Hello Message (Server → Client on connect)
// ============================================================================

/// Hello message sent by server immediately on WebSocket connection.
///
/// This message identifies the server and provides the client's context.
///
/// # Example
/// ```json
/// {
///   "name": "signalk-server-rust",
///   "version": "1.7.0",
///   "self": "vessels.urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d",
///   "roles": ["main"],
///   "timestamp": "2024-01-17T10:30:00.000Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMessage {
    /// Server name identifier.
    pub name: String,

    /// SignalK protocol version supported.
    pub version: String,

    /// The "self" context identifier for this vessel.
    #[serde(rename = "self")]
    pub self_urn: String,

    /// Server roles (e.g., ["main"], ["main", "master"]).
    pub roles: Vec<String>,

    /// Current server timestamp in ISO 8601 format.
    pub timestamp: String,
}

impl HelloMessage {
    /// Create a new Hello message.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        self_urn: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            self_urn: self_urn.into(),
            roles: vec!["main".to_string()],
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }
}

// ============================================================================
// Unified Message Enums
// ============================================================================

/// Messages that can be sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerMessage {
    /// Hello message sent on connection.
    Hello(HelloMessage),

    /// Delta update with new data.
    Delta(Delta),

    /// Response to a PUT request.
    PutResponse(PutResponse),
}

/// Messages that can be received from client.
///
/// Uses untagged deserialization - the message type is determined by
/// examining which fields are present.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClientMessage {
    /// Subscribe to data paths.
    Subscribe(SubscribeRequest),

    /// Unsubscribe from data paths.
    Unsubscribe(UnsubscribeRequest),

    /// PUT request to modify data.
    Put(PutRequest),
}

// ============================================================================
// Discovery Endpoint
// ============================================================================

/// Discovery response for `/signalk` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResponse {
    pub endpoints: DiscoveryEndpoints,
}

/// Endpoints advertised in discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryEndpoints {
    pub v1: DiscoveryV1,
}

/// Version 1 API endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryV1 {
    pub version: String,
    #[serde(rename = "signalk-http")]
    pub signalk_http: String,
    #[serde(rename = "signalk-ws")]
    pub signalk_ws: String,
}

impl DiscoveryResponse {
    /// Create a discovery response for the given host.
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            endpoints: DiscoveryEndpoints {
                v1: DiscoveryV1 {
                    version: "1.7.0".to_string(),
                    signalk_http: format!("http://{}:{}/signalk/v1/api", host, port),
                    signalk_ws: format!("ws://{}:{}/signalk/v1/stream", host, port),
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_serialization() {
        let hello = HelloMessage::new("test-server", "1.7.0", "vessels.urn:mrn:signalk:uuid:test");
        let json = serde_json::to_string(&hello).unwrap();

        assert!(json.contains("\"name\":\"test-server\""));
        assert!(json.contains("\"version\":\"1.7.0\""));
        assert!(json.contains("\"self\":\"vessels.urn:mrn:signalk:uuid:test\""));
        assert!(json.contains("\"roles\":[\"main\"]"));
    }

    #[test]
    fn test_subscribe_deserialization() {
        let json = r#"{
            "context": "vessels.self",
            "subscribe": [{"path": "navigation.*", "period": 1000}]
        }"#;

        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Subscribe(req) => {
                assert_eq!(req.context, "vessels.self");
                assert_eq!(req.subscribe.len(), 1);
                assert_eq!(req.subscribe[0].path, "navigation.*");
            }
            _ => panic!("Expected Subscribe message"),
        }
    }

    #[test]
    fn test_put_deserialization() {
        let json = r#"{
            "requestId": "12345",
            "put": {
                "path": "steering.autopilot.target.headingTrue",
                "value": 1.52
            }
        }"#;

        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Put(req) => {
                assert_eq!(req.request_id, "12345");
                assert_eq!(req.put.path, "steering.autopilot.target.headingTrue");
            }
            _ => panic!("Expected Put message"),
        }
    }

    #[test]
    fn test_discovery_response() {
        let discovery = DiscoveryResponse::new("localhost", 3000);
        let json = serde_json::to_string(&discovery).unwrap();

        assert!(json.contains("http://localhost:3000/signalk/v1/api"));
        assert!(json.contains("ws://localhost:3000/signalk/v1/stream"));
    }
}
