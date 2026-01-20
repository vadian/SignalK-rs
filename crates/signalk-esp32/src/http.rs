//! HTTP handler utilities for ESP32 SignalK server.
//!
//! Provides helper functions for building SignalK-compliant HTTP responses
//! and WebSocket connection management.

use signalk_core::{MemoryStore, PathPattern, SignalKStore};
use signalk_protocol::{ClientMessage, DiscoveryResponse, HelloMessage, ServerMessage};
use std::sync::{Arc, Mutex};
use std::time::Instant;

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
// Throttling Support
// ============================================================================

/// Default minimum period between updates for a path (milliseconds).
/// If a subscription specifies minPeriod, this is the floor.
pub const DEFAULT_MIN_PERIOD_MS: u64 = 0;

/// A path pattern with throttling state.
///
/// Tracks when data was last sent for this pattern to enforce rate limiting.
/// Uses `std::time::Instant` which is available on ESP32 via esp-idf.
#[derive(Debug)]
pub struct ThrottledPattern {
    /// The path pattern to match.
    pattern: PathPattern,
    /// Minimum period between updates (milliseconds). 0 = no throttling.
    min_period_ms: u64,
    /// Desired period between updates (milliseconds). 0 = as fast as possible.
    period_ms: u64,
    /// Last time this pattern was sent to the client.
    last_sent: Option<Instant>,
}

impl ThrottledPattern {
    /// Create a new throttled pattern.
    pub fn new(pattern: PathPattern, period_ms: u64, min_period_ms: u64) -> Self {
        Self {
            pattern,
            min_period_ms,
            period_ms,
            last_sent: None,
        }
    }

    /// Create a throttled pattern with no throttling (instant updates).
    pub fn instant(pattern: PathPattern) -> Self {
        Self::new(pattern, 0, 0)
    }

    /// Get the underlying pattern string.
    pub fn as_str(&self) -> &str {
        self.pattern.as_str()
    }

    /// Check if this pattern matches a path.
    pub fn matches(&self, path: &str) -> bool {
        self.pattern.matches(path)
    }

    /// Check if enough time has passed to send an update.
    ///
    /// Returns true if:
    /// - No throttling is configured (min_period_ms == 0)
    /// - This is the first update (last_sent is None)
    /// - Enough time has elapsed since last send
    pub fn should_send(&self) -> bool {
        // No throttling configured
        if self.min_period_ms == 0 {
            return true;
        }

        // First update
        let Some(last) = self.last_sent else {
            return true;
        };

        // Check if enough time has passed
        let elapsed = last.elapsed().as_millis() as u64;
        elapsed >= self.min_period_ms
    }

    /// Mark this pattern as having been sent now.
    pub fn mark_sent(&mut self) {
        self.last_sent = Some(Instant::now());
    }

    /// Get the minimum period in milliseconds.
    pub fn min_period_ms(&self) -> u64 {
        self.min_period_ms
    }

    /// Get the desired period in milliseconds.
    pub fn period_ms(&self) -> u64 {
        self.period_ms
    }
}

// ============================================================================
// Client Subscription State
// ============================================================================

/// Per-client subscription state for delta filtering with throttling support.
#[derive(Debug)]
pub struct ClientSubscription {
    /// Context filter (e.g., "vessels.self", "*").
    pub context: Option<String>,
    /// Path patterns with throttling state.
    pub patterns: Vec<ThrottledPattern>,
}

impl ClientSubscription {
    /// Create a new subscription with throttled patterns.
    pub fn new_throttled(context: Option<String>, patterns: Vec<ThrottledPattern>) -> Self {
        Self { context, patterns }
    }

    /// Create a new subscription with simple patterns (no throttling).
    pub fn new(context: Option<String>, patterns: Vec<PathPattern>) -> Self {
        let throttled = patterns
            .into_iter()
            .map(ThrottledPattern::instant)
            .collect();
        Self {
            context,
            patterns: throttled,
        }
    }

    /// Check if a path matches any of the subscription patterns.
    pub fn matches_path(&self, path: &str) -> bool {
        // Empty patterns = not subscribed to anything
        if self.patterns.is_empty() {
            return false;
        }
        self.patterns.iter().any(|p| p.matches(path))
    }

    /// Check if a path matches and should be sent (passes throttle check).
    ///
    /// Returns the index of the matching pattern if found and ready to send.
    pub fn should_send_path(&self, path: &str) -> Option<usize> {
        for (i, p) in self.patterns.iter().enumerate() {
            if p.matches(path) && p.should_send() {
                return Some(i);
            }
        }
        None
    }

    /// Mark a pattern as sent by index.
    pub fn mark_sent(&mut self, index: usize) {
        if let Some(p) = self.patterns.get_mut(index) {
            p.mark_sent();
        }
    }

    /// Get the number of patterns.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
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
///
/// Default subscriptions have no throttling (instant updates).
pub fn default_subscription_for_mode(mode: SubscribeMode) -> ClientSubscription {
    match mode {
        SubscribeMode::Self_ => ClientSubscription::new(
            Some("vessels.self".to_string()),
            vec![PathPattern::new("*").unwrap()],
        ),
        SubscribeMode::All => ClientSubscription::new(
            Some("*".to_string()),
            vec![PathPattern::new("*").unwrap()],
        ),
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
/// Captures period and minPeriod from subscribe messages for throttling.
pub fn process_client_message(
    message: &str,
    current: &ClientSubscription,
) -> Option<ClientSubscription> {
    let msg: ClientMessage = serde_json::from_str(message).ok()?;

    match msg {
        ClientMessage::Subscribe(req) => {
            let mut patterns: Vec<ThrottledPattern> = Vec::new();

            // Copy existing patterns (preserving their throttle state)
            for existing in &current.patterns {
                // Only copy if not being replaced by new subscription
                let path = existing.as_str();
                let is_being_replaced = req.subscribe.iter().any(|s| s.path == path);
                if !is_being_replaced {
                    // Create new ThrottledPattern with same settings but reset timer
                    if let Ok(pattern) = PathPattern::new(path) {
                        patterns.push(ThrottledPattern::new(
                            pattern,
                            existing.period_ms(),
                            existing.min_period_ms(),
                        ));
                    }
                }
            }

            // Add new subscriptions with throttling parameters
            for sub in req.subscribe {
                if let Ok(pattern) = PathPattern::new(&sub.path) {
                    // Avoid duplicates
                    if !patterns.iter().any(|p| p.as_str() == pattern.as_str()) {
                        // Use period/minPeriod from subscription, defaulting to 0 (instant)
                        let period_ms = sub.period.unwrap_or(0);
                        let min_period_ms = sub.min_period.unwrap_or(DEFAULT_MIN_PERIOD_MS);
                        patterns.push(ThrottledPattern::new(pattern, period_ms, min_period_ms));
                    }
                }
            }

            Some(ClientSubscription::new_throttled(Some(req.context), patterns))
        }
        ClientMessage::Unsubscribe(req) => {
            let mut patterns: Vec<ThrottledPattern> = Vec::new();

            for existing in &current.patterns {
                let path = existing.as_str();
                let should_remove = req.unsubscribe.iter().any(|u| u.path == "*" || u.path == path);
                if !should_remove {
                    // Keep this pattern
                    if let Ok(pattern) = PathPattern::new(path) {
                        patterns.push(ThrottledPattern::new(
                            pattern,
                            existing.period_ms(),
                            existing.min_period_ms(),
                        ));
                    }
                }
            }

            Some(ClientSubscription::new_throttled(
                if req.context == "*" {
                    None
                } else {
                    Some(req.context)
                },
                patterns,
            ))
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
