//! SignalK data model types.
//!
//! These types represent the core SignalK specification structures:
//! - Delta messages for efficient updates
//! - Full data model hierarchy
//! - Source tracking for multi-device scenarios

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A SignalK delta message containing one or more updates.
///
/// Deltas are the primary mechanism for transmitting changes in SignalK.
/// They contain a context (which vessel/object) and a list of updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    /// The context path (e.g., "vessels.urn:mrn:signalk:uuid:...")
    /// If None, defaults to "vessels.self"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,

    /// The list of updates in this delta
    pub updates: Vec<Update>,
}

/// A single update within a delta, containing values from one source at one timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Update {
    /// Reference to source in /sources (e.g., "nmea0183.GP")
    #[serde(rename = "$source", skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,

    /// Embedded source object (alternative to $source)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,

    /// ISO 8601 timestamp (UTC)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,

    /// The path-value pairs in this update
    pub values: Vec<PathValue>,

    /// Metadata updates (separate from values)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Vec<PathMeta>>,
}

/// A single path-value pair within an update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathValue {
    /// The SignalK path (e.g., "navigation.speedOverGround")
    pub path: String,

    /// The value at this path
    pub value: serde_json::Value,
}

/// Metadata for a path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathMeta {
    /// The SignalK path this metadata applies to
    pub path: String,

    /// The metadata value
    pub value: Meta,
}

/// Source information describing where data originated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    /// Label identifying the source bus (e.g., "N2K-1", "serial-COM1")
    pub label: String,

    /// Type of source (e.g., "NMEA0183", "NMEA2000", "signalk")
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,

    /// NMEA 2000 source address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,

    /// NMEA 2000 device CAN name
    #[serde(rename = "canName", skip_serializing_if = "Option::is_none")]
    pub can_name: Option<String>,

    /// NMEA 2000 PGN
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pgn: Option<u32>,

    /// NMEA 0183 sentence type (e.g., "RMC", "GGA")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,

    /// NMEA 0183 talker ID (e.g., "GP", "II")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub talker: Option<String>,

    /// AIS message type (1-27)
    #[serde(rename = "aisType", skip_serializing_if = "Option::is_none")]
    pub ais_type: Option<u8>,
}

/// Metadata describing a SignalK path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Display name for gauges (no units)
    #[serde(rename = "displayName", skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Long name for displays with more space
    #[serde(rename = "longName", skip_serializing_if = "Option::is_none")]
    pub long_name: Option<String>,

    /// Short name for compact displays
    #[serde(rename = "shortName", skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,

    /// SI unit string (e.g., "m/s", "rad", "K")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,

    /// Timeout in seconds after which data is stale
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,

    /// Display scale configuration
    #[serde(rename = "displayScale", skip_serializing_if = "Option::is_none")]
    pub display_scale: Option<DisplayScale>,

    /// Alarm zones
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zones: Option<Vec<Zone>>,

    /// Indicates this path supports PUT requests
    #[serde(rename = "supportsPut", skip_serializing_if = "Option::is_none")]
    pub supports_put: Option<bool>,
}

/// Display scale configuration for gauges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayScale {
    /// Lower bound of display
    pub lower: f64,

    /// Upper bound of display
    pub upper: f64,

    /// Scale type
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub scale_type: Option<ScaleType>,

    /// Power for power scale type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power: Option<f64>,
}

/// Scale type for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScaleType {
    Linear,
    Logarithmic,
    Squareroot,
    Power,
}

/// An alarm/warning zone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    /// Lower bound (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lower: Option<f64>,

    /// Upper bound (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upper: Option<f64>,

    /// Alarm state when in this zone
    pub state: AlarmState,

    /// Message to display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Alarm states in order of severity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlarmState {
    Nominal,
    Normal,
    Alert,
    Warn,
    Alarm,
    Emergency,
}

/// Hello message sent by server on WebSocket connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    /// Server name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// SignalK schema version
    pub version: String,

    /// Server timestamp (if time source available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,

    /// Self vessel URN
    #[serde(rename = "self")]
    pub self_urn: String,

    /// Server roles
    pub roles: Vec<String>,
}

/// Position in WGS84 coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_deserialize() {
        let json = r#"{
            "context": "vessels.self",
            "updates": [{
                "$source": "nmea0183.GP",
                "timestamp": "2024-01-17T10:30:00.000Z",
                "values": [
                    {"path": "navigation.speedOverGround", "value": 3.85}
                ]
            }]
        }"#;

        let delta: Delta = serde_json::from_str(json).unwrap();
        assert_eq!(delta.context, Some("vessels.self".to_string()));
        assert_eq!(delta.updates.len(), 1);
        assert_eq!(delta.updates[0].values[0].path, "navigation.speedOverGround");
    }

    #[test]
    fn test_delta_serialize() {
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test.source".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:30:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("navigation.speedOverGround"));
        assert!(json.contains("3.85"));
    }

    #[test]
    fn test_hello_serialize() {
        let hello = Hello {
            name: Some("signalk-server-rs".to_string()),
            version: "1.7.0".to_string(),
            timestamp: Some("2024-01-17T10:30:00.000Z".to_string()),
            self_urn: "vessels.urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d".to_string(),
            roles: vec!["main".to_string()],
        };

        let json = serde_json::to_string(&hello).unwrap();
        assert!(json.contains("signalk-server-rs"));
        assert!(json.contains("1.7.0"));
    }
}
