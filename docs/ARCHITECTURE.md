# SignalK-RS Architecture

This document describes the architecture of the Rust SignalK server implementation.

## Overview

SignalK-RS is a modular implementation of the [SignalK specification v1.7.0](https://signalk.org/specification/1.7.0/doc/) designed for:
- **Linux servers** - Full-featured with plugin support via Deno (planned)
- **ESP32 embedded** - Lightweight, no plugins (future)

## Reference Materials

- **Specification**: `../signalk-specification/` - Local clone for offline reference
- **Reference Implementation**: `../signalk-server/` - TypeScript server for API compatibility
- **Demo Server**: wss://demo.signalk.org - Live demo for testing

## Crate Structure

```
signalk-rs/
├── crates/
│   ├── signalk-core/        # Runtime-agnostic data model (no async)
│   ├── signalk-protocol/    # WebSocket/REST message types
│   ├── signalk-server/      # WebSocket server (tokio)
│   ├── signalk-web/         # Admin UI & REST API (axum)
│   ├── signalk-plugins/     # Deno plugin runtime (planned)
│   ├── signalk-providers/   # Data source parsers (planned)
│   └── signalk-esp32/       # ESP32-specific components (WiFi, NVS, HTTP)
│
└── bins/
    ├── signalk-server-linux/  # Full Linux binary (port 4000)
    └── signalk-server-esp32/  # ESP32 binary (port 80) - separate build
```

### Platform Support

| Platform | Toolchain | Crates Used | Features |
|----------|-----------|-------------|----------|
| Linux | Standard Rust | core, protocol, server, web, plugins | Full (Admin UI, plugins, providers) |
| ESP32 (Xtensa) | esp-rs | core, protocol, esp32 | Minimal (WebSocket, REST, discovery) |

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

**Important: self URN Format**

The `self` property must include the `vessels.` prefix per Signal K spec:
```rust
// Correct
let store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d");

// The full model will have:
// - "self": "vessels.urn:mrn:signalk:uuid:..."
// - "vessels": { "urn:mrn:signalk:uuid:...": { ... } }  // Key WITHOUT "vessels." prefix
```

**Multi-Source Value Storage:**

When multiple sources provide data for the same path, the store maintains all values:

```json
{
  "navigation": {
    "speedOverGround": {
      "value": 3.85,           // Primary value (most recent)
      "$source": "nmea0183.GP", // Primary source
      "timestamp": "2024-01-17T10:30:00.000Z",
      "values": {              // All source values
        "nmea0183.GP": { "value": 3.85, "timestamp": "2024-01-17T10:30:00.000Z" },
        "nmea2000.115": { "value": 3.82, "timestamp": "2024-01-17T10:29:59.000Z" }
      }
    }
  }
}
```

**Sources Hierarchy:**

The `/sources` tree is automatically populated from delta messages:

```json
{
  "sources": {
    "nmea0183": { "GP": {} },
    "nmea2000": { "115": {}, "127": {} },
    "actisense": { "type": "NMEA2000" }
  }
}
```

### signalk-protocol

**Purpose:** WebSocket and REST API message definitions.

**Key Types:**
- `HelloMessage` - Server identification sent on connect (includes `self` with `vessels.` prefix)
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
- `ServerConfig` - Server configuration (includes `self_urn` with `vessels.` prefix)
- `ServerEvent` - Events for injecting data (from providers)
- `SubscriptionManager` - Per-client subscription state
- `ClientSubscription` - Individual subscription with path pattern

**Connection Flow:**
1. Client connects via WebSocket
2. Server sends `HelloMessage` with `self` property
3. If `serverevents=all`, server sends VESSEL_INFO, PROVIDERSTATUS, etc.
4. Server filters and broadcasts deltas based on subscriptions
5. Client can send `SubscribeRequest` / `UnsubscribeRequest`
6. Client can send `PutRequest` for actions (planned)

### signalk-web

**Purpose:** Admin Web UI and REST API (Linux only).

**Components:**
- Static file serving for React Admin UI
- REST API routes matching TypeScript implementation
- WebSocket server events for dashboard

**Server Events (sent when `serverevents=all`):**

| Event Type | Description |
|------------|-------------|
| `VESSEL_INFO` | Vessel name and UUID (sent once on connect) |
| `PROVIDERSTATUS` | Provider/plugin status updates |
| `SERVERSTATISTICS` | Delta rate, path count, client count (1 Hz) |
| `DEBUG_SETTINGS` | Debug configuration |
| `RECEIVE_LOGIN_STATUS` | Authentication status |
| `SOURCEPRIORITIES` | Source priority settings |
| `LOG` | Real-time log entries |

**Route Groups:**
- `/admin/` - Static Admin UI files (React SPA)
- `/signalk/v1/` - SignalK REST API and WebSocket
- `/skServer/` - Server management endpoints

**Key Files:**
- `routes/auth.rs` - Authentication endpoints
- `routes/config.rs` - Settings and vessel configuration
- `routes/security.rs` - User and device management
- `server_events.rs` - Real-time dashboard event types
- `statistics.rs` - Performance metric collection

### signalk-plugins (Planned)

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

### signalk-providers (Planned)

**Purpose:** Parse data from various marine sources.

**Planned Providers:**
- NMEA 0183 (serial/TCP/UDP)
- NMEA 2000 (CAN bus via socketcan)
- SignalK TCP/UDP
- File replay

### signalk-esp32

**Purpose:** ESP32-specific shared components for embedded SignalK servers.

**Key Modules:**
- `wifi` - WiFi connection management with scanning and auto-reconnect
- `config` - NVS-based configuration storage (planned)
- `http` - Helper functions for SignalK HTTP/WebSocket handlers

**Design Principles:**
- Reusable across different ESP32 board variants
- Uses blocking APIs (no async) - FreeRTOS threads for concurrency
- Memory-efficient for constrained environments

## Current Architecture (Linux)

The Linux implementation uses a unified Axum server on port 4000:

```
┌─────────────────────────────────────────────────────────┐
│                   Unified Axum Server (Port 4000)       │
│                                                         │
│  ┌──────────────┐    broadcast::channel    ┌──────────┐│
│  │ Event Loop   │ ──────────────────────── │ WS Client││
│  │              │                          └──────────┘│
│  │ - Apply delta│    (Delta broadcast)     ┌──────────┐│
│  │ - Update store│ ─────────────────────── │ WS Client││
│  └──────────────┘                          └──────────┘│
│         ▲                                      │        │
│         │ mpsc::channel                        │        │
│         │ (ServerEvent)                        │        │
│  ┌──────┴───────┐                    ┌────────┴──────┐ │
│  │   Providers  │                    │  REST API     │ │
│  │   (Demo)     │                    │  /admin/ UI   │ │
│  └──────────────┘                    └───────────────┘ │
└─────────────────────────────────────────────────────────┘
```

**Endpoints:**
- `GET /signalk` - Discovery endpoint
- `GET /signalk/v1/api` - Full data model
- `GET /signalk/v1/api/*path` - Path-specific data
- `WS /signalk/v1/stream` - WebSocket with query params
- `GET /admin/*` - Admin UI static files
- `GET /skServer/*` - Server management REST API

## ESP32 Architecture

The ESP32 implementation uses esp-idf-svc on port 80 with real-time delta streaming:

```
┌─────────────────────────────────────────────────────────────┐
│              esp-idf-svc Server (Port 80)                   │
│                                                             │
│  ┌────────────────┐                                         │
│  │ Demo Generator │──────┐                                  │
│  │ (std::thread)  │      │                                  │
│  └────────────────┘      │ mpsc::channel                    │
│                          ▼                                  │
│  ┌────────────────────────────┐     ┌───────────────────┐  │
│  │     Delta Processor        │     │   WS Clients      │  │
│  │     (std::thread)          │     │   HashMap<i32,    │  │
│  │                            │────►│   DetachedSender> │  │
│  │  1. Apply to MemoryStore   │     └─────────┬─────────┘  │
│  │  2. Broadcast to clients   │               │            │
│  └────────────────────────────┘               │            │
│                                               ▼            │
│                                    ┌──────────────────┐    │
│                                    │  WebSocket Push  │    │
│                                    │  (async via      │    │
│                                    │   httpd_queue)   │    │
│                                    └──────────────────┘    │
│                                                             │
│  ┌────────────────────────────────────────────────────┐    │
│  │                  HTTP Server                        │    │
│  │  GET /signalk         - Discovery                   │    │
│  │  GET /signalk/v1/api  - Full model JSON            │    │
│  │  WS  /signalk/v1/stream - WebSocket streaming      │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

**Key Implementation Details:**

- **Delta Broadcast**: Uses `EspHttpWsDetachedSender` to push deltas from the processor thread to WebSocket clients. This leverages ESP-IDF's `httpd_ws_send_frame_async` for server-initiated push.
- **Client Tracking**: Connected clients stored in `Arc<Mutex<HashMap<i32, EspHttpWsDetachedSender>>>` keyed by socket fd.
- **Thread Stack**: All threads use 16KB stack via `std::thread::Builder::stack_size()` to match `CONFIG_PTHREAD_STACK_MIN`.

**Key Differences from Linux:**

| Component | Linux | ESP32 |
|-----------|-------|-------|
| HTTP Server | Axum (async) | esp-idf-svc (blocking) |
| Delta Broadcast | `broadcast::channel` | `EspHttpWsDetachedSender` |
| Concurrency | tokio::spawn | std::thread::Builder |
| Sync primitives | RwLock | Mutex |
| Config storage | Filesystem | NVS (planned) |
| Admin UI | Full React (34MB) | None |
| Plugins | Deno runtime | Not supported |
| Port | 4000 | 80 |
| Binary size | ~5MB | ~500KB |
| RAM usage | ~50MB | ~80KB |

**Shared Code:**

The following crates work unchanged on ESP32:
- `signalk-core` - MemoryStore, Delta, PathValue
- `signalk-protocol` - HelloMessage, ServerMessage, DiscoveryResponse

**Build Requirements:**

ESP32 requires the Espressif Rust toolchain (installed via `espup`). The `rust-toolchain.toml` in the ESP32 directory automatically selects the correct toolchain:

```bash
# Install ESP toolchain (one-time setup)
cargo install espup && espup install
. $HOME/export-esp.sh  # Source ESP environment

# Build and flash ESP32 binary using Makefile targets
WIFI_SSID="network" WIFI_PASSWORD="pass" make run-esp          # Dev build
WIFI_SSID="network" WIFI_PASSWORD="pass" make run-esp-release  # Release build

# Or manually:
cd bins/signalk-server-esp32
WIFI_SSID="network" WIFI_PASSWORD="pass" \
ESP_IDF_SDKCONFIG_DEFAULTS="../../sdkconfig.defaults;../../sdkconfig.defaults.dev" \
cargo run
```

**Build Configurations:**

| Target | Partition | Optimizations | Max Size |
|--------|-----------|---------------|----------|
| `make run-esp` | 3MB factory | Debug (-O2) | 3 MB |
| `make run-esp-release` | OTA (2x 1.5MB) | Size (-Os, LTO) | 1.5 MB |

See [ESP32_BINARY_SIZE_PLAN.md](./ESP32_BINARY_SIZE_PLAN.md) for details on size optimization.

**Known Issue - sdkconfig.defaults Location:**

The `esp-idf-sys` build system uses `sdkconfig.defaults` from the **workspace root**, not from the ESP32 binary directory. This is because it resolves paths relative to where `Cargo.lock` and `target/` are located. See [esp-idf-sys BUILD-OPTIONS.md](https://github.com/esp-rs/esp-idf-sys/blob/master/BUILD-OPTIONS.md) for details.

See [bins/signalk-server-esp32/README.md](../bins/signalk-server-esp32/README.md) for full setup instructions.

## Data Flow

### Delta Processing

```
Provider (NMEA sentence or demo data)
    │
    ▼
Parse to Delta { context, updates }
    │
    ▼
ServerEvent::DeltaReceived
    │
    ▼
Event Loop
    │
    ├──► MemoryStore.apply_delta()
    │    - Resolves "vessels.self" to actual URN
    │    - Stores under "vessels.<urn>.<path>"
    │
    └──► broadcast::send(delta)
            │
            ▼
        Per-Client Handler
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

**Test Coverage (89 total tests):**

| Crate | Tests | Description |
|-------|-------|-------------|
| signalk-core | 31 | Data model, store operations, multi-source values, sources hierarchy |
| signalk-protocol | 9 | Message serialization/deserialization |
| signalk-server | 47 | Unit tests + integration tests (WebSocket, subscriptions, delta caching) |
| signalk-web | 2 | Statistics collection, client tracking |

**Key Integration Tests:**
- `test_hello_message_on_connect` - Verifies Hello message with correct `self` format
- `test_delta_broadcast` - Verifies deltas are broadcast to connected clients
- `test_subscription_filtering` - Verifies path-based subscription filtering
- `test_multiple_sources_same_path` - Verifies multi-source value storage
- `test_initial_cached_values` - Verifies sendCachedValues=true functionality
- `test_subscription_policy_warning_*` - Verifies policy validation warnings

### Comparing with Reference Implementation

```bash
# Start Rust server on port 4000
cargo run -p signalk-server-linux

# Start TypeScript reference on port 3000
cd ../signalk-server && npm start

alternatively run signalk/signalk-server docker image and expose the port

# Compare outputs
websocat "ws://localhost:4000/signalk/v1/stream?subscribe=none"
websocat "ws://localhost:3000/signalk/v1/stream?subscribe=none"

# Test server events (Dashboard query)
websocat "ws://localhost:4000/signalk/v1/stream?serverevents=all&subscribe=none"
```

## Performance Targets

- Delta processing latency: <10ms (Rust core)
- Plugin round-trip: <5ms (Deno IPC, planned)
- Memory: <50MB base, scales with data
- Connections: 100+ concurrent clients

## Implementation Roadmap

See [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) for detailed phase tracking and specification details.
