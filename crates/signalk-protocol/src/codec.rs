//! WebSocket message codec for SignalK protocol.
//!
//! SignalK uses JSON messages over WebSocket text frames. This module provides
//! encoding and decoding utilities for the protocol messages.

use crate::messages::{ClientMessage, ServerMessage};
use thiserror::Error;

/// Errors that can occur during message encoding/decoding.
#[derive(Debug, Error)]
pub enum CodecError {
    /// JSON serialization failed.
    #[error("Failed to serialize message: {0}")]
    SerializeError(#[from] serde_json::Error),

    /// Received binary frame instead of text.
    #[error("Expected text frame, received binary")]
    BinaryFrame,

    /// Message type could not be determined.
    #[error("Unknown message type")]
    UnknownMessage,
}

/// Encode a server message to JSON string for WebSocket transmission.
pub fn encode_server_message(msg: &ServerMessage) -> Result<String, CodecError> {
    serde_json::to_string(msg).map_err(CodecError::from)
}

/// Decode a client message from JSON string received over WebSocket.
pub fn decode_client_message(text: &str) -> Result<ClientMessage, CodecError> {
    serde_json::from_str(text).map_err(CodecError::from)
}

/// Check if a JSON message appears to be a subscribe request.
///
/// This is useful for quick message type detection without full parsing.
pub fn is_subscribe_message(text: &str) -> bool {
    text.contains("\"subscribe\"")
}

/// Check if a JSON message appears to be an unsubscribe request.
pub fn is_unsubscribe_message(text: &str) -> bool {
    text.contains("\"unsubscribe\"")
}

/// Check if a JSON message appears to be a PUT request.
pub fn is_put_message(text: &str) -> bool {
    text.contains("\"put\"") && text.contains("\"requestId\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::HelloMessage;
    use signalk_core::{Delta, Update, PathValue};

    #[test]
    fn test_encode_hello() {
        let hello = HelloMessage::new("test", "1.7.0", "vessels.self");
        let msg = ServerMessage::Hello(hello);
        let json = encode_server_message(&msg).unwrap();

        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"version\":\"1.7.0\""));
    }

    #[test]
    fn test_encode_delta() {
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test.source".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:00Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.5),
                }],
                meta: None,
            }],
        };
        let msg = ServerMessage::Delta(delta);
        let json = encode_server_message(&msg).unwrap();

        assert!(json.contains("\"context\":\"vessels.self\""));
        assert!(json.contains("\"navigation.speedOverGround\""));
    }

    #[test]
    fn test_decode_subscribe() {
        let json = r#"{"context":"vessels.self","subscribe":[{"path":"navigation.*"}]}"#;
        let msg = decode_client_message(json).unwrap();

        match msg {
            ClientMessage::Subscribe(req) => {
                assert_eq!(req.context, "vessels.self");
            }
            _ => panic!("Expected Subscribe"),
        }
    }

    #[test]
    fn test_decode_put() {
        let json = r#"{"requestId":"123","put":{"path":"test.path","value":42}}"#;
        let msg = decode_client_message(json).unwrap();

        match msg {
            ClientMessage::Put(req) => {
                assert_eq!(req.request_id, "123");
                assert_eq!(req.put.path, "test.path");
            }
            _ => panic!("Expected Put"),
        }
    }

    #[test]
    fn test_message_type_detection() {
        assert!(is_subscribe_message(r#"{"subscribe":[...]}"#));
        assert!(is_unsubscribe_message(r#"{"unsubscribe":[...]}"#));
        assert!(is_put_message(r#"{"requestId":"1","put":{...}}"#));

        assert!(!is_subscribe_message(r#"{"put":{...}}"#));
    }
}
