//! HTTP handler utilities for ESP32 SignalK server.
//!
//! Provides helper functions for building SignalK-compliant HTTP responses
//! and WebSocket connection management.

use signalk_core::{MemoryStore, PathPattern, SignalKStore};
use signalk_protocol::{ClientMessage, DiscoveryResponse, HelloMessage, ServerMessage};
use std::sync::{Arc, Mutex};

// ============================================================================
// WebSocket Query Parameters
// ============================================================================

/// Initial subscription mode from query parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubscribeMode {
    /// Subscribe to self vessel only (default).
    #[default]
    Self_,
    /// Subscribe to all vessels.
    All,
    /// No initial subscription.
    None,
}

impl SubscribeMode {
    /// Parse from query string value.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "all" => Self::All,
            "none" => Self::None,
            _ => Self::Self_,
        }
    }
}

/// Parsed WebSocket query parameters.
#[derive(Debug, Clone)]
pub struct WsQueryParams {
    /// Initial subscription mode (default: self).
    pub subscribe: SubscribeMode,
    /// Whether to send cached values on connect (default: true).
    pub send_cached_values: bool,
}

impl Default for WsQueryParams {
    fn default() -> Self {
        Self {
            subscribe: SubscribeMode::Self_,
            send_cached_values: true,
        }
    }
}

impl WsQueryParams {
    /// Parse query parameters from a URI query string.
    ///
    /// Example: "subscribe=all&sendCachedValues=false"
    pub fn parse(query: &str) -> Self {
        let mut params = Self::default();

        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                match key {
                    "subscribe" => params.subscribe = SubscribeMode::from_str(value),
                    "sendCachedValues" => params.send_cached_values = value != "false",
                    _ => {} // Ignore unknown params (serverevents, sendMeta, etc.)
                }
            }
        }

        params
    }
}

// ============================================================================
// Client Subscription State
// ============================================================================

/// Per-client subscription state for delta filtering.
#[derive(Debug)]
pub struct ClientSubscription {
    /// Context filter (e.g., "vessels.self", "*").
    pub context: Option<String>,
    /// Path patterns to match.
    pub patterns: Vec<PathPattern>,
}

impl ClientSubscription {
    /// Create a new subscription.
    pub fn new(context: Option<String>, patterns: Vec<PathPattern>) -> Self {
        Self { context, patterns }
    }

    /// Check if a path matches any of the subscription patterns.
    pub fn matches_path(&self, path: &str) -> bool {
        // Empty patterns = not subscribed to anything
        if self.patterns.is_empty() {
            return false;
        }
        self.patterns.iter().any(|p| p.matches(path))
    }

    /// Check if a context matches the subscription filter.
    pub fn matches_context(&self, delta_context: Option<&str>) -> bool {
        match &self.context {
            None => false, // No context = not subscribed
            Some(ctx) if ctx == "*" => true,
            Some(ctx) if ctx == "vessels.self" => {
                // Match "vessels.self" or the actual vessel URN
                delta_context == Some("vessels.self")
                    || delta_context.map_or(false, |c| c.starts_with("vessels.urn:"))
            }
            Some(ctx) => delta_context == Some(ctx.as_str()),
        }
    }

    /// Check if the subscription is empty (no subscriptions).
    pub fn is_empty(&self) -> bool {
        self.context.is_none() && self.patterns.is_empty()
    }
}

impl Default for ClientSubscription {
    fn default() -> Self {
        Self {
            context: None,
            patterns: Vec::new(),
        }
    }
}

/// Create default subscription based on query parameter mode.
pub fn default_subscription_for_mode(mode: SubscribeMode) -> ClientSubscription {
    match mode {
        SubscribeMode::Self_ => ClientSubscription {
            context: Some("vessels.self".to_string()),
            patterns: vec![PathPattern::new("*").unwrap()],
        },
        SubscribeMode::All => ClientSubscription {
            context: Some("*".to_string()),
            patterns: vec![PathPattern::new("*").unwrap()],
        },
        SubscribeMode::None => ClientSubscription {
            context: None,
            patterns: Vec::new(), // Empty = no matches until subscribe message
        },
    }
}

// ============================================================================
// Client Message Handling
// ============================================================================

/// Process a client message and return updated subscription state.
///
/// Returns Some(subscription) if the message updates subscriptions, None otherwise.
pub fn process_client_message(
    message: &str,
    current: &ClientSubscription,
) -> Option<ClientSubscription> {
    let msg: ClientMessage = serde_json::from_str(message).ok()?;

    match msg {
        ClientMessage::Subscribe(req) => {
            let mut patterns = current.patterns.clone();

            for sub in req.subscribe {
                if let Ok(pattern) = PathPattern::new(&sub.path) {
                    // Avoid duplicates
                    if !patterns.iter().any(|p| p.as_str() == pattern.as_str()) {
                        patterns.push(pattern);
                    }
                }
            }

            Some(ClientSubscription {
                context: Some(req.context),
                patterns,
            })
        }
        ClientMessage::Unsubscribe(req) => {
            let mut patterns = current.patterns.clone();

            for unsub in req.unsubscribe {
                if unsub.path == "*" {
                    // Unsubscribe from all
                    patterns.clear();
                } else {
                    // Remove matching pattern
                    patterns.retain(|p| p.as_str() != unsub.path);
                }
            }

            Some(ClientSubscription {
                context: if req.context == "*" {
                    None
                } else {
                    Some(req.context)
                },
                patterns,
            })
        }
        ClientMessage::Put(_) => {
            // PUT requests don't affect subscriptions
            None
        }
    }
}

// ============================================================================
// Hello and Discovery Helpers
// ============================================================================

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
