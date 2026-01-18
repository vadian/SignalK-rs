//! # signalk-server
//!
//! SignalK server implementation with pluggable async runtime.
//!
//! Enable features based on target platform:
//! - `tokio-runtime` (default) - For Linux/desktop
//! - `esp-idf-runtime` - For ESP32 (future)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use signalk_server::{SignalKServer, ServerConfig, ServerEvent};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ServerConfig {
//!         name: "my-server".to_string(),
//!         bind_addr: "0.0.0.0:3000".parse()?,
//!         ..Default::default()
//!     };
//!
//!     let server = SignalKServer::new(config);
//!     let event_tx = server.event_sender();
//!
//!     // Send deltas to the server
//!     // event_tx.send(ServerEvent::DeltaReceived(delta)).await?;
//!
//!     server.run().await
//! }
//! ```

pub use signalk_core::{Delta, MemoryStore, PathPattern, SignalKStore};

#[cfg(feature = "tokio-runtime")]
mod server;
#[cfg(feature = "tokio-runtime")]
mod subscription;

#[cfg(feature = "tokio-runtime")]
pub use server::{ServerConfig, ServerEvent, SignalKServer};
#[cfg(feature = "tokio-runtime")]
pub use subscription::{ClientSubscription, SubscriptionManager};
