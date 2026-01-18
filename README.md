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

## Building

### Linux (default)

```bash
cargo build --release
cargo run --bin signalk-server
```

### ESP32

Requires ESP32 Rust toolchain. See [esp-rs documentation](https://esp-rs.github.io/book/).

```bash
# After toolchain setup:
cargo build --release --bin signalk-server-esp32
```

## Status

🚧 **Early Development** - See [docs/RESEARCH_PLAN.md](docs/RESEARCH_PLAN.md) for roadmap.

## License

Apache-2.0
