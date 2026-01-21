# ESP32 Memory Optimization TODO

## Problem

The ESP32 crashes with `memory allocation of 200000 bytes failed` when a WebSocket client connects. The backtrace shows:

```
regex_automata::nfa::thompson::compiler::Utf8State::clear
  -> alloc::vec::from_elem (200000 bytes)
  -> signalk_core::path::PathPattern::new
  -> signalk_esp32::http::default_subscription_for_mode
```

**Root cause:** The `regex` crate allocates ~200KB to compile even simple patterns like `*`. ESP32 only has ~320KB heap available.

## Current PathPattern Implementation

Location: `crates/signalk-core/src/path.rs`

```rust
pub struct PathPattern {
    pattern: String,
    regex: Regex,  // <-- This is the problem
}

impl PathPattern {
    pub fn new(pattern: &str) -> Result<Self, String> {
        let regex_pattern = Self::pattern_to_regex(pattern);
        let regex = RegexBuilder::new(&regex_pattern)
            .build()
            .map_err(|e| e.to_string())?;
        // ...
    }
}
```

## Solutions

### Option 1: Simple Glob Matching (Recommended for ESP32)

Replace regex with simple string matching for SignalK path patterns:

```rust
pub struct PathPattern {
    pattern: String,
    segments: Vec<PatternSegment>,
}

enum PatternSegment {
    Literal(String),      // exact match
    SingleWildcard,       // * matches one segment
    MultiWildcard,        // ** matches multiple segments (not in SignalK spec)
}

impl PathPattern {
    pub fn matches(&self, path: &str) -> bool {
        // Simple segment-by-segment matching
        // "navigation.*" matches "navigation.position" but not "navigation.position.latitude"
        // "*" matches any single path
    }
}
```

SignalK only uses simple wildcards:
- `*` - matches any single path segment
- `navigation.*` - matches `navigation.position`, `navigation.speedOverGround`
- No regex features needed

### Option 2: Feature Flag for Regex

Keep regex for Linux, use simple matching for ESP32:

```rust
#[cfg(not(feature = "esp32"))]
use regex::Regex;

#[cfg(feature = "esp32")]
struct SimplePattern { /* ... */ }
```

### Option 3: Lightweight Regex Alternative

Use `regex-lite` crate which has lower memory overhead, or `globset` for glob patterns.

## Implementation Plan

1. [ ] Create `SimplePathPattern` in signalk-core that doesn't use regex
2. [ ] Add feature flag `simple-glob` to signalk-core
3. [ ] Enable `simple-glob` for ESP32 builds
4. [ ] Test pattern matching: `*`, `navigation.*`, `propulsion.*.revolutions`
5. [ ] Benchmark memory usage on ESP32

## Other Memory Issues to Address

### sendCachedValues

Even after fixing PathPattern, `sendCachedValues` will fail because:
- Serializing full model to String requires ~200KB allocation
- Solution: Stream JSON directly to WebSocket using fragmented frames

### REST API /signalk/v1/api

Same issue - full model serialization. Options:
- Use `serde_json::to_writer()` with HTTP response (supports Write trait)
- Limit model size on ESP32
- Return error for full model, only allow path queries

## Priority

1. **HIGH** - Fix PathPattern (blocking WebSocket connections)
2. **MEDIUM** - Streaming sendCachedValues (nice to have)
3. **LOW** - REST API streaming (can use path queries instead)

## References

- ESP32 heap: ~320KB total, ~280KB available after WiFi
- Regex compilation overhead: https://github.com/rust-lang/regex/issues/583
- `regex-lite` crate: Lower memory, subset of regex features