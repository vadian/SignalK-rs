//! WebSocket server events for real-time Admin UI updates.
//!
//! The Admin UI connects to `/signalk/v1/stream?serverevents=all&subscribe=none`
//! to receive server event messages:
//!
//! - `VESSEL_INFO` - Vessel name and UUID (sent once on connect)
//! - `PROVIDERSTATUS` - Provider/plugin status updates
//! - `SERVERSTATISTICS` - Performance metrics (deltas/sec, paths, clients)
//! - `DEBUG_SETTINGS` - Debug configuration
//! - `RECEIVE_LOGIN_STATUS` - Authentication status
//! - `SOURCEPRIORITIES` - Source priority settings
//! - `LOG` - Real-time log entries
//!
//! ## Message Formats
//!
//! ```json
//! { "type": "VESSEL_INFO", "data": { "name": "My Boat", "uuid": "urn:mrn:..." } }
//! { "type": "SERVERSTATISTICS", "from": "signalk-server", "data": { "deltaRate": 10, ... } }
//! { "type": "PROVIDERSTATUS", "from": "signalk-server", "data": [{ "id": "nmea0183", ... }] }
//! { "type": "RECEIVE_LOGIN_STATUS", "data": { "status": "notLoggedIn", ... } }
//! ```

use serde::{Deserialize, Serialize};

/// Server event message sent over WebSocket.
///
/// These events are sent to clients that connect with `serverevents=all`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    /// Vessel information (sent once on connect).
    #[serde(rename = "VESSEL_INFO")]
    VesselInfo { data: VesselInfoData },

    /// Server statistics update (sent at ~1 Hz).
    #[serde(rename = "SERVERSTATISTICS")]
    ServerStatistics {
        from: String,
        data: ServerStatistics,
    },

    /// Provider status update (sent on change).
    #[serde(rename = "PROVIDERSTATUS")]
    ProviderStatus {
        from: String,
        data: Vec<ProviderStatus>,
    },

    /// Login/authentication status (sent once on connect).
    #[serde(rename = "RECEIVE_LOGIN_STATUS")]
    LoginStatus { data: LoginStatus },

    /// Debug settings (sent once on connect).
    #[serde(rename = "DEBUG_SETTINGS")]
    DebugSettings { data: DebugSettings },

    /// Source priorities (sent once on connect).
    #[serde(rename = "SOURCEPRIORITIES")]
    SourcePriorities { data: SourcePriorities },

    /// Log entry (sent in real-time).
    #[serde(rename = "LOG")]
    Log { data: LogEntry },
}

/// Vessel information payload for VESSEL_INFO event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VesselInfoData {
    /// Vessel name (can be null).
    pub name: Option<String>,

    /// Vessel UUID (without "vessels." prefix).
    pub uuid: String,
}

/// Login status for RECEIVE_LOGIN_STATUS event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus {
    /// Login status: "notLoggedIn", "loggedIn".
    pub status: String,

    /// Whether read-only access is allowed without login.
    pub read_only_access: bool,

    /// Whether authentication is required.
    pub authentication_required: bool,

    /// Whether new user registration is allowed.
    pub allow_new_user_registration: bool,

    /// Whether device access requests are allowed.
    pub allow_device_access_requests: bool,
}

impl Default for LoginStatus {
    fn default() -> Self {
        Self {
            status: "notLoggedIn".to_string(),
            read_only_access: true,
            authentication_required: false,
            allow_new_user_registration: false,
            allow_device_access_requests: true,
        }
    }
}

/// Debug settings for DEBUG_SETTINGS event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSettings {
    /// Debug namespaces enabled (comma-separated).
    pub debug_enabled: String,

    /// Whether to remember debug settings.
    pub remember_debug: bool,
}

/// Source priorities for SOURCEPRIORITIES event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourcePriorities {
    // Empty object for now - can be expanded later
}

/// Server performance statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

impl LogEntry {
    /// Create a new log entry with the current timestamp.
    pub fn new(level: &str, message: &str) -> Self {
        Self {
            level: level.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            namespace: None,
        }
    }

    /// Create a log entry with a namespace.
    pub fn with_namespace(level: &str, message: &str, namespace: &str) -> Self {
        Self {
            level: level.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            namespace: Some(namespace.to_string()),
        }
    }
}
