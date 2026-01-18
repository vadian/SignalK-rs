//! HTTP route handlers for the Signal K server.
//!
//! This module organizes routes into submodules matching the TypeScript server's
//! API structure for compatibility.

pub mod auth;
pub mod config;
pub mod security;
pub mod plugins;
pub mod backup;

use axum::{routing::get, Router};
use crate::AppState;

/// Create the main Axum router with all routes.
///
/// Routes are organized as:
/// - `/signalk/v1/` - Signal K API (auth, stream, API)
/// - `/skServer/` - Server management
/// - `/admin/` - Static Admin UI files
pub fn create_router(_state: AppState) -> Router {
    Router::new()
        // Health check / discovery
        .route("/signalk", get(discovery_handler))
        // TODO: Mount route submodules
        // .nest("/signalk/v1", signalk_v1_routes(state.clone()))
        // .nest("/skServer", server_routes(state.clone()))
        // .nest_service("/admin", admin_static_service())
}

/// Handler for `/signalk` discovery endpoint.
///
/// Returns the Signal K discovery document with available endpoints.
async fn discovery_handler() -> &'static str {
    // TODO: Return proper discovery JSON
    r#"{"endpoints":{"v1":{"version":"1.7.0","signalk-http":"/signalk/v1/api","signalk-ws":"/signalk/v1/stream"}}}"#
}
