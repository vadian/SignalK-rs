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
//! use signalk_web::{WebState, create_web_routes};
//!
//! let web_state = WebState::new(store, delta_tx, config);
//! let routes = create_web_routes();
//! ```

pub mod routes;
pub mod server_events;
pub mod statistics;

// Re-exports
pub use routes::create_router;
pub use server_events::{
    DebugSettings, LogEntry, LoginStatus, ProviderStatus, ServerEvent, ServerStatistics,
    SourcePriorities, VesselInfoData,
};
pub use statistics::StatisticsCollector;

use signalk_core::{MemoryStore, ServerSettings, VesselInfo};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Server configuration.
#[derive(Debug, Clone)]
pub struct WebConfig {
    pub name: String,
    pub version: String,
    pub self_urn: String,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            name: "signalk-server-rust".to_string(),
            version: "0.1.0".to_string(),
            // self_urn must include "vessels." prefix per Signal K spec
            self_urn: "vessels.urn:mrn:signalk:uuid:00000000-0000-0000-0000-000000000000"
                .to_string(),
        }
    }
}

/// Shared server state for all route handlers.
///
/// This is wrapped in Arc and shared across all Axum handlers.
pub struct WebState {
    /// Reference to the SignalK data store.
    pub store: Arc<RwLock<MemoryStore>>,

    /// Broadcast channel for server events (statistics, logs).
    pub server_events_tx: broadcast::Sender<ServerEvent>,

    /// Statistics collector.
    pub statistics: Arc<StatisticsCollector>,

    /// Server configuration.
    pub config: WebConfig,

    /// Vessel information (cached).
    pub vessel_info: RwLock<VesselInfo>,

    /// Server settings (cached).
    pub settings: RwLock<ServerSettings>,
}

impl WebState {
    /// Create new server state.
    pub fn new(store: Arc<RwLock<MemoryStore>>, config: WebConfig) -> Self {
        let (server_events_tx, _) = broadcast::channel(256);

        Self {
            store,
            server_events_tx,
            statistics: Arc::new(StatisticsCollector::new()),
            config,
            vessel_info: RwLock::new(VesselInfo {
                name: Some("SignalK Vessel".to_string()),
                ..Default::default()
            }),
            settings: RwLock::new(ServerSettings::default()),
        }
    }

    /// Get a statistics snapshot.
    pub fn get_statistics(&self) -> ServerStatistics {
        self.statistics.snapshot()
    }

    /// Broadcast a server event to all listeners.
    pub fn broadcast_event(&self, event: ServerEvent) {
        let _ = self.server_events_tx.send(event);
    }

    /// Subscribe to server events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<ServerEvent> {
        self.server_events_tx.subscribe()
    }
}

/// Type alias for shared state in Axum handlers.
pub type AppState = Arc<WebState>;
