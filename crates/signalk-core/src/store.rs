//! SignalK data store.
//!
//! The store maintains the current state of all SignalK data and provides
//! methods for querying and updating it.

use crate::model::{Delta, PathValue, Update};
use serde_json::Value;
use std::collections::HashMap;

/// Trait for SignalK data storage implementations.
pub trait SignalKStore: Send + Sync {
    /// Apply a delta to the store, merging values into the tree.
    fn apply_delta(&mut self, delta: &Delta);

    /// Get value at an absolute path (e.g., "vessels.self.navigation.position").
    fn get_path(&self, path: &str) -> Option<Value>;

    /// Get value relative to self vessel (e.g., "navigation.position").
    fn get_self_path(&self, path: &str) -> Option<Value>;

    /// Get the full state for a context (e.g., "vessels.self").
    fn get_context(&self, context: &str) -> Option<Value>;

    /// Get the self vessel identifier.
    fn self_urn(&self) -> &str;

    /// Get the full data model as JSON.
    fn full_model(&self) -> &Value;
}

/// In-memory SignalK store implementation.
///
/// Stores the full SignalK tree as a nested JSON structure.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    /// The full SignalK data tree
    data: Value,
    /// The self vessel URN
    self_urn: String,
    /// SignalK version
    version: String,
}

impl MemoryStore {
    /// Create a new empty store with the given self vessel URN.
    ///
    /// The self_urn should be in the format "vessels.urn:mrn:signalk:uuid:..."
    /// per the Signal K spec. The "self" property in the full model points to
    /// this complete path.
    pub fn new(self_urn: &str) -> Self {
        // Extract just the URN part (without "vessels." prefix) for the vessels object key
        let urn_key = self_urn.strip_prefix("vessels.").unwrap_or(self_urn);

        let data = serde_json::json!({
            "version": "1.7.0",
            "self": self_urn,  // Full path like "vessels.urn:mrn:signalk:uuid:..."
            "vessels": {
                urn_key: {}    // Just the URN as the key
            },
            "sources": {}
        });

        Self {
            data,
            self_urn: self_urn.to_string(),
            version: "1.7.0".to_string(),
        }
    }

    /// Resolve "vessels.self" to the actual vessel URN.
    ///
    /// The self_urn is already in "vessels.urn:..." format, so we just return it directly.
    fn resolve_context(&self, context: &str) -> String {
        if context == "vessels.self" {
            self.self_urn.clone()
        } else {
            context.to_string()
        }
    }

    /// Set a value at a path, creating intermediate objects as needed.
    fn set_path_value(&mut self, base_path: &str, path: &str, value: Value) {
        let full_path = if path.is_empty() {
            base_path.to_string()
        } else {
            format!("{}.{}", base_path, path)
        };

        let segments: Vec<&str> = full_path.split('.').collect();
        let mut current = &mut self.data;

        for (i, segment) in segments.iter().enumerate() {
            if i == segments.len() - 1 {
                // Last segment: set the value
                if let Value::Object(map) = current {
                    map.insert(segment.to_string(), value.clone());
                }
            } else {
                // Intermediate segment: ensure object exists
                if let Value::Object(map) = current {
                    if !map.contains_key(*segment) {
                        map.insert(segment.to_string(), serde_json::json!({}));
                    }
                    current = map.get_mut(*segment).unwrap();
                }
            }
        }
    }

    /// Get a value at a path.
    fn get_path_value(&self, path: &str) -> Option<Value> {
        let segments: Vec<&str> = path.split('.').collect();
        let mut current = &self.data;

        for segment in segments {
            match current {
                Value::Object(map) => {
                    current = map.get(segment)?;
                }
                _ => return None,
            }
        }

        Some(current.clone())
    }

    /// Count the number of leaf paths (values) in the store.
    fn count_paths_recursive(value: &Value) -> usize {
        match value {
            Value::Object(map) => {
                // If this object has a "value" key, it's a leaf node
                if map.contains_key("value") {
                    1
                } else {
                    map.values().map(Self::count_paths_recursive).sum()
                }
            }
            _ => 0,
        }
    }

    /// Get the number of unique paths with values in the store.
    pub fn path_count(&self) -> usize {
        if let Some(vessels) = self.data.get("vessels") {
            Self::count_paths_recursive(vessels)
        } else {
            0
        }
    }
}

impl SignalKStore for MemoryStore {
    fn apply_delta(&mut self, delta: &Delta) {
        // Resolve context - "vessels.self" becomes the actual URN path
        let context = delta
            .context
            .as_ref()
            .map(|c| self.resolve_context(c))
            .unwrap_or_else(|| self.self_urn.clone());

        for update in &delta.updates {
            for pv in &update.values {
                // Store the value with metadata wrapper
                let value_obj = serde_json::json!({
                    "value": pv.value,
                    "$source": update.source_ref,
                    "timestamp": update.timestamp
                });

                // Store at the resolved context path (no duplicate "self" entry)
                self.set_path_value(&context, &pv.path, value_obj);
            }
        }
    }

    fn get_path(&self, path: &str) -> Option<Value> {
        self.get_path_value(path)
    }

    fn get_self_path(&self, path: &str) -> Option<Value> {
        // self_urn is already "vessels.urn:...", so just append the path
        let full_path = format!("{}.{}", self.self_urn, path);
        self.get_path_value(&full_path)
    }

    fn get_context(&self, context: &str) -> Option<Value> {
        let resolved = self.resolve_context(context);
        self.get_path_value(&resolved)
    }

    fn self_urn(&self) -> &str {
        &self.self_urn
    }

