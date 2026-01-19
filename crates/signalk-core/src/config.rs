//! Configuration storage abstraction.
//!
//! This module provides traits for configuration storage that can be
//! implemented differently on each platform:
//! - Linux: File-based storage (`~/.signalk/`)
//! - ESP32: NVS (Non-Volatile Storage)
//!
//! By abstracting storage, REST API handler logic can be shared
//! between platforms.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;

/// Errors that can occur during configuration operations.
#[derive(Debug)]
pub enum ConfigError {
    /// The requested configuration was not found.
    NotFound(String),
    /// Failed to read configuration.
    ReadError(String),
    /// Failed to write configuration.
    WriteError(String),
    /// Configuration data is invalid.
    InvalidData(String),
    /// Storage is not available.
    StorageUnavailable(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound(key) => write!(f, "Configuration not found: {}", key),
            ConfigError::ReadError(msg) => write!(f, "Read error: {}", msg),
            ConfigError::WriteError(msg) => write!(f, "Write error: {}", msg),
            ConfigError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            ConfigError::StorageUnavailable(msg) => write!(f, "Storage unavailable: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Abstract configuration storage.
///
/// Implementations provide platform-specific storage mechanisms:
/// - `FileConfigStorage` for Linux (file-based)
/// - `NvsConfigStorage` for ESP32 (flash-based)
///
/// All methods are synchronous to support embedded platforms.
/// Async wrappers can be added at the framework layer.
pub trait ConfigStorage: Send + Sync {
    // ========================================================================
    // Server Settings
    // ========================================================================

    /// Load server settings.
    fn load_settings(&self) -> Result<ServerSettings, ConfigError>;

    /// Save server settings.
    fn save_settings(&self, settings: &ServerSettings) -> Result<(), ConfigError>;

    // ========================================================================
    // Vessel Information
    // ========================================================================

    /// Load vessel information.
    fn load_vessel(&self) -> Result<VesselInfo, ConfigError>;

    /// Save vessel information.
    fn save_vessel(&self, vessel: &VesselInfo) -> Result<(), ConfigError>;

    // ========================================================================
    // Security Configuration
    // ========================================================================

    /// Load security configuration.
    fn load_security(&self) -> Result<SecurityConfig, ConfigError>;

    /// Save security configuration.
    fn save_security(&self, config: &SecurityConfig) -> Result<(), ConfigError>;

    // ========================================================================
    // Plugin Configuration
    // ========================================================================

    /// Load configuration for a specific plugin.
    fn load_plugin_config(&self, plugin_id: &str) -> Result<serde_json::Value, ConfigError>;

    /// Save configuration for a specific plugin.
    fn save_plugin_config(
        &self,
        plugin_id: &str,
        config: &serde_json::Value,
    ) -> Result<(), ConfigError>;

    /// List all plugin IDs with saved configuration.
    fn list_plugin_configs(&self) -> Result<Vec<String>, ConfigError>;

    // ========================================================================
    // Generic Key-Value (for extensibility)
    // ========================================================================

    /// Load a value by key.
    fn load_value<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError>;

    /// Save a value by key.
    fn save_value<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ConfigError>;

    /// Check if a key exists.
    fn has_key(&self, key: &str) -> bool;

    /// Delete a key.
    fn delete_key(&self, key: &str) -> Result<(), ConfigError>;
}

// ============================================================================
// Configuration Types (shared across platforms)
// ============================================================================

/// Server settings matching TypeScript implementation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettings {
    /// Interface enable/disable flags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<InterfaceSettings>,

    /// HTTP port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// HTTPS port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sslport: Option<u16>,

    /// Enable SSL/TLS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl: Option<bool>,

    /// Enable WebSocket compression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_compression: Option<bool>,

    /// Enable mDNS discovery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mdns: Option<bool>,

    /// Minutes before pruning inactive contexts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune_contexts_minutes: Option<u32>,

    /// Enable access logging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_logging: Option<bool>,

    /// Log file directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging_directory: Option<String>,

    /// Keep only recent logs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_most_recent_logs_only: Option<bool>,

    /// Number of log files to retain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_count_to_keep: Option<u32>,

    /// Enable plugin logging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_plugin_logging: Option<bool>,
}

/// Interface enable/disable settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rest: Option<bool>,

    #[serde(rename = "signalk-ws", skip_serializing_if = "Option::is_none")]
    pub signalk_ws: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub appstore: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub webapps: Option<bool>,
}

/// Vessel information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VesselInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mmsi: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub callsign: Option<String>,
}

/// Security configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    /// Allow read-only access without authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_read_only: Option<bool>,

    /// Token expiration (e.g., "1d", "7d").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<String>,

    /// Allow new user self-registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_new_user_registration: Option<bool>,

    /// Allow device access requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_device_access_requests: Option<bool>,

    /// Registered users.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<UserRecord>>,

    /// Authorized devices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devices: Option<Vec<DeviceRecord>>,
}

/// User record in security configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRecord {
    pub user_id: String,

    #[serde(rename = "type")]
    pub user_type: String,

    /// Password hash (never serialized to clients).
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
}

/// Device record in security configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRecord {
    pub client_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub permissions: String,
}

// ============================================================================
// Handler Logic (framework-agnostic)
// ============================================================================

/// Configuration handler logic that can be used by any HTTP framework.
///
/// These functions contain the business logic for configuration endpoints.
/// Framework-specific code (Axum, esp-idf-http) wraps these with their
/// request/response types.
pub struct ConfigHandlers;

impl ConfigHandlers {
    /// Get server settings.
    pub fn get_settings<S: ConfigStorage>(storage: &S) -> Result<ServerSettings, ConfigError> {
        storage.load_settings()
    }

