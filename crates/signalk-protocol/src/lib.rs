//! # signalk-protocol
//!
//! SignalK protocol message types and codec.
//!
//! This crate defines the WebSocket and REST API message formats.
//!
//! ## Message Types
//!
//! - [`HelloMessage`] - Server hello on connection
//! - [`ServerMessage`] - Messages from server to client
//! - [`ClientMessage`] - Messages from client to server
//! - [`DiscoveryResponse`] - `/signalk` endpoint response
//!
//! ## Codec
//!
//! The [`codec`] module provides encoding/decoding utilities for
//! WebSocket JSON messages.

pub mod codec;
pub mod messages;

pub use codec::*;
pub use messages::*;
