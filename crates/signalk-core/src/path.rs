//! SignalK path parsing and matching.
//!
//! SignalK paths are dot-separated strings like "navigation.speedOverGround".
//! This module provides utilities for parsing paths and matching them against
//! subscription patterns that may include wildcards.

use regex::Regex;
use std::sync::OnceLock;

/// A parsed SignalK path.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    /// The original path string
    raw: String,
    /// Path segments split by '.'
    segments: Vec<String>,
}

impl Path {
    /// Parse a path string into segments.
    pub fn new(path: &str) -> Self {
        Self {
            raw: path.to_string(),
            segments: path.split('.').map(String::from).collect(),
        }
    }

    /// Get the raw path string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Get the path segments.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Check if this path starts with a given prefix.
    pub fn starts_with(&self, prefix: &Path) -> bool {
        if prefix.segments.len() > self.segments.len() {
            return false;
        }
        self.segments
            .iter()
            .zip(prefix.segments.iter())
            .all(|(a, b)| a == b)
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Path::new(s)
    }
}

impl From<String> for Path {
    fn from(s: String) -> Self {
        Path::new(&s)
    }
}

/// A subscription pattern that may contain wildcards.
///
/// Supported patterns:
/// - Exact: "navigation.speedOverGround"
/// - Suffix wildcard: "navigation.*"
/// - Mid-path wildcard: "propulsion.*.revolutions"
/// - Full wildcard: "*"
#[derive(Debug, Clone)]
pub struct PathPattern {
    raw: String,
    regex: Regex,
}

impl PathPattern {
    /// Create a new path pattern.
    ///
    /// Converts SignalK wildcard syntax to regex:
    /// - `*` at end matches any suffix
    /// - `*` in middle matches exactly one segment
    pub fn new(pattern: &str) -> Result<Self, PatternError> {
        let regex_str = Self::pattern_to_regex(pattern);
        let regex =
            Regex::new(&regex_str).map_err(|e| PatternError::InvalidRegex(e.to_string()))?;

        Ok(Self {
            raw: pattern.to_string(),
            regex,
        })
    }

    /// Convert a SignalK pattern to a regex string.
    fn pattern_to_regex(pattern: &str) -> String {
        if pattern == "*" {
            return "^.*$".to_string();
        }

        let mut regex = String::from("^");
        let segments: Vec<&str> = pattern.split('.').collect();

        for (i, segment) in segments.iter().enumerate() {
            if i > 0 {
                regex.push_str(r"\.");
            }

            if *segment == "*" {
                if i == segments.len() - 1 {
                    // Trailing wildcard: match any suffix
                    regex.push_str(r".*");
                } else {
                    // Mid-path wildcard: match exactly one segment
                    regex.push_str(r"[^.]+");
                }
            } else {
                // Escape special regex characters and add literal segment
                regex.push_str(&regex::escape(segment));
            }
        }

        regex.push('$');
        regex
    }

    /// Check if a path matches this pattern.
    pub fn matches(&self, path: &str) -> bool {
        self.regex.is_match(path)
    }

    /// Get the raw pattern string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

/// Errors that can occur when creating a path pattern.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PatternError {
    #[error("Invalid regex: {0}")]
    InvalidRegex(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_parsing() {
        let path = Path::new("navigation.speedOverGround");
        assert_eq!(path.segments(), &["navigation", "speedOverGround"]);
    }

    #[test]
    fn test_path_starts_with() {
        let path = Path::new("navigation.speedOverGround");
        let prefix = Path::new("navigation");
        assert!(path.starts_with(&prefix));

        let non_prefix = Path::new("propulsion");
        assert!(!path.starts_with(&non_prefix));
    }

    #[test]
    fn test_exact_pattern() {
        let pattern = PathPattern::new("navigation.speedOverGround").unwrap();
        assert!(pattern.matches("navigation.speedOverGround"));
        assert!(!pattern.matches("navigation.courseOverGroundTrue"));
        assert!(!pattern.matches("navigation"));
    }

    #[test]
    fn test_suffix_wildcard() {
        let pattern = PathPattern::new("navigation.*").unwrap();
        assert!(pattern.matches("navigation.speedOverGround"));
        assert!(pattern.matches("navigation.position"));
        assert!(pattern.matches("navigation.course.rhumbline.nextPoint"));
        assert!(!pattern.matches("propulsion.port.revolutions"));
    }

    #[test]
    fn test_mid_path_wildcard() {
        let pattern = PathPattern::new("propulsion.*.revolutions").unwrap();
        assert!(pattern.matches("propulsion.port.revolutions"));
        assert!(pattern.matches("propulsion.starboard.revolutions"));
        assert!(!pattern.matches("propulsion.port.oilPressure"));
        assert!(!pattern.matches("propulsion.revolutions"));
    }

    #[test]
    fn test_full_wildcard() {
        let pattern = PathPattern::new("*").unwrap();
        assert!(pattern.matches("navigation.speedOverGround"));
        assert!(pattern.matches("anything.at.all"));
        assert!(pattern.matches("x"));
    }
}
