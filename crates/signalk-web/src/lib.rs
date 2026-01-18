//! # signalk-web
//!
//! Admin Web UI and REST API for Signal K server.
//!
//! This crate provides:
//! - REST API endpoints compatible with the TypeScript Signal K server
//! - WebSocket server events for real-time dashboard updates
//! - Static file serving for the Admin UI
//! - Server statistics collection and broadcasting
//!
//! ## Architecture
//!
//! The web layer is built on Axum and provides these route groups:
//!
//! - `/admin/` - Static files for React Admin UI
//! - `/signalk/v1/` - Signal K REST API and WebSocket
//! - `/skServer/` - Server management endpoints
//!
//! ## Usage
//!
//! ```rust,ignore
//! use signalk_web::{create_router, ServerState};
//!
//! let state = ServerState::new(server, config);
//! let app = create_router(state);
//!
//! let listener = TcpListener::bind("0.0.0.0:3000").await?;
//! axum::serve(listener, app).await?;
//! ```

pub mod routes;
pub mod server_events;
pub mod statistics;

// Re-exports
pub use routes::create_router;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared server state for all route handlers.
///
/// This is wrapped in Arc and shared across all Axum handlers.
pub struct ServerState {
    // TODO: Add reference to SignalKServer
    // TODO: Add reference to configuration
    // TODO: Add statistics collector
    _placeholder: (),
}

impl ServerState {
    /// Create new server state.
    pub fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for shared state in Axum handlers.
pub type AppState = Arc<RwLock<ServerState>>;
