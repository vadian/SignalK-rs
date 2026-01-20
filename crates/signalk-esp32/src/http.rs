//! HTTP handler utilities for ESP32 SignalK server.
//!
//! Provides helper functions for building SignalK-compliant HTTP responses.

use signalk_core::MemoryStore;
use signalk_core::SignalKStore;
use signalk_protocol::{DiscoveryResponse, HelloMessage, ServerMessage};
use std::sync::{Arc, Mutex};

/// Create a HelloMessage for WebSocket connections.
pub fn create_hello_message(name: &str, version: &str, self_urn: &str) -> ServerMessage {
    let hello = HelloMessage::new(name, version, self_urn);
    ServerMessage::Hello(hello)
}

/// Create a discovery response JSON string.
pub fn create_discovery_json(host: &str, port: u16) -> Result<String, serde_json::Error> {
    let discovery = DiscoveryResponse::new(host, port);
    serde_json::to_string(&discovery)
}

/// Get the full SignalK data model as JSON.
pub fn get_full_model_json(store: &Arc<Mutex<MemoryStore>>) -> Result<String, String> {
    match store.lock() {
        Ok(store) => serde_json::to_string(store.full_model()).map_err(|e| e.to_string()),
        Err(_) => Err("Store is locked".to_string()),
    }
}

/// Get a specific path from the SignalK data model.
pub fn get_path_json(store: &Arc<Mutex<MemoryStore>>, path: &str) -> Result<String, String> {
    match store.lock() {
        Ok(store) => match store.get_path(path) {
            Some(value) => serde_json::to_string(&value).map_err(|e| e.to_string()),
            None => Err(format!("Path not found: {}", path)),
        },
        Err(_) => Err("Store is locked".to_string()),
    }
}

/// Get current timestamp in ISO 8601 format.
///
/// Note: Without NTP, this returns time since boot. Configure SNTP for accurate timestamps.
pub fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    // If time looks valid (after year 2020), format properly
    if secs > 1577836800 {
        // 2020-01-01
        // Calculate date components (simplified - doesn't handle leap years perfectly)
        let days = secs / 86400;
        let time_secs = secs % 86400;

        // Approximate year calculation
        let year = 1970 + (days / 365);
        let day_of_year = days % 365;

        // Approximate month/day (simplified)
        let month = (day_of_year / 30) + 1;
        let day = (day_of_year % 30) + 1;

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            year,
            month.min(12),
            day.min(31),
            (time_secs / 3600) % 24,
            (time_secs / 60) % 60,
            time_secs % 60,
            millis
        )
    } else {
        // Time since boot (NTP not configured)
        format!(
            "1970-01-01T{:02}:{:02}:{:02}.{:03}Z",
            (secs / 3600) % 24,
            (secs / 60) % 60,
            secs % 60,
            millis
        )
    }
}
