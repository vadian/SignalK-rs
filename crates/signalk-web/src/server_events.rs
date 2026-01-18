//! WebSocket server events for real-time Admin UI updates.
//!
//! The Admin UI connects to `/signalk/v1/stream?serverevents=all&subscribe=none`
//! to receive server event messages:
//!
//! - `PROVIDERSTATUS` - Provider/plugin status updates
//! - `SERVERSTATISTICS` - Performance metrics (deltas/sec, paths, clients)
//! - `LOG` - Real-time log entries
//!
//! ## Message Formats
//!
//! ```json
//! { "type": "SERVERSTATISTICS", "data": { "deltaRate": 10, "numberOfAvailablePaths": 150, ... } }
//! { "type": "PROVIDERSTATUS", "data": [{ "id": "nmea0183", "connected": true, ... }] }
//! { "type": "LOG", "data": { "level": "info", "message": "...", "timestamp": "..." } }
//! ```

use serde::{Deserialize, Serialize};

/// Server event message sent over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerEvent {
    /// Server statistics update (sent at ~1 Hz).
    #[serde(rename = "SERVERSTATISTICS")]
    ServerStatistics(ServerStatistics),

    /// Provider status update (sent on change).
    #[serde(rename = "PROVIDERSTATUS")]
    ProviderStatus(Vec<ProviderStatus>),

    /// Log entry (sent in real-time).
    #[serde(rename = "LOG")]
    Log(LogEntry),
}

/// Server performance statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatistics {
    /// Deltas processed per second.
    pub delta_rate: f64,

    /// Number of unique paths with values.
    pub number_of_available_paths: usize,

    /// Connected WebSocket clients.
    pub ws_clients: usize,

    /// Server uptime in seconds.
    pub uptime: u64,

    /// Per-provider statistics.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub provider_statistics: Vec<ProviderStatistics>,
}

/// Statistics for a single data provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatistics {
    /// Provider identifier.
    pub id: String,

    /// Deltas received from this provider.
    pub delta_count: u64,
}

/// Status of a data provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    /// Provider identifier.
    pub id: String,

    /// Provider type (e.g., "NMEA0183", "NMEA2000").
    pub provider_type: String,

    /// Whether the provider is connected.
    pub connected: bool,

    /// Error message if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Log entry for real-time log streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Log level: "debug", "info", "warn", "error".
    pub level: String,

    /// Log message.
    pub message: String,

    /// ISO 8601 timestamp.
    pub timestamp: String,

    /// Optional namespace (for debug filtering).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

// TODO: Implement WebSocket handler for server events
// TODO: Implement broadcast channel for server events
