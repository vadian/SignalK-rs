//! Subscription management for SignalK clients.
//!
//! This module handles per-client subscriptions, filtering deltas
//! based on subscribed paths and contexts.

use signalk_core::{Delta, MemoryStore, PathPattern, PathValue, SignalKStore, Update};
use signalk_protocol::{Subscription, SubscriptionPolicy};

/// Represents a client's subscription to a specific path pattern.
#[derive(Debug, Clone)]
pub struct ClientSubscription {
    /// Context pattern (e.g., "vessels.self", "vessels.*", "*")
    pub context: String,
    /// Path pattern (e.g., "navigation.*", "environment.wind.*")
    pub path: String,
    /// Subscription period in milliseconds (for rate limiting)
    pub period: Option<u64>,
    /// Minimum period for throttling
    pub min_period: Option<u64>,
    /// Subscription policy
    pub policy: SubscriptionPolicy,
    /// Compiled path pattern for efficiency
    matcher: PathPattern,
}

impl ClientSubscription {
    /// Create a new subscription.
    pub fn new(context: &str, path: &str) -> Self {
        Self {
            context: context.to_string(),
            path: path.to_string(),
            period: None,
            min_period: None,
            policy: SubscriptionPolicy::Instant,
            matcher: PathPattern::new(path).expect("Invalid path pattern"),
        }
    }

    /// Create from a protocol Subscription.
    pub fn from_protocol(context: &str, sub: &Subscription) -> Self {
        Self {
            context: context.to_string(),
            path: sub.path.clone(),
            period: sub.period,
            min_period: sub.min_period,
            policy: sub.policy.clone().unwrap_or(SubscriptionPolicy::Instant),
            matcher: PathPattern::new(&sub.path).expect("Invalid path pattern"),
        }
    }

    /// Check if this subscription matches a given context and path.
    pub fn matches(&self, context: &str, path: &str) -> bool {
        self.matches_context(context) && self.matcher.matches(path)
    }

    /// Check if the context matches.
    fn matches_context(&self, context: &str) -> bool {
        if self.context == "*" {
            return true;
        }
        if self.context == "vessels.self" {
            // Match both "vessels.self" and the actual self URN
            return context == "vessels.self" || context.starts_with("vessels.urn:");
        }
        self.context == context
    }
}

/// Manages subscriptions for a single client connection.
pub struct SubscriptionManager {
    /// The self URN for this server.
    self_urn: String,
    /// Active subscriptions.
    subscriptions: Vec<ClientSubscription>,
}

impl SubscriptionManager {
    /// Create a new subscription manager.
    pub fn new(self_urn: &str) -> Self {
        Self {
            self_urn: self_urn.to_string(),
            subscriptions: Vec::new(),
        }
    }

    /// Subscribe to all paths for the self vessel (default subscription).
    pub fn subscribe_self_all(&mut self) {
        self.subscriptions
            .push(ClientSubscription::new("vessels.self", "*"));
    }

    /// Subscribe to nothing (clear all subscriptions).
    pub fn subscribe_none(&mut self) {
        self.subscriptions.clear();
    }

    /// Subscribe to all contexts and paths.
    pub fn subscribe_all(&mut self) {
        self.subscriptions.clear();
        self.subscriptions.push(ClientSubscription::new("*", "*"));
    }

    /// Add subscriptions from a subscribe request.
    ///
    /// Returns a list of warning messages for inconsistent subscription parameters
    /// (e.g., minPeriod with non-instant policy).
    pub fn add_subscriptions(&mut self, context: &str, subs: &[Subscription]) -> Vec<String> {
        let mut warnings = Vec::new();

        for sub in subs {
            // Check for inconsistent subscription parameters
            if let Some(min_period) = sub.min_period {
                if min_period > 0 {
                    if let Some(ref policy) = sub.policy {
                        if *policy != SubscriptionPolicy::Instant {
                            warnings.push(format!(
                                "minPeriod assumes policy 'instant', ignoring policy {policy:?}"
                            ));
                        }
                    }
                }
            }

            if let Some(period) = sub.period {
                if period > 0 && sub.min_period.is_none() {
                    if let Some(ref policy) = sub.policy {
                        if *policy != SubscriptionPolicy::Fixed {
                            warnings.push(format!(
                                "period assumes policy 'fixed', ignoring policy {policy:?}"
                            ));
                        }
                    }
                }
            }

            self.subscriptions
                .push(ClientSubscription::from_protocol(context, sub));
        }

        warnings
    }

