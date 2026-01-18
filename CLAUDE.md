# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A high-performance Signal K server implementation in Rust for marine navigation systems. Targets both Linux (full features via Axum + Tokio) and ESP32 (embedded via esp-idf).

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
├── signalk-server-linux/  # Full Linux binary
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
- `MemoryStore` - In-memory SignalK data tree
- `PathPattern` - Wildcard path matching for subscriptions (`navigation.*`, `propulsion.*.revolutions`)
- `HelloMessage` / `ServerMessage` / `ClientMessage` - Protocol messages (signalk-protocol)
- `SignalKServer` / `ServerEvent` - Server implementation (signalk-server)

### Unified Server Architecture (Linux)

Single Axum server on port 3001 handles:
- WebSocket connections at `/signalk/v1/stream`
- REST API at `/signalk/v1/api`
- Admin UI at `/admin/`
- Discovery at `/signalk`

## Testing

Integration tests are in `signalk-server/tests/integration_test.rs` with 25+ test cases covering WebSocket connections, subscription filtering, multi-client handling, and delta broadcasting.

Test pattern:
```rust
let (addr, event_tx, handle) = start_test_server().await;
let mut ws = connect_client(addr).await;
// ... test interactions
ws.close(None).await.ok();
handle.abort();
```

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - Detailed architecture and crate responsibilities
- [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) - SignalK spec details, phase roadmap, API requirements
- [docs/ESP32_MODULARITY.md](docs/ESP32_MODULARITY.md) - ESP32 deployment strategy
- [docs/ESP32_WEB_UI_ANALYSIS.md](docs/ESP32_WEB_UI_ANALYSIS.md) - Admin UI size analysis for embedded
