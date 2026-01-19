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
│   ├── signalk-core/        # Runtime-agnostic data model
│   ├── signalk-protocol/    # WebSocket/REST message types
│   ├── signalk-server/      # WebSocket server (tokio)
│   ├── signalk-web/         # Admin UI & REST API (axum)
│   ├── signalk-plugins/     # Deno plugin runtime (planned)
│   └── signalk-providers/   # Data source parsers (planned)
│
└── bins/
    ├── signalk-server-linux/  # Full Linux binary (port 4000)
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

**Important: self URN Format**

The `self` property must include the `vessels.` prefix per Signal K spec:
```rust
// Correct
let store = MemoryStore::new("vessels.urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d");

// The full model will have:
// - "self": "vessels.urn:mrn:signalk:uuid:..."
// - "vessels": { "urn:mrn:signalk:uuid:...": { ... } }  // Key WITHOUT "vessels." prefix
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

**Test Coverage (27 tests):**
- `test_hello_message_on_connect` - Verifies Hello message with correct `self` format
- `test_delta_broadcast` - Verifies deltas are broadcast to connected clients
- `test_subscription_filtering` - Verifies path-based subscription filtering
- `test_multiple_clients` - Verifies concurrent client handling
- `test_unsubscribe` - Verifies unsubscribe stops delta delivery
- `test_put_request_returns_not_implemented` - Verifies PUT handling
- And 21 more...

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
