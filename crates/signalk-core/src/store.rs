//! SignalK data store.
//!
//! The store maintains the current state of all SignalK data and provides
//! methods for querying and updating it.
//!
//! ## Multi-Source Value Storage
//!
//! Per the Signal K specification, when multiple sources provide data for the
//! same path, the store maintains:
//! - A primary `value` and `$source` (the most recent update)
//! - A `values` object containing all source values keyed by source ID
//!
//! Example structure:
//! ```json
//! {
//!   "navigation": {
//!     "speedOverGround": {
//!       "value": 3.85,
//!       "$source": "nmea0183.GP",
//!       "timestamp": "2024-01-17T10:30:00.000Z",
//!       "values": {
//!         "nmea0183.GP": { "value": 3.85, "timestamp": "2024-01-17T10:30:00.000Z" },
//!         "nmea2000.115": { "value": 3.82, "timestamp": "2024-01-17T10:29:59.000Z" }
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! ## Sources Hierarchy
//!
//! The store also maintains a `/sources` tree that tracks all data sources
//! that have provided data. This is populated automatically from delta messages.

use crate::model::{Delta, PathValue, Source, Update};
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

    /// Get all sources that have provided data.
    fn get_sources(&self) -> Option<Value>;
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
    /// This is the low-level setter that doesn't handle multi-source values.
    fn set_path_value(&mut self, base_path: &str, path: &str, value: Value) {
        let full_path = if path.is_empty() {
            base_path.to_string()
        } else {
            format!("{base_path}.{path}")
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

    /// Set a SignalK value at a path with multi-source support.
    ///
    /// This method:
    /// 1. Updates the primary value and $source
    /// 2. Stores the source-specific value in the `values` map
    /// 3. Preserves existing values from other sources
    fn set_signalk_value(
        &mut self,
        base_path: &str,
        path: &str,
        value: &Value,
        source_ref: Option<&str>,
        timestamp: Option<&str>,
    ) {
        let full_path = if path.is_empty() {
            base_path.to_string()
        } else {
            format!("{base_path}.{path}")
        };

        let segments: Vec<&str> = full_path.split('.').collect();
        let mut current = &mut self.data;

        // Navigate to the parent of the leaf node
        for (i, segment) in segments.iter().enumerate() {
            if i == segments.len() - 1 {
                // Last segment: handle SignalK value structure
                if let Value::Object(map) = current {
                    let existing = map.get(*segment);

                    // Build the new value object
                    let mut value_obj = serde_json::json!({
                        "value": value
                    });

                    if let Some(src) = source_ref {
                        value_obj["$source"] = Value::String(src.to_string());
                    }

                    if let Some(ts) = timestamp {
                        value_obj["timestamp"] = Value::String(ts.to_string());
                    }

                    // Handle the `values` map for multi-source support
                    if let Some(src) = source_ref {
                        // Create source-specific entry
                        let source_entry = serde_json::json!({
                            "value": value,
                            "timestamp": timestamp
                        });

                        // Preserve existing values map or create new one
                        let mut values_map = if let Some(existing_val) = existing {
                            if let Some(existing_values) = existing_val.get("values") {
                                existing_values.clone()
                            } else {
                                serde_json::json!({})
                            }
                        } else {
                            serde_json::json!({})
                        };

                        // Add/update this source's entry
                        if let Value::Object(vm) = &mut values_map {
                            vm.insert(src.to_string(), source_entry);
                        }

                        value_obj["values"] = values_map;
                    }

                    map.insert(segment.to_string(), value_obj);
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

    /// Register a source in the /sources hierarchy.
    fn register_source(&mut self, source_ref: Option<&str>, source: Option<&Source>) {
        // Get or create source label
        let label = if let Some(src_ref) = source_ref {
            // $source format is usually "label.qualifier" (e.g., "nmea0183.GP", "n2k.115")
            // Extract the label part (before the dot) or use the whole string
            src_ref.split('.').next().unwrap_or(src_ref).to_string()
        } else if let Some(src) = source {
            src.label.clone()
        } else {
            return; // No source info to register
        };

        // Get or create the /sources object
        if let Value::Object(data) = &mut self.data {
            let sources = data
                .entry("sources")
                .or_insert_with(|| serde_json::json!({}));

            if let Value::Object(sources_map) = sources {
                // Create or update the source entry
                if !sources_map.contains_key(&label) {
                    let mut source_entry = serde_json::json!({});

                    // If we have a full Source object, populate more details
                    if let Some(src) = source {
                        if let Some(t) = &src.source_type {
                            source_entry["type"] = Value::String(t.clone());
                        }
                    }

                    sources_map.insert(label.clone(), source_entry);
                }

                // If there's a sub-source (e.g., "115" from "n2k.115"), register it
                if let Some(src_ref) = source_ref {
                    let parts: Vec<&str> = src_ref.split('.').collect();
                    if parts.len() > 1 {
                        let sub_source = parts[1..].join(".");
                        if let Some(Value::Object(label_entry)) = sources_map.get_mut(&label) {
                            label_entry
                                .entry(&sub_source)
                                .or_insert_with(|| serde_json::json!({}));
                        }
                    }
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
            // Register the source in the /sources hierarchy
            self.register_source(update.source_ref.as_deref(), update.source.as_ref());

            for pv in &update.values {
                // Store the value with multi-source support
                self.set_signalk_value(
                    &context,
                    &pv.path,
                    &pv.value,
                    update.source_ref.as_deref(),
                    update.timestamp.as_deref(),
                );
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

    fn get_sources(&self) -> Option<Value> {
        self.data.get("sources").cloned()
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

    // ============================================================
    // Multi-source value tests (matching reference implementation)
    // ============================================================

    #[test]
    fn test_multi_source_values_same_path() {
        // Test based on signalk-server/test/multiple-values.js
        // When multiple sources update the same path, all values should be stored
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        // First source provides a value
        let delta1 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("source1.115".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.trip.log".to_string(),
                    value: serde_json::json!(1),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta1);

        // Verify first value
        let value = store.get_self_path("navigation.trip.log").unwrap();
        assert_eq!(value["value"], serde_json::json!(1));
        assert_eq!(value["$source"], "source1.115");

        // Second source provides a different value for same path
        let delta2 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("source2.116".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:01.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.trip.log".to_string(),
                    value: serde_json::json!(2),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta2);

        // Verify the primary value is from the most recent source
        let value = store.get_self_path("navigation.trip.log").unwrap();
        assert_eq!(value["value"], serde_json::json!(2));
        assert_eq!(value["$source"], "source2.116");

        // Verify both sources are stored in the values map
        assert!(value["values"].is_object());
        assert_eq!(
            value["values"]["source1.115"]["value"],
            serde_json::json!(1)
        );
        assert_eq!(
            value["values"]["source2.116"]["value"],
            serde_json::json!(2)
        );
    }

    #[test]
    fn test_multi_source_preserves_timestamps() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

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

        let delta2 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps2".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:01.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.90),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta1);
        store.apply_delta(&delta2);

        let value = store.get_self_path("navigation.speedOverGround").unwrap();

        // Check timestamps are preserved per source
        assert_eq!(
            value["values"]["gps1"]["timestamp"],
            "2024-01-17T10:00:00.000Z"
        );
        assert_eq!(
            value["values"]["gps2"]["timestamp"],
            "2024-01-17T10:00:01.000Z"
        );
    }

    #[test]
    fn test_same_source_updates_value() {
        // When the same source updates a path, it should replace its own value
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

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

        let delta2 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps1".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:01.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(4.00),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta1);
        store.apply_delta(&delta2);

        let value = store.get_self_path("navigation.speedOverGround").unwrap();

        // Primary value should be updated
        assert_eq!(value["value"], serde_json::json!(4.00));

        // Only one source should be in the values map
        let values_map = value["values"].as_object().unwrap();
        assert_eq!(values_map.len(), 1);
        assert_eq!(value["values"]["gps1"]["value"], serde_json::json!(4.00));
    }

    // ============================================================
    // Sources hierarchy tests
    // ============================================================

    #[test]
    fn test_sources_populated_from_source_ref() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("nmea0183.GP".to_string()),
                source: None,
                timestamp: Some("2024-01-17T10:00:00.000Z".to_string()),
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta);

        // Check sources hierarchy
        let sources = store.get_sources().unwrap();
        assert!(sources["nmea0183"].is_object());
        assert!(sources["nmea0183"]["GP"].is_object());
    }

    #[test]
    fn test_sources_populated_from_multiple_providers() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        // NMEA 0183 source
        let delta1 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("nmea0183.GP".to_string()),
                source: None,
                timestamp: None,
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        // NMEA 2000 source
        let delta2 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("n2k.115".to_string()),
                source: None,
                timestamp: None,
                values: vec![PathValue {
                    path: "navigation.courseOverGroundTrue".to_string(),
                    value: serde_json::json!(1.52),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta1);
        store.apply_delta(&delta2);

        let sources = store.get_sources().unwrap();

        // Both source labels should exist
        assert!(sources["nmea0183"].is_object());
        assert!(sources["n2k"].is_object());

        // Sub-sources should exist
        assert!(sources["nmea0183"]["GP"].is_object());
        assert!(sources["n2k"]["115"].is_object());
    }

    #[test]
    fn test_sources_with_embedded_source_object() {
        use crate::model::Source;

        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: None,
                source: Some(Source {
                    label: "actisense".to_string(),
                    source_type: Some("NMEA2000".to_string()),
                    src: Some("115".to_string()),
                    can_name: None,
                    pgn: Some(128267),
                    sentence: None,
                    talker: None,
                    ais_type: None,
                }),
                timestamp: None,
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta);

        let sources = store.get_sources().unwrap();

        // Source label should be created
        assert!(sources["actisense"].is_object());
        // Type should be captured
        assert_eq!(sources["actisense"]["type"], "NMEA2000");
    }

    #[test]
    fn test_path_count_with_multi_source() {
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        // Two sources updating the same path should still count as one path
        let delta1 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps1".to_string()),
                source: None,
                timestamp: None,
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.85),
                }],
                meta: None,
            }],
        };

        let delta2 = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("gps2".to_string()),
                source: None,
                timestamp: None,
                values: vec![PathValue {
                    path: "navigation.speedOverGround".to_string(),
                    value: serde_json::json!(3.90),
                }],
                meta: None,
            }],
        };

        store.apply_delta(&delta1);
        store.apply_delta(&delta2);

        // Should count as only 1 path, not 2
        assert_eq!(store.path_count(), 1);
    }

    #[test]
    fn test_no_source_provided() {
        // When no source is provided, value should still be stored
        let mut store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:test-vessel");

        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: None,
                source: None,
                timestamp: Some("2024-01-17T10:00:00.000Z".to_string()),
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
        // $source should not be present when no source provided
        assert!(value.get("$source").is_none() || value["$source"].is_null());
    }
}
