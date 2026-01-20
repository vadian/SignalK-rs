//! ESP32-specific components for SignalK server.
//!
//! This crate provides reusable components for ESP32-based SignalK implementations:
//! - WiFi connection management
//! - NVS (Non-Volatile Storage) configuration
//! - HTTP/WebSocket handler utilities
//!
//! # Architecture
//!
//! This crate is designed to be shared across different ESP32 binary targets
//! (e.g., different board variants, different feature sets). The main binary
//! (`signalk-server-esp32`) imports this crate and uses its components.
//!
//! # Example
//!
//! ```ignore
//! use signalk_esp32::wifi::connect_wifi;
//! use signalk_esp32::config::NvsConfig;
//!
//! // Connect to WiFi
//! let wifi = connect_wifi("ssid", "password", modem, sysloop)?;
//!
//! // Load configuration from NVS
//! let config = NvsConfig::load()?;
//! ```

pub mod wifi;
pub mod config;
pub mod http;