    /// Remove a subscription by context and path.
    pub fn remove_subscription(&mut self, context: &str, path: &str) {
        if path == "*" && context == "*" {
            // Unsubscribe from everything
            self.subscriptions.clear();
        } else {
            self.subscriptions
                .retain(|s| !(s.context == context && s.path == path));
        }
    }

    /// Check if any subscription matches a given context and path.
    pub fn matches(&self, context: &str, path: &str) -> bool {
        self.subscriptions.iter().any(|s| s.matches(context, path))
    }

    /// Filter a delta to only include paths the client is subscribed to.
    ///
    /// Returns None if no paths match any subscription.
    pub fn filter_delta(&self, delta: &Delta) -> Option<Delta> {
        let context = delta.context.as_deref().unwrap_or("vessels.self");

        // Check if any subscription could match this context
        if !self
            .subscriptions
            .iter()
            .any(|s| s.matches_context(context))
        {
            return None;
        }

        // Filter updates to only include matching paths
        let filtered_updates: Vec<Update> = delta
            .updates
            .iter()
            .filter_map(|update| {
                let filtered_values: Vec<PathValue> = update
                    .values
                    .iter()
                    .filter(|pv| self.matches(context, &pv.path))
                    .cloned()
                    .collect();

                if filtered_values.is_empty() {
                    None
                } else {
                    Some(Update {
                        source_ref: update.source_ref.clone(),
                        source: update.source.clone(),
                        timestamp: update.timestamp.clone(),
                        values: filtered_values,
                        meta: update.meta.clone(),
                    })
                }
            })
            .collect();

        if filtered_updates.is_empty() {
            None
        } else {
            Some(Delta {
                context: delta.context.clone(),
                updates: filtered_updates,
            })
        }
    }

