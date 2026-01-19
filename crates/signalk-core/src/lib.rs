//! # signalk-core
//!
//! Core SignalK data model and store implementation.
//!
//! This crate provides:
//! - Data model types (Delta, Update, Value, Source, etc.)
//! - Path parsing and wildcard matching
//! - In-memory store implementation
//! - Subscription logic (without I/O)
//! - Configuration storage abstraction
//!
//! This crate is intentionally runtime-agnostic and contains no async code,
//! making it usable on both Linux (tokio) and ESP32 (esp-idf) targets.

pub mod config;
pub mod model;
pub mod path;
pub mod store;

pub use config::{
    ConfigError, ConfigHandlers, ConfigStorage, InterfaceSettings, SecurityConfig, ServerSettings,
    VesselInfo,
};
pub use model::*;
pub use path::{Path, PathPattern, PatternError};
pub use store::{MemoryStore, SignalKStore};