    /// Update server settings.
    pub fn put_settings<S: ConfigStorage>(
        storage: &S,
        settings: ServerSettings,
    ) -> Result<(), ConfigError> {
        storage.save_settings(&settings)
    }

    /// Get vessel information.
    pub fn get_vessel<S: ConfigStorage>(storage: &S) -> Result<VesselInfo, ConfigError> {
        storage.load_vessel()
    }

    /// Update vessel information.
    pub fn put_vessel<S: ConfigStorage>(
        storage: &S,
        vessel: VesselInfo,
    ) -> Result<(), ConfigError> {
        storage.save_vessel(&vessel)
    }

    /// Get security configuration (without sensitive data).
    pub fn get_security_config<S: ConfigStorage>(
        storage: &S,
    ) -> Result<SecurityConfig, ConfigError> {
        storage.load_security()
    }

    /// Get list of users (without passwords).
    pub fn get_users<S: ConfigStorage>(storage: &S) -> Result<Vec<UserRecord>, ConfigError> {
        let config = storage.load_security()?;
        Ok(config.users.unwrap_or_default())
    }

    /// Get plugin configuration.
    pub fn get_plugin_config<S: ConfigStorage>(
        storage: &S,
        plugin_id: &str,
    ) -> Result<serde_json::Value, ConfigError> {
        storage.load_plugin_config(plugin_id)
    }

    /// Save plugin configuration.
    pub fn put_plugin_config<S: ConfigStorage>(
        storage: &S,
        plugin_id: &str,
        config: serde_json::Value,
    ) -> Result<(), ConfigError> {
        storage.save_plugin_config(plugin_id, &config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::RwLock;

    /// In-memory storage for testing.
    struct MemoryConfigStorage {
        data: RwLock<HashMap<String, String>>,
    }

    impl MemoryConfigStorage {
        fn new() -> Self {
            Self {
                data: RwLock::new(HashMap::new()),
            }
        }
    }

    impl ConfigStorage for MemoryConfigStorage {
        fn load_settings(&self) -> Result<ServerSettings, ConfigError> {
            self.load_value("settings")
        }

        fn save_settings(&self, settings: &ServerSettings) -> Result<(), ConfigError> {
            self.save_value("settings", settings)
        }

        fn load_vessel(&self) -> Result<VesselInfo, ConfigError> {
            self.load_value("vessel")
        }

        fn save_vessel(&self, vessel: &VesselInfo) -> Result<(), ConfigError> {
            self.save_value("vessel", vessel)
        }

        fn load_security(&self) -> Result<SecurityConfig, ConfigError> {
            self.load_value("security")
        }

        fn save_security(&self, config: &SecurityConfig) -> Result<(), ConfigError> {
            self.save_value("security", config)
        }

        fn load_plugin_config(&self, plugin_id: &str) -> Result<serde_json::Value, ConfigError> {
            self.load_value(&format!("plugin:{}", plugin_id))
        }

        fn save_plugin_config(
            &self,
            plugin_id: &str,
            config: &serde_json::Value,
        ) -> Result<(), ConfigError> {
            self.save_value(&format!("plugin:{}", plugin_id), config)
        }

        fn list_plugin_configs(&self) -> Result<Vec<String>, ConfigError> {
            let data = self.data.read().unwrap();
            Ok(data
                .keys()
                .filter_map(|k| k.strip_prefix("plugin:").map(String::from))
                .collect())
        }

        fn load_value<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
            let data = self.data.read().unwrap();
            let json = data
                .get(key)
                .ok_or_else(|| ConfigError::NotFound(key.to_string()))?;
            serde_json::from_str(json).map_err(|e| ConfigError::InvalidData(e.to_string()))
        }

        fn save_value<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ConfigError> {
            let json =
                serde_json::to_string(value).map_err(|e| ConfigError::WriteError(e.to_string()))?;
            self.data.write().unwrap().insert(key.to_string(), json);
            Ok(())
        }

        fn has_key(&self, key: &str) -> bool {
            self.data.read().unwrap().contains_key(key)
        }

        fn delete_key(&self, key: &str) -> Result<(), ConfigError> {
            self.data.write().unwrap().remove(key);
            Ok(())
        }
    }

    #[test]
    fn test_settings_round_trip() {
        let storage = MemoryConfigStorage::new();

        let settings = ServerSettings {
            port: Some(3000),
            mdns: Some(true),
            ..Default::default()
        };

        ConfigHandlers::put_settings(&storage, settings.clone()).unwrap();
        let loaded = ConfigHandlers::get_settings(&storage).unwrap();

        assert_eq!(loaded.port, Some(3000));
        assert_eq!(loaded.mdns, Some(true));
    }

    #[test]
    fn test_vessel_round_trip() {
        let storage = MemoryConfigStorage::new();

        let vessel = VesselInfo {
            name: Some("Test Vessel".to_string()),
            mmsi: Some("123456789".to_string()),
            ..Default::default()
        };

        ConfigHandlers::put_vessel(&storage, vessel).unwrap();
        let loaded = ConfigHandlers::get_vessel(&storage).unwrap();

        assert_eq!(loaded.name, Some("Test Vessel".to_string()));
        assert_eq!(loaded.mmsi, Some("123456789".to_string()));
    }

    #[test]
    fn test_plugin_config() {
        let storage = MemoryConfigStorage::new();

        let config = serde_json::json!({
            "enabled": true,
            "updateRate": 1000
        });

        ConfigHandlers::put_plugin_config(&storage, "my-plugin", config.clone()).unwrap();
        let loaded = ConfigHandlers::get_plugin_config(&storage, "my-plugin").unwrap();

        assert_eq!(loaded["enabled"], true);
        assert_eq!(loaded["updateRate"], 1000);
    }
}