    /// Get an initial delta with all current values matching subscriptions.
    ///
    /// This is sent when a client first connects with `sendCachedValues=true`.
    /// Returns None if there are no cached values to send.
    pub fn get_initial_delta(&self, store: &MemoryStore) -> Option<Delta> {
        if self.subscriptions.is_empty() {
            return None;
        }

        // Collect values from the store that match our subscriptions
        let mut path_values = Vec::new();
        let mut source_ref: Option<String> = None;
        let mut timestamp: Option<String> = None;

        // Get the self vessel data from the store
        let self_urn = store.self_urn();
        let urn_key = self_urn.strip_prefix("vessels.").unwrap_or(self_urn);

        if let Some(vessel_data) = store
            .full_model()
            .get("vessels")
            .and_then(|v| v.get(urn_key))
        {
            self.collect_matching_paths(
                vessel_data,
                "",
                "vessels.self",
                &mut path_values,
                &mut source_ref,
                &mut timestamp,
            );
        }

        if path_values.is_empty() {
            return None;
        }

        Some(Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref,
                source: None,
                timestamp,
                values: path_values,
                meta: None,
            }],
        })
    }

    /// Recursively collect paths and values from a JSON object that match subscriptions.
    fn collect_matching_paths(
        &self,
        value: &serde_json::Value,
        current_path: &str,
        context: &str,
        path_values: &mut Vec<PathValue>,
        source_ref: &mut Option<String>,
        timestamp: &mut Option<String>,
    ) {
        if let serde_json::Value::Object(map) = value {
            // Check if this is a leaf value node (has "value" key)
            if map.contains_key("value") {
                // This is a SignalK value node
                if self.matches(context, current_path) {
                    path_values.push(PathValue {
                        path: current_path.to_string(),
                        value: map.get("value").cloned().unwrap_or(serde_json::Value::Null),
                    });

                    // Capture source and timestamp from the first matching value
                    if source_ref.is_none() {
                        if let Some(src) = map.get("$source").and_then(|s| s.as_str()) {
                            *source_ref = Some(src.to_string());
                        }
                    }
                    if timestamp.is_none() {
                        if let Some(ts) = map.get("timestamp").and_then(|t| t.as_str()) {
                            *timestamp = Some(ts.to_string());
                        }
                    }
                }
            } else {
                // Recurse into child objects
                for (key, child) in map {
                    // Skip "values" map - we only want the primary value
                    if key == "values" {
                        continue;
                    }

                    let child_path = if current_path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{current_path}.{key}")
                    };

                    self.collect_matching_paths(
                        child,
                        &child_path,
                        context,
                        path_values,
                        source_ref,
                        timestamp,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_matching() {
        let sub = ClientSubscription::new("vessels.self", "navigation.*");

        assert!(sub.matches("vessels.self", "navigation.speedOverGround"));
        assert!(sub.matches("vessels.self", "navigation.position"));
        assert!(!sub.matches("vessels.self", "environment.wind.speedApparent"));
        assert!(!sub.matches("vessels.other", "navigation.speedOverGround"));
    }

    #[test]
    fn test_wildcard_context() {
        let sub = ClientSubscription::new("*", "navigation.position");

        assert!(sub.matches("vessels.self", "navigation.position"));
        assert!(sub.matches("vessels.urn:mrn:test", "navigation.position"));
        assert!(!sub.matches("vessels.self", "navigation.speedOverGround"));
    }

    #[test]
    fn test_subscription_manager() {
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");

        // Default: subscribe to nothing
        assert!(!mgr.matches("vessels.self", "navigation.position"));

        // Subscribe to self
        mgr.subscribe_self_all();
        assert!(mgr.matches("vessels.self", "navigation.position"));
        assert!(mgr.matches("vessels.self", "environment.wind.speedApparent"));

        // Subscribe to specific path
        mgr.subscribe_none();
        mgr.add_subscriptions(
            "vessels.self",
            &[Subscription {
                path: "navigation.*".to_string(),
                period: Some(1000),
                format: None,
                policy: None,
                min_period: None,
            }],
        );

        assert!(mgr.matches("vessels.self", "navigation.position"));
        assert!(!mgr.matches("vessels.self", "environment.wind.speedApparent"));
    }

    #[test]
    fn test_filter_delta() {
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.add_subscriptions(
            "vessels.self",
            &[Subscription {
                path: "navigation.*".to_string(),
                period: None,
                format: None,
                policy: None,
                min_period: None,
            }],
        );

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: Some("2024-01-01T00:00:00Z".to_string()),
                values: vec![
                    PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: serde_json::json!(3.5),
                    },
                    PathValue {
                        path: "environment.wind.speedApparent".to_string(),
                        value: serde_json::json!(5.0),
                    },
                ],
                meta: None,
            }],
        };

        let filtered = mgr.filter_delta(&delta).unwrap();
        assert_eq!(filtered.updates.len(), 1);
        assert_eq!(filtered.updates[0].values.len(), 1);
        assert_eq!(
            filtered.updates[0].values[0].path,
            "navigation.speedOverGround"
        );
    }

    #[test]
    fn test_subscription_with_period() {
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.add_subscriptions(
            "vessels.self",
            &[Subscription {
                path: "navigation.*".to_string(),
                period: Some(1000),
                format: None,
                policy: Some(SubscriptionPolicy::Instant),
                min_period: Some(100),
            }],
        );

        // Verify subscription was added
        assert!(mgr.matches("vessels.self", "navigation.speedOverGround"));
    }

    #[test]
    fn test_unsubscribe_specific_path() {
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");

        mgr.add_subscriptions(
            "vessels.self",
            &[
                Subscription {
                    path: "navigation.*".to_string(),
                    period: None,
                    format: None,
                    policy: None,
                    min_period: None,
                },
                Subscription {
                    path: "environment.*".to_string(),
                    period: None,
                    format: None,
                    policy: None,
                    min_period: None,
                },
            ],
        );

        assert!(mgr.matches("vessels.self", "navigation.speedOverGround"));
        assert!(mgr.matches("vessels.self", "environment.wind.speedApparent"));

        // Unsubscribe from navigation only
        mgr.remove_subscription("vessels.self", "navigation.*");

        assert!(!mgr.matches("vessels.self", "navigation.speedOverGround"));
        assert!(mgr.matches("vessels.self", "environment.wind.speedApparent"));
    }

    #[test]
    fn test_filter_delta_no_match() {
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.add_subscriptions(
            "vessels.self",
            &[Subscription {
                path: "navigation.*".to_string(),
                period: None,
                format: None,
                policy: None,
                min_period: None,
            }],
        );

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: Some("2024-01-01T00:00:00Z".to_string()),
                values: vec![PathValue {
                    path: "environment.wind.speedApparent".to_string(),
                    value: serde_json::json!(5.0),
                }],
                meta: None,
            }],
        };

        let filtered = mgr.filter_delta(&delta);
        assert!(filtered.is_none());
    }

    #[test]
    fn test_filter_preserves_metadata() {
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.add_subscriptions(
            "vessels.self",
            &[Subscription {
                path: "navigation.*".to_string(),
                period: None,
                format: None,
                policy: None,
                min_period: None,
            }],
        );

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps".to_string()),
                source: None,
                timestamp: Some("2024-01-01T00:00:00Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.5),
                }],
                meta: None,
            }],
        };

        let filtered = mgr.filter_delta(&delta).unwrap();
        assert_eq!(filtered.updates[0].source_ref, Some("gps".to_string()));
        assert_eq!(
            filtered.updates[0].timestamp,
            Some("2024-01-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn test_multiple_matching_subscriptions() {
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");

        // Add overlapping subscriptions
        mgr.add_subscriptions(
            "vessels.self",
            &[
                Subscription {
                    path: "navigation.*".to_string(),
                    period: None,
                    format: None,
                    policy: None,
                    min_period: None,
                },
                Subscription {
                    path: "navigation.speedOverGround".to_string(),
                    period: None,
                    format: None,
                    policy: None,
                    min_period: None,
                },
            ],
        );

        // Should match (via either subscription)
        assert!(mgr.matches("vessels.self", "navigation.speedOverGround"));
    }

    #[test]
    fn test_context_resolution_with_urn() {
        let sub = ClientSubscription::new("vessels.self", "navigation.*");

        // Should match actual URN as well as "vessels.self"
        assert!(sub.matches("vessels.self", "navigation.speedOverGround"));
        assert!(sub.matches(
            "vessels.urn:mrn:signalk:uuid:test",
            "navigation.speedOverGround"
        ));
    }

    #[test]
    fn test_wildcard_all_contexts() {
        let sub = ClientSubscription::new("*", "*");

        assert!(sub.matches("vessels.self", "navigation.speedOverGround"));
        assert!(sub.matches("vessels.urn:mrn:test", "environment.wind.speedApparent"));
        assert!(sub.matches("aircraft.self", "navigation.position"));
    }

    #[test]
    fn test_filter_multiple_updates() {
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.add_subscriptions(
            "vessels.self",
            &[Subscription {
                path: "navigation.*".to_string(),
                period: None,
                format: None,
                policy: None,
                min_period: None,
            }],
        );

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![
                Update {
                    source_ref: Some("gps".to_string()),
                    source: None,
                    timestamp: Some("2024-01-01T00:00:00Z".to_string()),
                    values: vec![PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: serde_json::json!(3.5),
                    }],
                    meta: None,
                },
                Update {
                    source_ref: Some("wind".to_string()),
                    source: None,
                    timestamp: Some("2024-01-01T00:00:01Z".to_string()),
                    values: vec![PathValue {
                        path: "environment.wind.speedApparent".to_string(),
                        value: serde_json::json!(10.0),
                    }],
                    meta: None,
                },
            ],
        };

        let filtered = mgr.filter_delta(&delta).unwrap();
        // Should only have one update (navigation)
        assert_eq!(filtered.updates.len(), 1);
        assert_eq!(filtered.updates[0].source_ref, Some("gps".to_string()));
    }

    // ============================================================
    // Tests for get_initial_delta (sendCachedValues support)
    // ============================================================

    #[test]
    fn test_get_initial_delta_empty_store() {
        let store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test");
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.subscribe_self_all();

        // Empty store should return None
        let initial = mgr.get_initial_delta(&store);
        assert!(initial.is_none());
    }

    #[test]
    fn test_get_initial_delta_no_subscriptions() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test");
        let mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");

        // Add some data to the store
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: Some("2024-01-01T00:00:00Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.5),
                }],
                meta: None,
            }],
        };
        store.apply_delta(&delta);

        // No subscriptions should return None
        let initial = mgr.get_initial_delta(&store);
        assert!(initial.is_none());
    }

    #[test]
    fn test_get_initial_delta_with_data() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test");
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.subscribe_self_all();

        // Add data to the store
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps".to_string()),
                source: None,
                timestamp: Some("2024-01-01T00:00:00Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.5),
                }],
                meta: None,
            }],
        };
        store.apply_delta(&delta);

        // Should get initial delta with the stored value
        let initial = mgr.get_initial_delta(&store).unwrap();
        assert_eq!(initial.context, Some("vessels.self".to_string()));
        assert_eq!(initial.updates.len(), 1);

        // Find the navigation.speedOverGround value
        let values = &initial.updates[0].values;
        let speed_value = values
            .iter()
            .find(|pv| pv.path == "navigation.speedOverGround");
        assert!(speed_value.is_some());
        assert_eq!(speed_value.unwrap().value, serde_json::json!(3.5));
    }

    #[test]
    fn test_get_initial_delta_filters_by_subscription() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test");
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");

        // Only subscribe to navigation paths
        mgr.add_subscriptions(
            "vessels.self",
            &[Subscription {
                path: "navigation.*".to_string(),
                period: None,
                format: None,
                policy: None,
                min_period: None,
            }],
        );

        // Add data to multiple paths
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: Some("2024-01-01T00:00:00Z".to_string()),
                values: vec![
                    PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: serde_json::json!(3.5),
                    },
                    PathValue {
                        path: "environment.wind.speedApparent".to_string(),
                        value: serde_json::json!(10.0),
                    },
                ],
                meta: None,
            }],
        };
        store.apply_delta(&delta);

        // Should only get navigation paths
        let initial = mgr.get_initial_delta(&store).unwrap();
        let paths: Vec<&str> = initial.updates[0]
            .values
            .iter()
            .map(|pv| pv.path.as_str())
            .collect();

        assert!(paths.contains(&"navigation.speedOverGround"));
        assert!(!paths.contains(&"environment.wind.speedApparent"));
    }

    #[test]
    fn test_get_initial_delta_preserves_source_and_timestamp() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test");
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.subscribe_self_all();

        // Add data with specific source and timestamp
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("nmea0183.GP".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:30:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.5),
                }],
                meta: None,
            }],
        };
        store.apply_delta(&delta);

        let initial = mgr.get_initial_delta(&store).unwrap();

        // Source and timestamp should be captured
        assert_eq!(
            initial.updates[0].source_ref,
            Some("nmea0183.GP".to_string())
        );
        assert_eq!(
            initial.updates[0].timestamp,
            Some("2024-01-17T10:30:00.000Z".to_string())
        );
    }

    #[test]
    fn test_get_initial_delta_multiple_paths() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test");
        let mut mgr = SubscriptionManager::new("vessels.urn:mrn:signalk:uuid:test");
        mgr.subscribe_self_all();

        // Add multiple paths
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: Some("2024-01-01T00:00:00Z".to_string()),
                values: vec![
                    PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: serde_json::json!(3.5),
                    },
                    PathValue {
                        path: "navigation.courseOverGroundTrue".to_string(),
                        value: serde_json::json!(1.52),
                    },
                    PathValue {
                        path: "environment.wind.speedApparent".to_string(),
                        value: serde_json::json!(10.0),
                    },
                ],
                meta: None,
            }],
        };
        store.apply_delta(&delta);

        let initial = mgr.get_initial_delta(&store).unwrap();

        // Should contain all three paths
        assert_eq!(initial.updates[0].values.len(), 3);
    }
}
