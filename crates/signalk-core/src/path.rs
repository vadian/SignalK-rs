//! SignalK path parsing and matching.
//!
//! SignalK paths are dot-separated strings like "navigation.speedOverGround".
//! This module provides utilities for parsing paths and matching them against
//! subscription patterns that may include wildcards.
//!
//! Pattern matching uses simple glob-style matching without regex to minimize
//! memory usage on embedded platforms (ESP32).

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

/// A segment in a path pattern.
#[derive(Debug, Clone, PartialEq)]
enum PatternSegment {
    /// Exact literal match for this segment
    Literal(String),
    /// Single wildcard (*) - matches exactly one segment when mid-path,
    /// or any suffix when at the end
    Wildcard,
}

/// A subscription pattern that may contain wildcards.
///
/// Supported patterns:
/// - Exact: "navigation.speedOverGround"
/// - Suffix wildcard: "navigation.*"
/// - Mid-path wildcard: "propulsion.*.revolutions"
/// - Full wildcard: "*"
///
/// Uses simple segment-based matching instead of regex to minimize memory
/// usage on embedded platforms like ESP32.
#[derive(Debug, Clone)]
pub struct PathPattern {
    raw: String,
    segments: Vec<PatternSegment>,
    /// True if the pattern ends with a wildcard (matches any suffix)
    trailing_wildcard: bool,
}

impl PathPattern {
    /// Create a new path pattern.
    ///
    /// Pattern syntax:
    /// - `*` at end matches any suffix (e.g., "navigation.*" matches "navigation.position.latitude")
    /// - `*` in middle matches exactly one segment (e.g., "propulsion.*.revolutions")
    /// - `*` alone matches any path
    pub fn new(pattern: &str) -> Result<Self, PatternError> {
        let raw = pattern.to_string();
        let parts: Vec<&str> = pattern.split('.').collect();

        // Check for empty pattern
        if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
            return Err(PatternError::EmptyPattern);
        }

        let trailing_wildcard = parts.last() == Some(&"*");

        let segments: Vec<PatternSegment> = parts
            .iter()
            .map(|&s| {
                if s == "*" {
                    PatternSegment::Wildcard
                } else {
                    PatternSegment::Literal(s.to_string())
                }
            })
            .collect();

        Ok(Self {
            raw,
            segments,
            trailing_wildcard,
        })
    }

    /// Check if a path matches this pattern.
    pub fn matches(&self, path: &str) -> bool {
        let path_parts: Vec<&str> = path.split('.').collect();

        // Special case: single wildcard matches everything
        if self.segments.len() == 1 && self.segments[0] == PatternSegment::Wildcard {
            return true;
        }

        // If trailing wildcard, path must have at least (pattern_len - 1) segments
        // If no trailing wildcard, path must have exactly pattern_len segments
        if self.trailing_wildcard {
            if path_parts.len() < self.segments.len() - 1 {
                return false;
            }
        } else if path_parts.len() != self.segments.len() {
            return false;
        }

        // Match each segment
        for (i, segment) in self.segments.iter().enumerate() {
            match segment {
                PatternSegment::Literal(lit) => {
                    if i >= path_parts.len() || path_parts[i] != lit {
                        return false;
                    }
                }
                PatternSegment::Wildcard => {
                    // Trailing wildcard matches any remaining suffix
                    if self.trailing_wildcard && i == self.segments.len() - 1 {
                        return true;
                    }
                    // Mid-path wildcard must have a corresponding path segment
                    if i >= path_parts.len() {
                        return false;
                    }
                    // Wildcard matches any single segment (non-empty)
                    if path_parts[i].is_empty() {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Get the raw pattern string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

/// Errors that can occur when creating a path pattern.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PatternError {
    #[error("Empty pattern")]
    EmptyPattern,
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
