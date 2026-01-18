//! # signalk-server
//!
//! SignalK server implementation with pluggable async runtime.
//!
//! Enable features based on target platform:
//! - `tokio-runtime` (default) - For Linux/desktop
//! - `esp-idf-runtime` - For ESP32 (future)

pub use signalk_core::{Delta, MemoryStore, SignalKStore};

// TODO: Server implementation
