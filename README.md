# signalk-rs

A high-performance [Signal K](https://signalk.org/) server implementation in Rust.

## Goals

- **Performance**: Rust core for efficient delta processing and WebSocket handling
- **Plugin Compatibility**: Run existing Signal K JavaScript plugins via Deno runtime
- **Dual Target**: Deploy on Linux (full features) or ESP32 (embedded, no plugins)
- **Modular**: Clean crate separation for flexibility

## Project Structure

```
signalk-rs/
├── crates/
│   ├── signalk-core/        # Data model, store, path matching (runtime-agnostic)
│   ├── signalk-protocol/    # WebSocket/REST message types
│   ├── signalk-server/      # Server implementation (tokio or esp-idf)
│   ├── signalk-plugins/     # Deno plugin bridge (Linux only)
│   └── signalk-providers/   # NMEA parsers, data sources
│
└── bins/
    ├── signalk-server-linux/   # Full server + plugins
    └── signalk-server-esp32/   # Embedded server (no plugins)
```

## Quick Start

```bash
make help          # Show all available commands
make run           # Start server in debug mode (port 4000)
make test          # Run all tests
```

## Building

All builds use `make` targets. Run `make help` for the full list.

### Linux Server

```bash
make build              # Build debug
make build-release      # Build optimized release
make run                # Run debug server
make run-release        # Run release server
```

The server runs on port 4000:
- WebSocket: `ws://localhost:4000/signalk/v1/stream`
- REST API: `http://localhost:4000/signalk/v1/api`
- Admin UI: `http://localhost:4000/admin/`

### ESP32

Requires ESP-IDF toolchain. See [esp-rs documentation](https://esp-rs.github.io/book/).

```bash
make build-esp          # Build dev (3MB partition)
make build-esp-release  # Build release (OTA partitions)
make run-esp            # Build and flash (dev)
make run-esp-release    # Build and flash (release)
make esp-size           # Show binary size
```

## Development

```bash
make test           # Run all tests
make test-core      # Test signalk-core only
make test-server    # Test signalk-server only
make clippy         # Run linter
make fmt            # Format code
make ci             # Full CI checks (format, lint, test)
make pre-commit     # Pre-commit checks
make watch          # Watch for changes and rebuild
make watch-run      # Watch and restart server on changes
```

## Status

🚧 **Early Development** - See [docs/RESEARCH_PLAN.md](docs/RESEARCH_PLAN.md) for roadmap.

## License

Apache-2.0
