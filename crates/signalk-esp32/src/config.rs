//! NVS (Non-Volatile Storage) configuration for ESP32.
//!
//! Provides persistent configuration storage using ESP-IDF's NVS flash.

use serde::{Deserialize, Serialize};

/// Server configuration stored in NVS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server name identifier.
    pub name: String,

    /// SignalK protocol version.
    pub version: String,

    /// Vessel URN (unique identifier).
    pub self_urn: String,

    /// HTTP server port.
    pub http_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "signalk-server-esp32".to_string(),
            version: "1.7.0".to_string(),
            self_urn: String::new(), // Must be set before use
            http_port: 80,
        }
    }
}

impl ServerConfig {
    /// Create a new config with generated UUID.
    pub fn new_with_uuid() -> Self {
        // Note: uuid crate with v4 feature needed for this
        // For now, use a placeholder that should be replaced with actual UUID generation
        let uuid = generate_uuid();
        Self {
            self_urn: format!("vessels.urn:mrn:signalk:uuid:{}", uuid),
            ..Default::default()
        }
    }
}

/// WiFi configuration stored in NVS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiConfig {
    /// WiFi network SSID.
    pub ssid: String,

    /// WiFi network password (empty for open networks).
    pub password: String,
}

impl Default for WifiConfig {
    fn default() -> Self {
        Self {
            ssid: String::new(),
            password: String::new(),
        }
    }
}

/// Generate a simple UUID-like string.
///
/// Note: This is a simple implementation. In production, use the `uuid` crate
/// with proper entropy source, or read from ESP32's hardware RNG.
fn generate_uuid() -> String {
    // Use ESP32's random number generator if available
    // For now, use a timestamp-based approach
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos();

    // Format as UUID-like string (not cryptographically secure)
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        secs as u32,
        (secs >> 32) as u16,
        (nanos >> 16) as u16,
        nanos as u16,
        (secs ^ (nanos as u64)) & 0xFFFFFFFFFFFF
    )
}

// Future: NVS storage implementation
// pub struct NvsStorage {
//     nvs: EspDefaultNvsPartition,
// }
//
// impl NvsStorage {
//     pub fn new() -> Result<Self> { ... }
//     pub fn load_server_config(&self) -> Result<ServerConfig> { ... }
//     pub fn save_server_config(&self, config: &ServerConfig) -> Result<()> { ... }
//     pub fn load_wifi_config(&self) -> Result<WifiConfig> { ... }
//     pub fn save_wifi_config(&self, config: &WifiConfig) -> Result<()> { ... }
// }