    fn full_model(&self) -> &Value {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_store() {
        // self_urn must include "vessels." prefix per Signal K spec
        let store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");
        assert_eq!(store.self_urn(), "vessels.urn:mrn:signalk:uuid:test-vessel");

        // Verify initial structure
        let full = store.full_model();
        assert_eq!(full["version"], "1.7.0");
        assert_eq!(full["self"], "vessels.urn:mrn:signalk:uuid:test-vessel");
        assert!(full["vessels"].is_object());
        assert!(full["vessels"]["urn:mrn:signalk:uuid:test-vessel"].is_object());
        assert!(full["sources"].is_object());
    }

    #[test]
    fn test_apply_delta() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test.source".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:30:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta);

        let value = store.get_self_path("navigation.speedOverGround").unwrap();
        assert_eq!(value["value"], serde_json::json!(3.85));
        assert_eq!(value["$source"], "test.source");
        assert_eq!(value["timestamp"], "2024-01-17T10:30:00.000Z");
    }

    #[test]
    fn test_get_context() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test.source".to_string()),
                source: None,
                timestamp: None,
                values: vec![
                    PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: serde_json::json!(3.85),
                    },
                    PathValue {
                        path: "navigation.courseOverGroundTrue".to_string(),
                        value: serde_json::json!(1.52),
                    },
                ],
                meta: None,
            }],
        };

        store.apply_delta(&delta);

        let context = store.get_context("vessels.self").unwrap();
        assert!(context["navigation"]["speedOverGround"]["value"] == serde_json::json!(3.85));
    }

    #[test]
    fn test_multiple_updates_same_path() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        // First update
        let delta1 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps1".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta1);

        // Second update (should overwrite)
        let delta2 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps2".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:01:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(4.12),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta2);

        let value = store.get_self_path("navigation.speedOverGround").unwrap();
        assert_eq!(value["value"], serde_json::json!(4.12));
        assert_eq!(value["$source"], "gps2");
    }

    #[test]
    fn test_nested_path_creation() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "propulsion.mainEngine.oilTemperature".to_string(),
                    value: serde_json::json!(85.5),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta);

        // Verify intermediate objects were created
        let value = store
            .get_self_path("propulsion.mainEngine.oilTemperature")
            .unwrap();
        assert_eq!(value["value"], serde_json::json!(85.5));
    }

    #[test]
    fn test_get_path_absolute() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: None,
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta);

        // Query with absolute path
        let value = store
            .get_path("vessels.urn:mrn:signalk:uuid:test-vessel.navigation.speedOverGround")
            .unwrap();
        assert_eq!(value["value"], serde_json::json!(3.85));
    }

    #[test]
    fn test_get_path_nonexistent() {
        let store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        // Query non-existent path
        let value = store.get_self_path("navigation.nonexistent");
        assert!(value.is_none());
    }

    #[test]
    fn test_complex_value_types() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:00.000Z".to_string()),
                values: vec![
                    PathValue {
                        path: "navigation.position".to_string(),
                        value: serde_json::json!({
                            "latitude": 47.123456,
                            "longitude": -122.654321
                        }),
                    },
                    PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: serde_json::json!(3.85),
                    },
                    PathValue {
                        path: "navigation.destination.waypoint".to_string(),
                        value: serde_json::json!("WP001"),
                    },
                ],
                meta: None,
            }],
        };

        store.apply_delta(&delta);

        let position = store.get_self_path("navigation.position").unwrap();
        assert_eq!(position["value"]["latitude"], 47.123456);
        assert_eq!(position["value"]["longitude"], -122.654321);

        let speed = store.get_self_path("navigation.speedOverGround").unwrap();
        assert_eq!(speed["value"], 3.85);

        let waypoint = store
            .get_self_path("navigation.destination.waypoint")
            .unwrap();
        assert_eq!(waypoint["value"], "WP001");
    }

    #[test]
    fn test_null_value_handling() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        // Set a value
        let delta1 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta1);

        // Set to null (clear the value)
        let delta2 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:01:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::Value::Null,
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta2);

        let value = store.get_self_path("navigation.speedOverGround").unwrap();
        assert!(value["value"].is_null());
    }

    #[test]
    fn test_multiple_contexts() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        // Update self vessel
        let delta1 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: None,
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta1);

        // Update another vessel
        let delta2 = Delta {
            context: Some("vessels.urn:mrn:signalk:uuid:other-vessel".to_string()),
            updates: vec![Update {
                source_ref: Some("ais".to_string()),
                source: None,
                timestamp: None,
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(5.2),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta2);

        // Verify both contexts exist
        let self_speed = store.get_self_path("navigation.speedOverGround").unwrap();
        assert_eq!(self_speed["value"], 3.85);

        let other_speed = store
            .get_path("vessels.urn:mrn:signalk:uuid:other-vessel.navigation.speedOverGround")
            .unwrap();
        assert_eq!(other_speed["value"], 5.2);
    }

    #[test]
    fn test_full_model_query() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("test".to_string()),
                source: None,
                timestamp: None,
                values: vec![
                    PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: serde_json::json!(3.85),
                    },
                    PathValue {
                        path: "environment.wind.speedApparent".to_string(),
                        value: serde_json::json!(12.5),
                    },
                ],
                meta: None,
            }],
        };

        store.apply_delta(&delta);

        let model = store.full_model();
        assert_eq!(model["version"], "1.7.0");
        assert!(model["vessels"]["urn:mrn:signalk:uuid:test-vessel"]["navigation"].is_object());
        assert!(model["vessels"]["urn:mrn:signalk:uuid:test-vessel"]["environment"].is_object());
    }
}
