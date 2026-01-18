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
    pub fn new(self_urn: &str) -> Self {
        let data = serde_json::json!({
            "version": "1.7.0",
            "self": format!("vessels.{}", self_urn),
            "vessels": {
                self_urn: {}
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
    fn resolve_context(&self, context: &str) -> String {
        if context == "vessels.self" {
            format!("vessels.{}", self.self_urn)
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
}

impl SignalKStore for MemoryStore {
    fn apply_delta(&mut self, delta: &Delta) {
        let context = delta
            .context
            .as_ref()
            .map(|c| self.resolve_context(c))
            .unwrap_or_else(|| format!("vessels.{}", self.self_urn));

        for update in &delta.updates {
            for pv in &update.values {
                // Store the value with metadata wrapper
                let value_obj = serde_json::json!({
                    "value": pv.value,
                    "$source": update.source_ref,
                    "timestamp": update.timestamp
                });

                self.set_path_value(&context, &pv.path, value_obj);
            }
        }
    }

    fn get_path(&self, path: &str) -> Option<Value> {
        self.get_path_value(path)
    }

    fn get_self_path(&self, path: &str) -> Option<Value> {
        let full_path = format!("vessels.{}.{}", self.self_urn, path);
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
        let store = MemoryStore::new("urn:mrn:signalk:uuid:test-vessel");
        assert_eq!(store.self_urn(), "urn:mrn:signalk:uuid:test-vessel");
    }

    #[test]
    fn test_apply_delta() {
        let mut store = MemoryStore::new("urn:mrn:signalk:uuid:test-vessel");

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
    }

    #[test]
    fn test_get_context() {
        let mut store = MemoryStore::new("urn:mrn:signalk:uuid:test-vessel");

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
}
