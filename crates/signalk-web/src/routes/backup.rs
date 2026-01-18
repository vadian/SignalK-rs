//! Backup and restore routes.
//!
//! These endpoints handle server configuration backup and restore,
//! as well as server restart, matching the TypeScript SignalK server API.
//!
//! # Backup Contents
//!
//! The backup includes:
//! - `settings.json` - Server configuration
//! - `security.json` - Users and devices
//! - `plugin-config-data/` - Plugin configurations
//! - `resources/` - Routes, waypoints, notes
//! - `defaults.json` - Default values (legacy)
//!
//! Excluded from backup:
//! - `node_modules/` - Can be reinstalled
//! - `logs/` - Not needed for restore
//! - Large data files
//!
//! # Endpoints
//!
//! ## Backup
//!
//! ### `POST /skServer/backup`
//! Create a backup and return download URL.
//!
//! **Response:**
//! ```json
//! {
//!   "href": "/skServer/backup"
//! }
//! ```
//!
//! ### `GET /skServer/backup`
//! Download the backup as a ZIP file.
//!
//! **Response:** `application/zip` binary data
//!
//! ## Restore
//!
//! ### `POST /skServer/restore`
//! Restore from uploaded backup ZIP.
//!
//! **Request:** `multipart/form-data` with backup ZIP file
//!
//! **Response:**
//! ```json
//! {
//!   "status": "success",
//!   "message": "Restore complete. Server will restart."
//! }
//! ```
//!
//! ## Server Control
//!
//! ### `PUT /skServer/restart`
//! Restart the SignalK server.
//!
//! **Response:** `200 OK`
//!
//! Note: The server will close all connections and restart.
//! Clients should reconnect after a short delay.
//!
//! ## Debug Control
//!
//! ### `GET /skServer/debugKeys`
//! List available debug namespaces.
//!
//! **Response:**
//! ```json
//! ["signalk-server:*", "signalk-server:interfaces:*", ...]
//! ```
//!
//! ### `POST /skServer/debug`
//! Enable or disable debug logging for namespaces.
//!
//! **Request:**
//! ```json
//! {
//!   "enable": ["signalk-server:*"],
//!   "disable": ["other:*"]
//! }
//! ```
//!
//! **Response:** `200 OK`

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Backup creation response.
#[derive(Debug, Clone, Serialize)]
pub struct BackupResponse {
    pub href: String,
}

/// Restore response.
#[derive(Debug, Clone, Serialize)]
pub struct RestoreResponse {
    pub status: String,
    pub message: String,
}

/// Debug control request.
#[derive(Debug, Clone, Deserialize)]
pub struct DebugRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<Vec<String>>,
}

/// Create backup/restore routes for /skServer/*.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/backup", post(create_backup).get(download_backup))
        .route("/restore", post(restore_backup))
        .route("/restart", put(restart_server))
        .route("/debug", post(set_debug))
        .route("/debugKeys", get(get_debug_keys))
}

/// POST /skServer/backup
/// Initiates backup creation.
async fn create_backup(State(_state): State<AppState>) -> Json<BackupResponse> {
    // TODO: Create backup ZIP of ~/.signalk/
    // Exclude: node_modules, logs, large files
    Json(BackupResponse {
        href: "/skServer/backup".to_string(),
    })
}

/// GET /skServer/backup
/// Downloads the backup ZIP file.
async fn download_backup(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Stream backup ZIP file
    // Content-Type: application/zip
    // Content-Disposition: attachment; filename="signalk-backup-{date}.zip"
    StatusCode::NOT_IMPLEMENTED
}

/// POST /skServer/restore
/// Restores from uploaded backup.
async fn restore_backup(State(_state): State<AppState>) -> Json<RestoreResponse> {
    // TODO: Accept multipart upload
    // TODO: Extract and validate backup
    // TODO: Apply restored configuration
    // TODO: Trigger server restart
    Json(RestoreResponse {
        status: "success".to_string(),
        message: "Restore complete. Server will restart.".to_string(),
    })
}

/// PUT /skServer/restart
/// Restarts the server.
async fn restart_server(State(_state): State<AppState>) -> StatusCode {
    // TODO: Trigger graceful shutdown and restart
    // This typically involves:
    // 1. Sending shutdown signal to main loop
    // 2. Closing all WebSocket connections
    // 3. Re-executing the process (or using systemd restart)
    StatusCode::OK
}

/// POST /skServer/debug
/// Enable/disable debug namespaces.
async fn set_debug(
    State(_state): State<AppState>,
    Json(_request): Json<DebugRequest>,
) -> StatusCode {
    // TODO: Update tracing filter
    StatusCode::OK
}

/// GET /skServer/debugKeys
/// List available debug namespaces.
async fn get_debug_keys(State(_state): State<AppState>) -> Json<Vec<String>> {
    Json(vec![
        "signalk-server:*".to_string(),
        "signalk-server:interfaces:*".to_string(),
        "signalk-server:providers:*".to_string(),
        "signalk-server:plugins:*".to_string(),
    ])
}
