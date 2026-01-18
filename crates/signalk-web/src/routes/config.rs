//! Server configuration routes.
//!
//! These endpoints manage server and vessel configuration, matching the
//! TypeScript SignalK server API for compatibility with the Admin UI.
//!
//! # Endpoints
//!
//! ## Server Settings
//!
//! ### `GET /skServer/settings`
//! Returns the current server settings.
//!
//! **Response:**
//! ```json
//! {
//!   "interfaces": {
//!     "appstore": true,
//!     "plugins": true,
//!     "rest": true,
//!     "signalk-ws": true
//!   },
//!   "port": 3000,
//!   "ssl": false,
//!   "wsCompression": false,
//!   "accessLogging": false,
//!   "mdns": true,
//!   "pruneContextsMinutes": 60,
//!   "loggingDirectory": "~/.signalk/logs",
//!   "keepMostRecentLogsOnly": true,
//!   "logCountToKeep": 24,
//!   "enablePluginLogging": true
//! }
//! ```
//!
//! ### `PUT /skServer/settings`
//! Updates server settings. Server may restart if critical settings change.
//!
//! **Request:** Same schema as GET response.
//!
//! **Response:** `200 OK` on success.
//!
//! ## Vessel Configuration
//!
//! ### `GET /skServer/vessel`
//! Returns vessel information stored in the SignalK tree.
//!
//! **Response:**
//! ```json
//! {
//!   "name": "My Boat",
//!   "mmsi": "123456789",
//!   "uuid": "urn:mrn:signalk:uuid:...",
//!   "design": {
//!     "length": { "value": { "overall": 12.5 } },
//!     "beam": { "value": 4.2 },
//!     "draft": { "value": { "maximum": 1.8 } }
//!   },
//!   "communication": {
//!     "callsignVhf": "WXY1234"
//!   }
//! }
//! ```
//!
//! ### `PUT /skServer/vessel`
//! Updates vessel configuration.
//!
//! **Request:** Same schema as GET response.
//!
//! **Response:** `200 OK` on success.
//!
//! # Configuration File
//!
//! Settings are persisted to `~/.signalk/settings.json` in a format
//! compatible with the TypeScript SignalK server.

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Server settings matching TypeScript implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettings {
    /// Interface enable/disable flags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<InterfaceSettings>,

    /// HTTP port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// HTTPS port (when SSL enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sslport: Option<u16>,

    /// Enable SSL/TLS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl: Option<bool>,

    /// Enable WebSocket compression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_compression: Option<bool>,

    /// Enable access logging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_logging: Option<bool>,

    /// Enable mDNS discovery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mdns: Option<bool>,

    /// Minutes before pruning inactive contexts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune_contexts_minutes: Option<u32>,

    /// Log file directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging_directory: Option<String>,

    /// Keep only recent logs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_most_recent_logs_only: Option<bool>,

    /// Number of log files to retain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_count_to_keep: Option<u32>,

    /// Enable plugin logging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_plugin_logging: Option<bool>,
}

/// Interface enable/disable settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceSettings {
    pub appstore: Option<bool>,
    pub plugins: Option<bool>,
    pub rest: Option<bool>,
    #[serde(rename = "signalk-ws")]
    pub signalk_ws: Option<bool>,
    pub tcp: Option<bool>,
    pub webapps: Option<bool>,
}

/// Vessel information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VesselInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mmsi: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub design: Option<VesselDesign>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub communication: Option<VesselCommunication>,
}

/// Vessel design specifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VesselDesign {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub beam: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_height: Option<serde_json::Value>,
}

/// Vessel communication settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VesselCommunication {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callsign_vhf: Option<String>,
}

/// Create configuration routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(get_settings).put(put_settings))
        .route("/vessel", get(get_vessel).put(put_vessel))
}

/// GET /skServer/settings
async fn get_settings(State(_state): State<AppState>) -> Json<ServerSettings> {
    // TODO: Load from configuration file
    Json(ServerSettings {
        interfaces: Some(InterfaceSettings {
            appstore: Some(true),
            plugins: Some(true),
            rest: Some(true),
            signalk_ws: Some(true),
            tcp: Some(false),
            webapps: Some(true),
        }),
        port: Some(3000),
        sslport: None,
        ssl: Some(false),
        ws_compression: Some(false),
        access_logging: Some(false),
        mdns: Some(true),
        prune_contexts_minutes: Some(60),
        logging_directory: Some("~/.signalk/logs".to_string()),
        keep_most_recent_logs_only: Some(true),
        log_count_to_keep: Some(24),
        enable_plugin_logging: Some(true),
    })
}

/// PUT /skServer/settings
async fn put_settings(
    State(_state): State<AppState>,
    Json(_settings): Json<ServerSettings>,
) -> StatusCode {
    // TODO: Save to configuration file
    // TODO: Trigger restart if needed
    StatusCode::OK
}

/// GET /skServer/vessel
async fn get_vessel(State(_state): State<AppState>) -> Json<VesselInfo> {
    // TODO: Load from SignalK store
    Json(VesselInfo {
        name: Some("SignalK Vessel".to_string()),
        mmsi: None,
        uuid: None,
        design: None,
        communication: None,
    })
}

/// PUT /skServer/vessel
async fn put_vessel(
    State(_state): State<AppState>,
    Json(_vessel): Json<VesselInfo>,
) -> StatusCode {
    // TODO: Update SignalK store and persist
    StatusCode::OK
}
