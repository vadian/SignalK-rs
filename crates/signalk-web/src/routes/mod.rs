//! HTTP route handlers for the Signal K server.
//!
//! This module organizes routes into submodules matching the TypeScript server's
//! API structure for compatibility.

pub mod auth;
pub mod backup;
pub mod config;
pub mod plugins;
pub mod security;

use crate::AppState;
use axum::{extract::State, response::Json, routing::get, Router};

/// Create the main Axum router with all routes.
///
/// Routes are organized as:
/// - `/signalk/v1/` - Signal K API (auth, stream, API)
/// - `/skServer/` - Server management
/// - `/admin/` - Static Admin UI files
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Discovery endpoint
        .route("/signalk", get(discovery_handler))
        // SignalK v1 API routes
        .nest("/signalk/v1", signalk_v1_routes())
        // Server management routes
        .nest("/skServer", sk_server_routes())
        .with_state(state)
}

/// Create SignalK v1 API routes.
fn signalk_v1_routes() -> Router<AppState> {
    Router::new()
        // Auth routes
        .nest("/auth", auth::auth_routes())
        // Access request routes
        .merge(auth::access_routes())
        // Plugin/app routes
        .merge(plugins::api_routes())
}

/// Create /skServer management routes.
fn sk_server_routes() -> Router<AppState> {
    Router::new()
        // Login status (from auth module)
        .merge(auth::server_routes())
        // Settings & vessel config
        .merge(config::routes())
        // Security management
        .nest("/security", security::routes())
        .merge(security::enable_security_route())
        // Plugin management
        .merge(plugins::server_routes())
        // Backup, restore, restart
        .merge(backup::routes())
}

/// Handler for `/signalk` discovery endpoint.
///
/// Returns the Signal K discovery document with available endpoints.
async fn discovery_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "endpoints": {
            "v1": {
                "version": "1.7.0",
                "signalk-http": "/signalk/v1/api",
                "signalk-ws": "/signalk/v1/stream"
            }
        },
        "server": {
            "id": state.config.name,
            "version": state.config.version
        }
    }))
}
