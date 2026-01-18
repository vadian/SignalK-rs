# SignalK-RS Architecture

This document describes the architecture of the Rust SignalK server implementation.

## Overview

SignalK-RS is a modular implementation of the [SignalK specification](https://signalk.org/specification/1.7.0/doc/) designed for:
- **Linux servers** - Full-featured with plugin support via Deno
- **ESP32 embedded** - Lightweight, no plugins (future)

## Crate Structure

```
signalk-rs/
├── crates/
│   ├── signalk-core/        # Runtime-agnostic data model
│   ├── signalk-protocol/    # WebSocket/REST message types
│   ├── signalk-server/      # WebSocket server (tokio)
│   ├── signalk-web/         # Admin UI & REST API (axum)
│   ├── signalk-plugins/     # Deno plugin runtime
│   └── signalk-providers/   # Data source parsers (NMEA, etc.)
│
└── bins/
    ├── signalk-server-linux/  # Full Linux binary
    └── signalk-server-esp32/  # ESP32 binary (future)
```

## Crate Responsibilities

### signalk-core

**Purpose:** Runtime-agnostic data model and storage.

**Key Types:**
- `Delta` - The fundamental SignalK update message
- `Update` - A single update within a delta (source + timestamp + values)
- `PathValue` - A path/value pair within an update
- `Source` - Data source metadata (NMEA sentence info, etc.)
- `MemoryStore` - In-memory SignalK data tree
- `PathPattern` - Wildcard path matching for subscriptions
- `ConfigStorage` - Storage abstraction trait for configuration
- `ConfigHandlers` - Framework-agnostic handler logic

**Design Principles:**
- No async code - works on any runtime
- No I/O - pure data structures and logic
- Serde-based serialization
- Storage abstraction for cross-platform compatibility

### signalk-protocol

**Purpose:** WebSocket and REST API message definitions.

**Key Types:**
- `HelloMessage` - Server identification sent on connect
- `ServerMessage` - Union of all server→client messages
- `ClientMessage` - Union of all client→server messages
- `SubscribeRequest` / `UnsubscribeRequest` - Subscription management
- `PutRequest` / `PutResponse` - Data modification
- `DiscoveryResponse` - `/signalk` endpoint response

**Codec:**
- `encode_server_message()` - Serialize for WebSocket transmission
- `decode_client_message()` - Parse incoming client messages

### signalk-server

**Purpose:** WebSocket server handling connections and subscriptions.

**Key Types:**
- `SignalKServer` - Main server struct
- `ServerConfig` - Server configuration
- `ServerEvent` - Events for injecting data (from providers)
- `SubscriptionManager` - Per-client subscription state
- `ClientSubscription` - Individual subscription with path pattern

**Connection Flow:**
1. Client connects via WebSocket
2. Server sends `HelloMessage`
3. Client optionally sends `SubscribeRequest`
4. Server filters and broadcasts deltas based on subscriptions
5. Client can send `PutRequest` for actions

**Threading Model:**
```
┌─────────────────────────────────────────────────────────┐
│                    SignalKServer                         │
│                                                          │
│  ┌──────────────┐    broadcast::channel    ┌──────────┐ │
│  │ Event Loop   │ ──────────────────────── │ Client 1 │ │
│  │              │                          └──────────┘ │
│  │ - Apply delta│    (Delta broadcast)     ┌──────────┐ │
│  │ - Update store│ ─────────────────────── │ Client 2 │ │
│  └──────────────┘                          └──────────┘ │
│         ▲                                               │
│         │ mpsc::channel                                 │
│         │ (ServerEvent)                                 │
│  ┌──────┴───────┐                                       │
│  │   Providers  │                                       │
│  └──────────────┘                                       │
└─────────────────────────────────────────────────────────┘
```

### signalk-web

**Purpose:** Admin Web UI and REST API (Linux only).

**Components:**
- Static file serving for React Admin UI
- REST API routes matching TypeScript implementation
- WebSocket server events for dashboard

**Route Groups:**
- `/admin/` - Static Admin UI files
- `/signalk/v1/` - SignalK REST API
- `/skServer/` - Server management endpoints

**Key Files:**
- `routes/auth.rs` - Authentication endpoints
- `routes/config.rs` - Settings and vessel configuration
- `routes/security.rs` - User and device management
- `routes/plugins.rs` - Plugin management
- `routes/backup.rs` - Backup/restore
- `server_events.rs` - Real-time dashboard updates
- `statistics.rs` - Performance metric collection

### signalk-plugins (Future)

**Purpose:** Run existing SignalK JavaScript plugins via Deno.

**Architecture:**
```
┌─────────────────────────────────────────────────────────┐
│                    Rust Server                          │
│  ┌─────────────────────────────────────────────────┐   │
│  │              Plugin Bridge (IPC)                 │   │
│  └─────────────────────────────────────────────────┘   │
└───────────────────────┬─────────────────────────────────┘
                        │ Unix socket / JSON messages
┌───────────────────────┴─────────────────────────────────┐
│                    Deno Runtime                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │  Plugin A   │  │  Plugin B   │  │  Plugin C   │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │           ServerAPI Shim (TypeScript)           │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### signalk-providers (Future)

**Purpose:** Parse data from various marine sources.

**Planned Providers:**
- NMEA 0183 (serial/TCP/UDP)
- NMEA 2000 (CAN bus via socketcan)
- SignalK TCP/UDP
- File replay

## Data Flow

### Delta Processing

```
Provider (NMEA sentence)
    │
    ▼
Parse to Delta
    │
    ▼
ServerEvent::DeltaReceived
    │
    ▼
Event Loop
    │
    ├──► MemoryStore.apply_delta()
    │
    └──► broadcast::send(delta)
            │
            ▼
        Per-Client Filter
            │
            ▼
        WebSocket.send()
```

### Subscription Filtering

Clients subscribe to paths with optional wildcards:
- `navigation.*` - All navigation paths
- `propulsion.*.revolutions` - Any engine's RPM
- `*` - Everything

The `SubscriptionManager` filters each delta to only include
paths matching the client's subscriptions.

## Configuration

### Storage Abstraction

Configuration storage is abstracted via the `ConfigStorage` trait in signalk-core.
This allows the same handler logic to work on different platforms:

```
┌─────────────────────────────────────────────────────────────┐
│                    ConfigStorage trait                       │
│  load_settings() / save_settings()                          │
│  load_vessel() / save_vessel()                              │
│  load_security() / save_security()                          │
│  load_plugin_config() / save_plugin_config()                │
└─────────────────────────────────────────────────────────────┘
           │                              │
           ▼                              ▼
┌─────────────────────┐      ┌─────────────────────┐
│   FileStorage       │      │    NvsStorage       │
│   (Linux)           │      │    (ESP32)          │
│   ~/.signalk/*.json │      │    esp-idf NVS      │
└─────────────────────┘      └─────────────────────┘
```

**Handler Logic Pattern:**

```rust
// Framework-agnostic handler in signalk-core
pub fn get_settings<S: ConfigStorage>(storage: &S) -> Result<ServerSettings, ConfigError> {
    storage.load_settings()
}

// Axum wrapper in signalk-web
async fn get_settings_route(
    State(state): State<AppState>,
) -> Result<Json<ServerSettings>, StatusCode> {
    ConfigHandlers::get_settings(&state.storage)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ESP32 wrapper (future)
fn handle_get_settings(storage: &NvsStorage) -> HttpResponse {
    match ConfigHandlers::get_settings(storage) {
        Ok(settings) => HttpResponse::ok_json(&settings),
        Err(_) => HttpResponse::error(500),
    }
}
```

This pattern enables:
- Shared business logic across platforms
- Platform-specific storage backends
- Easy testing with in-memory storage

### File Structure (Compatible with TypeScript server)

```
~/.signalk/
├── settings.json          # Server settings
├── security.json          # Users and ACLs
├── plugin-config-data/    # Per-plugin config
│   └── <plugin-id>.json
├── resources/             # Routes, waypoints, etc.
└── logs/                  # Server logs
```

### settings.json Schema

```json
{
  "interfaces": {
    "appstore": true,
    "plugins": true,
    "rest": true,
    "signalk-ws": true
  },
  "port": 3000,
  "ssl": false,
  "mdns": true,
  "wsCompression": false,
  "accessLogging": false,
  "security": {
    "strategy": "./tokensecurity"
  },
  "pipedProviders": [
    {
      "id": "nmea0183",
      "pipeElements": [
        { "type": "providers/serialport", "options": { "device": "/dev/ttyUSB0", "baudrate": 4800 } },
        { "type": "providers/nmea0183-signalk" }
      ]
    }
  ]
}
```

## Testing

### Unit Tests

Each crate has unit tests for core functionality:

```bash
cargo test --workspace
```

### Integration Tests

WebSocket server integration tests in `signalk-server/tests/`:

```bash
cargo test -p signalk-server --test integration_test
```

**Test Coverage:**
- `test_hello_message_on_connect` - Verifies Hello message is sent on WebSocket connection
- `test_delta_broadcast` - Verifies deltas are broadcast to connected clients
- `test_subscription_filtering` - Verifies path-based subscription filtering
- `test_multiple_clients` - Verifies concurrent client handling
- `test_unsubscribe` - Verifies unsubscribe stops delta delivery
- `test_put_request_returns_not_implemented` - Verifies PUT handling

## Performance Targets

- Delta processing latency: <10ms (Rust core)
- Plugin round-trip: <5ms (Deno IPC)
- Memory: <50MB base, scales with data
- Connections: 100+ concurrent clients

## Implementation Status

See [RESEARCH_PLAN.md](./RESEARCH_PLAN.md) for detailed phase tracking.

**Test Summary:** 33 tests passing across all crates.

### Completed
- [x] Core data model (signalk-core) - 12 unit tests
- [x] Protocol messages (signalk-protocol) - 9 unit tests
- [x] WebSocket server (signalk-server) - 4 unit tests
- [x] Integration tests (signalk-server) - 6 tests
- [x] Subscription filtering
- [x] Path pattern matching with wildcards
- [x] Statistics collection (signalk-web) - 2 unit tests

### In Progress
- [ ] REST API endpoints (signalk-web) - stubs complete
- [ ] Admin UI static serving
- [ ] Subscription policies (instant/ideal/fixed)
- [ ] Period/minPeriod throttling

### Planned
- [ ] Deno plugin runtime
- [ ] NMEA providers
- [ ] ESP32 port
