# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A high-performance Signal K server implementation in Rust for marine navigation systems. Targets both Linux (full features via Axum + Tokio) and ESP32 (embedded via esp-idf).

## Reference Materials

**IMPORTANT:** This project implements the [Signal K specification v1.7.0](https://signalk.org/specification/1.7.0/). All data model, API, and protocol decisions must follow the official spec.

- **Specification**: `../signalk-specification/` - Local clone of the Signal K specification
- **Reference Implementation**: `../signalk-server/` - TypeScript Signal K server for API compatibility testing
- **Online Spec**: https://signalk.org/specification/1.7.0/
- **Demo Server**: wss://demo.signalk.org - Live demo for testing message formats

### Testing Against References

```bash
# Compare WebSocket output with reference implementation
websocat "ws://localhost:4000/signalk/v1/stream?subscribe=all"
websocat "ws://localhost:3000/signalk/v1/stream?subscribe=all"  # TypeScript ref

# Compare with online demo
websocat "wss://demo.signalk.org/signalk/v1/stream?subscribe=all"

# Test Admin UI server events
websocat "ws://localhost:4000/signalk/v1/stream?serverevents=all&subscribe=none"
```

## Key Specification Requirements

- **Full model** must have top-level `version`, `self`, `vessels`, and `sources` keys
- **`self` property** MUST include `vessels.` prefix (e.g., `"vessels.urn:mrn:signalk:uuid:..."`)
- **Delta context** defaults to self vessel if omitted; uses `"vessels.self"` for self vessel
- **Vessels object** contains vessel data keyed by URN (without `vessels.` prefix in key)
- **Sources tracking** maintains data provenance via `$source` references
- **SI units only** - no unit conversions needed (speed always m/s, temperature always K, etc.)
- **ISO 8601 timestamps** (RFC 3339 format) on all data values

### Hello Message Format
```json
{
  "name": "signalk-server-rust",
  "version": "1.7.0",
  "self": "vessels.urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d",
  "roles": ["master", "main"],
  "timestamp": "2024-01-17T10:30:00.000Z"
}
```

### Server Events (for Admin UI Dashboard)

When clients connect with `?serverevents=all`, send these messages after hello:
1. `VESSEL_INFO` - `{"type":"VESSEL_INFO","data":{"name":null,"uuid":"urn:mrn:..."}}`
2. `PROVIDERSTATUS` - `{"type":"PROVIDERSTATUS","from":"signalk-server","data":[...]}`
3. `SERVERSTATISTICS` - `{"type":"SERVERSTATISTICS","from":"signalk-server","data":{...}}`
4. `DEBUG_SETTINGS` - `{"type":"DEBUG_SETTINGS","data":{"debugEnabled":"","rememberDebug":false}}`
5. `RECEIVE_LOGIN_STATUS` - `{"type":"RECEIVE_LOGIN_STATUS","data":{"status":"notLoggedIn",...}}`
6. `SOURCEPRIORITIES` - `{"type":"SOURCEPRIORITIES","data":{}}`

## Build & Development Commands

```bash
make help              # Show all available targets
make test              # Run all tests
make test-core         # Test signalk-core crate only
make test-server       # Test signalk-server crate only
make build-release     # Build optimized binary
make run               # Start server in debug mode
make clippy            # Run linter
make fmt               # Format code
make ci                # Run full CI checks (format, lint, test)
make pre-commit        # Pre-commit checks (fmt + clippy + test-quiet)
```

Run a single test:
```bash
cargo test -p signalk-server test_hello_message -- --nocapture
```

Enable debug logging:
```bash
RUST_LOG=debug,signalk_server=trace cargo run -p signalk-server-linux
```

## Architecture

### Crate Structure

```
crates/
├── signalk-core/        # Runtime-agnostic data model (NO async code)
├── signalk-protocol/    # WebSocket/REST message types
├── signalk-server/      # WebSocket server (Tokio runtime)
├── signalk-web/         # Admin UI & REST API (Axum framework)
├── signalk-plugins/     # Deno plugin bridge (planned)
└── signalk-providers/   # NMEA parsers (planned)

bins/
├── signalk-server-linux/  # Full Linux binary (port 4000)
└── signalk-server-esp32/  # ESP32 binary (planned)
```

### Key Design Principles

**Runtime Agnostic Core:** signalk-core contains NO async code - pure data structures and logic that work on any runtime. This enables targeting both Tokio (Linux) and esp-idf (ESP32).

**ConfigStorage Abstraction:** Handler logic in signalk-core is generic over `ConfigStorage` trait, allowing platform-specific storage backends (FileStorage for Linux, NvsStorage for ESP32).

**Data Flow:**
```
Provider → Delta → ServerEvent::DeltaReceived → Event Loop
    → MemoryStore.apply_delta() → broadcast::send()
    → Per-Client Filter → WebSocket.send()
```

### Key Types

- `Delta` / `Update` / `PathValue` - Core SignalK message types (signalk-core)
- `MemoryStore` - In-memory SignalK data tree with proper self URN handling
- `PathPattern` - Wildcard path matching for subscriptions (`navigation.*`, `propulsion.*.revolutions`)
- `HelloMessage` / `ServerMessage` / `ClientMessage` - Protocol messages (signalk-protocol)
- `ServerEvent` (signalk-web) - Admin UI WebSocket events (VESSEL_INFO, SERVERSTATISTICS, etc.)

### Unified Server Architecture (Linux)

Single Axum server on port 4000 handles:
- WebSocket connections at `/signalk/v1/stream`
- REST API at `/signalk/v1/api`
- Admin UI at `/admin/` (served from TypeScript reference implementation)
- Discovery at `/signalk`
- Server management at `/skServer/*`

### WebSocket Query Parameters

| Parameter | Values | Default | Description |
|-----------|--------|---------|-------------|
| `subscribe` | `self`, `all`, `none` | `self` | Initial subscription mode |
| `serverevents` | `all`, `none` | `none` | Enable Admin UI server events |
| `sendCachedValues` | `true`, `false` | `true` | Send current state on connect |
| `sendMeta` | `all`, `none` | `none` | Include metadata in responses |

## Testing

Integration tests are in `signalk-server/tests/integration_test.rs` with 27 test cases covering WebSocket connections, subscription filtering, multi-client handling, and delta broadcasting.

Test pattern:
```rust
let (addr, event_tx, handle) = start_test_server().await;
let mut ws = connect_client(addr).await;
// ... test interactions
ws.close(None).await.ok();
handle.abort();
```

## Current Implementation Status

### Working Features
- [x] WebSocket server with hello message (correct `self` format with `vessels.` prefix)
- [x] Delta broadcasting to connected clients
- [x] REST API `/signalk/v1/api` returning full data model
- [x] Discovery endpoint `/signalk`
- [x] Admin UI static file serving
- [x] Server events for Dashboard (`serverevents=all`)
- [x] Demo data generator for testing
- [x] Statistics collection and broadcasting

### In Progress
- [ ] Subscription filtering (subscribe/unsubscribe messages)
- [ ] sendCachedValues on connect

### Planned
- [ ] NMEA data providers
- [ ] Deno plugin bridge
- [ ] Security/authentication
- [ ] Full REST API compatibility

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - Detailed architecture and crate responsibilities
- [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) - SignalK spec details, phase roadmap, API requirements
- [docs/ESP32_MODULARITY.md](docs/ESP32_MODULARITY.md) - ESP32 deployment strategy
- [docs/ESP32_WEB_UI_ANALYSIS.md](docs/ESP32_WEB_UI_ANALYSIS.md) - Admin UI size analysis for embedded
