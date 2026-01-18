//! Plugin management routes.
//!
//! These endpoints manage SignalK plugins (webapps and server plugins),
//! matching the TypeScript SignalK server API for Admin UI compatibility.
//!
//! # Plugin Types
//!
//! SignalK supports two types of plugins:
//! - **Server plugins** - Run on the server, process/emit data
//! - **Webapps** - Client-side applications served by the server
//!
//! Plugins are discovered from npm packages with the keyword
//! `signalk-node-server-plugin` or `signalk-webapp`.
//!
//! # Endpoints
//!
//! ## Plugin List
//!
//! ### `GET /skServer/plugins`
//! List all installed plugins with their configuration.
//!
//! **Response:**
//! ```json
//! [
//!   {
//!     "id": "signalk-to-nmea2000",
//!     "name": "SignalK to NMEA 2000",
//!     "version": "1.2.3",
//!     "description": "Converts SignalK data to NMEA 2000",
//!     "enabled": true,
//!     "statusMessage": "Running",
//!     "data": { ... }
//!   }
//! ]
//! ```
//!
//! ## Plugin Configuration
//!
//! ### `POST /skServer/plugins/:id/config`
//! Save plugin configuration.
//!
//! **Request:**
//! ```json
//! {
//!   "enabled": true,
//!   "configuration": {
//!     "option1": "value1",
//!     "option2": 42
//!   }
//! }
//! ```
//!
//! **Response:** `200 OK`
//!
//! ## App Store
//!
//! ### `GET /signalk/v1/apps/list`
//! List available apps from the npm registry.
//!
//! **Response:**
//! ```json
//! [
//!   {
//!     "name": "@signalk/freeboard-sk",
//!     "version": "2.0.0",
//!     "description": "Navigation display",
//!     "isPlugin": true,
//!     "isWebapp": true,
//!     "installed": false,
//!     "updateAvailable": false
//!   }
//! ]
//! ```
//!
//! ## Webapps
//!
//! ### `GET /skServer/webapps`
//! List installed webapps.
//!
//! **Response:**
//! ```json
//! [
//!   {
//!     "name": "freeboard-sk",
//!     "version": "2.0.0",
//!     "description": "Navigation display",
//!     "location": "/freeboard-sk"
//!   }
//! ]
//! ```

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Plugin information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    /// Plugin identifier (npm package name).
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Installed version.
    pub version: String,

    /// Plugin description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the plugin is enabled.
    pub enabled: bool,

    /// Current status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// Plugin-specific configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Plugin configuration update.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<serde_json::Value>,
}

/// App store entry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStoreEntry {
    pub name: String,
    pub version: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub is_plugin: bool,
    pub is_webapp: bool,
    pub installed: bool,
    pub update_available: bool,
}

/// Webapp information.
#[derive(Debug, Clone, Serialize)]
pub struct Webapp {
    pub name: String,
    pub version: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// URL path where webapp is served.
    pub location: String,
}

/// Create plugin routes for /skServer/*.
pub fn server_routes() -> Router<AppState> {
    Router::new()
        .route("/plugins", get(get_plugins))
        .route("/plugins/:id/config", post(save_plugin_config))
        .route("/webapps", get(get_webapps))
}

/// Create routes for /signalk/v1/*.
pub fn api_routes() -> Router<AppState> {
    Router::new().route("/apps/list", get(get_app_list))
}

/// GET /skServer/plugins
async fn get_plugins(State(_state): State<AppState>) -> Json<Vec<Plugin>> {
    // TODO: Load actual plugin list
    Json(vec![])
}

/// POST /skServer/plugins/:id/config
async fn save_plugin_config(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Json(config): Json<PluginConfig>,
) -> StatusCode {
    // TODO: Save plugin configuration
    // Configuration is stored in ~/.signalk/plugin-config-data/{id}.json
    StatusCode::OK
}

/// GET /skServer/webapps
async fn get_webapps(State(_state): State<AppState>) -> Json<Vec<Webapp>> {
    // TODO: Load installed webapps
    Json(vec![])
}

/// GET /signalk/v1/apps/list
async fn get_app_list(State(_state): State<AppState>) -> Json<Vec<AppStoreEntry>> {
    // TODO: Query npm registry for signalk plugins/webapps
    Json(vec![])
}
