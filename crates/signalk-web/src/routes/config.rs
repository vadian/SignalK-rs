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
//! ### `PUT /skServer/settings`
//! Updates server settings. Server may restart if critical settings change.
//!
//! ## Vessel Configuration
//!
//! ### `GET /skServer/vessel`
//! Returns vessel information stored in the SignalK tree.
//!
//! ### `PUT /skServer/vessel`
//! Updates vessel configuration.
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
use signalk_core::{InterfaceSettings, ServerSettings, VesselInfo as CoreVesselInfo};

use crate::AppState;

/// Vessel information for API (includes design/communication)
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
async fn get_settings(State(state): State<AppState>) -> Json<ServerSettings> {
    let settings = state.settings.read().await;
    // Return settings with defaults filled in
    Json(ServerSettings {
        interfaces: settings.interfaces.clone().or(Some(InterfaceSettings {
            appstore: Some(true),
            plugins: Some(true),
            rest: Some(true),
            signalk_ws: Some(true),
            tcp: Some(false),
            webapps: Some(true),
        })),
        port: settings.port.or(Some(3001)),
        sslport: settings.sslport,
        ssl: settings.ssl.or(Some(false)),
        ws_compression: settings.ws_compression.or(Some(false)),
        access_logging: settings.access_logging.or(Some(false)),
        mdns: settings.mdns.or(Some(true)),
        prune_contexts_minutes: settings.prune_contexts_minutes.or(Some(60)),
        logging_directory: settings
            .logging_directory
            .clone()
            .or(Some("~/.signalk/logs".to_string())),
        keep_most_recent_logs_only: settings.keep_most_recent_logs_only.or(Some(true)),
        log_count_to_keep: settings.log_count_to_keep.or(Some(24)),
        enable_plugin_logging: settings.enable_plugin_logging.or(Some(true)),
    })
}

/// PUT /skServer/settings
async fn put_settings(
    State(state): State<AppState>,
    Json(new_settings): Json<ServerSettings>,
) -> StatusCode {
    let mut settings = state.settings.write().await;
    *settings = new_settings;
    // TODO: Persist to file and trigger restart if needed
    StatusCode::OK
}

/// GET /skServer/vessel
async fn get_vessel(State(state): State<AppState>) -> Json<VesselInfo> {
    let vessel = state.vessel_info.read().await;
    Json(VesselInfo {
        name: vessel.name.clone(),
        mmsi: vessel.mmsi.clone(),
        uuid: vessel.uuid.clone().or(Some(state.config.self_urn.clone())),
        design: None,
        communication: vessel.callsign.clone().map(|c| VesselCommunication {
            callsign_vhf: Some(c),
        }),
    })
}

/// PUT /skServer/vessel
async fn put_vessel(
    State(state): State<AppState>,
    Json(new_vessel): Json<VesselInfo>,
) -> StatusCode {
    let mut vessel = state.vessel_info.write().await;
    if let Some(name) = new_vessel.name {
        vessel.name = Some(name);
    }
    if let Some(mmsi) = new_vessel.mmsi {
        vessel.mmsi = Some(mmsi);
    }
    if let Some(uuid) = new_vessel.uuid {
        vessel.uuid = Some(uuid);
    }
    if let Some(callsign) = new_vessel.communication.and_then(|c| c.callsign_vhf) {
        vessel.callsign = Some(callsign);
    }
    // TODO: Persist to file
    StatusCode::OK
}
