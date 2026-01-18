# SignalK Rust Server - Research & Implementation Plan

## Executive Summary

This document consolidates research on building a high-performance SignalK server in Rust with **Deno-based JavaScript plugin compatibility**. The goal is a performant server that can run existing SignalK plugins with minimal modifications.

**Target Platform:** Linux (x86_64, ARM64 - Raspberry Pi, etc.)
**Plugin Strategy:** Deno runtime with compatibility shim for existing JS plugins

---

## 1. SignalK Specification Overview (v1.7.0)

### 1.1 Core Concepts

SignalK is a universal marine data model using JSON that provides:
- **Hierarchical data structure** with consistent paths (e.g., `navigation.speedOverGround`)
- **SI units throughout** - no conversion needed between paths
- **Source tracking** - multiple devices can provide the same data point
- **Delta updates** - efficient partial updates rather than full state transmission

### 1.2 Data Model Structure

**Root Object:**
```json
{
  "version": "1.7.0",
  "self": "vessels.urn:mrn:signalk:uuid:...",
  "vessels": { ... },
  "sources": { ... }
}
```

**Vessel Groups (13 categories):**
| Group | Description |
|-------|-------------|
| `navigation` | Position, heading, speed, course, attitude |
| `propulsion` | Engine data (RPM, oil pressure, temperature) |
| `electrical` | Batteries, alternators, solar panels |
| `environment` | Wind, water temp, air temp, humidity, depth |
| `steering` | Rudder angle, autopilot state |
| `tanks` | Fuel, water, holding tank levels |
| `communication` | VHF, AIS, radio states |
| `design` | Vessel specifications (length, beam, draft) |
| `sails` | Sail configuration and state |
| `sensors` | Generic sensor data |
| `performance` | Polar performance, VMG, targets |
| `notifications` | Alarms and alerts |
| `resources` | Routes, waypoints, notes |

**Value Object Structure:**
```json
{
  "value": 3.85,
  "$source": "nmea0183.GP",
  "timestamp": "2024-01-17T10:30:00.000Z",
  "meta": {
    "units": "m/s",
    "description": "Speed over ground"
  }
}
```

### 1.3 Multi-Source Handling

When multiple sources provide the same path:
- First source becomes default (`value` field)
- All sources stored in `values` object keyed by source ID
- Clients can subscribe to specific sources: `navigation.speedOverGround.values[n2k.115]`

---

## 2. WebSocket Protocol

### 2.1 Connection

**Endpoint:** `ws://host:port/signalk/v1/stream`

**Query Parameters:**
| Parameter | Values | Default | Description |
|-----------|--------|---------|-------------|
| `subscribe` | `self`, `all`, `none` | `self` | Initial subscription |
| `sendCachedValues` | `true`, `false` | `true` | Send current state on connect |

### 2.2 Hello Message (Server → Client)

```json
{
  "name": "signalk-server-rust",
  "version": "1.7.0",
  "timestamp": "2024-01-17T10:30:00.000Z",
  "self": "vessels.urn:mrn:signalk:uuid:...",
  "roles": ["main", "master"]
}
```

### 2.3 Delta Message Format

```json
{
  "context": "vessels.self",
  "updates": [{
    "$source": "nmea0183.GP",
    "timestamp": "2024-01-17T10:30:00.000Z",
    "values": [
      { "path": "navigation.speedOverGround", "value": 3.85 },
      { "path": "navigation.courseOverGroundTrue", "value": 1.52 }
    ]
  }]
}
```

### 2.4 Subscription Protocol

**Subscribe Message:**
```json
{
  "context": "vessels.self",
  "subscribe": [{
    "path": "navigation.*",
    "period": 1000,
    "format": "delta",
    "policy": "ideal",
    "minPeriod": 200
  }]
}
```

**Unsubscribe Message:**
```json
{
  "context": "*",
  "unsubscribe": [{ "path": "*" }]
}
```

**Path Wildcards:**
- `*` - match any single segment
- `propulsion/*/oilTemperature` - any engine's oil temp
- `navigation.*` - all navigation paths

**Policy Options:**
| Policy | Behavior |
|--------|----------|
| `instant` | Send immediately (throttled by `minPeriod`) |
| `ideal` | Instant + resend last value if no update within `period` |
| `fixed` | Send at regular `period` intervals |

### 2.5 PUT Request Protocol

```json
{
  "context": "vessels.self",
  "requestId": "uuid",
  "put": {
    "path": "steering.autopilot.target.headingTrue",
    "value": 1.52
  }
}
```

---

## 3. Reference Implementation Analysis (Node.js)

### 3.1 Architecture Overview

```
Server (main class)
├── Express HTTP server
├── Primus WebSocket abstraction
├── app (mixed-in interfaces)
│   ├── signalk (FullSignalK data model)
│   ├── deltaCache (caches all deltas)
│   ├── streambundle (BaconJS reactive streams)
│   ├── subscriptionmanager
│   ├── interfaces (ws, rest, tcp)
│   ├── providers (piped data sources)
│   └── security (JWT, ACLs)
└── intervals (cleanup/stats timers)
```

### 3.2 Delta Processing Pipeline

```
Incoming Delta
    ↓
handleMessage(providerId, delta)
    ↓
DeltaChain (registered handlers)
    ↓
FullSignalK.addDelta() → merge into tree
    ↓
StreamBundle.pushDelta() → BaconJS buses
    ↓
DeltaCache.onValue() → store for replay
    ↓
SubscriptionManager → filter & send to clients
```

### 3.3 Key Implementation Patterns

**Delta Cache Structure:**
```typescript
cache[context][path][source] = {
  context, path, value, $source, timestamp, isMeta
}
```

**Source Priority System:**
- Per-path source precedence configuration
- Time-based fallover (if preferred source stale, accept lower priority)
- Reduces conflicts from multiple NMEA sources

**Subscription Filtering:**
- Regex-based path matching
- Context filtering with glob patterns
- Period/minPeriod sampling via BaconJS operators

### 3.4 Memory Considerations

- String interning for context.path splits
- Object references shared (need Arc/Rc in Rust)
- Streaming-based, never fully load deltas
- Backpressure monitoring on client send buffers

---

## 4. Existing Rust Spike Analysis (signalk-server-esp)

### 4.1 Current State

**Implemented:**
- WiFi connectivity (WPA2 Personal)
- WebSocket client connection to SignalK server
- Device authentication flow (OAuth2-like)
- Token storage in NVS
- Sensor abstraction with Observable pattern
- Basic delta message sending (hardcoded values)

**Dependencies:**
- `esp-idf-svc`, `esp-idf-hal` - ESP32 framework
- `signalk` crate (v0.6.3) - delta building
- `eyeball` - observable state pattern
- `smol` - async runtime (not fully integrated)
- `serde`, `serde_json` - serialization

### 4.2 Architecture Strengths

- Type-safe state machine: `New → Initialized → Running`
- Clean sensor abstraction with `Observable<T>`
- Working authentication flow
- Good error handling with `anyhow::Result`

### 4.3 Gaps to Address

| Area | Current State | Needed |
|------|---------------|--------|
| Sensor integration | Hardcoded values | Read from attached sensors |
| Async execution | Blocking loop | Integrate `smol` or `embassy` |
| Reconnection | None | Auto-reconnect on failure |
| Configuration | Compile-time only | Runtime config via AP mode |
| Server mode | Client only | Full server with subscriptions |
| Path mapping | Single hardcoded path | Dynamic path configuration |

---

## 5. Plugin Compatibility Architecture

### 5.1 Design Goals

1. **Run existing plugins with minimal changes** - no complete rewrites
2. **Rust core for performance** - data model, WebSocket, delta processing
3. **Deno for JS plugins** - leverages npm compatibility, secure sandbox
4. **Clean API boundary** - Rust ↔ Deno communication via message passing

### 5.2 Core Plugin API Subset (Phase 1)

Based on analysis of the `ServerAPI` interface and common plugin patterns, these are the essential methods to support:

**Data Access (read)**
```typescript
getSelfPath(path: string): any        // Read from vessels.self
getPath(path: string): any            // Read from any context
getMetadata(path: string): Metadata   // Get path metadata
```

**Data Emission (write)**
```typescript
handleMessage(id: string, delta: object, version?: SKVersion): void
```

**Stream Subscription**
```typescript
// Simplified reactive subscription (replaces BaconJS streambundle)
subscribe(path: string, callback: (value: any) => void): Unsubscribe
subscribeSelf(path: string, callback: (value: any) => void): Unsubscribe
```

**PUT/Action Handlers**
```typescript
registerPutHandler(context: string, path: string, callback: PutHandler, source: string): void
```

**Delta Interception**
```typescript
registerDeltaInputHandler(handler: DeltaInputHandler): void
```

**Logging & Status**
```typescript
debug(msg: string): void
error(msg: string): void
setPluginStatus(msg: string): void
setPluginError(msg: string): void
```

**Configuration**
```typescript
savePluginOptions(config: object, cb: Callback): void
readPluginOptions(): object
getDataDirPath(): string
```

### 5.3 Deferred API (Phase 2+)

- `registerWithRouter()` - HTTP routes (need HTTP server in Rust)
- `registerResourceProvider()` - Resource API
- `registerAutopilotProvider()` - Autopilot API
- `getCourse()` / `setDestination()` - Course API
- `getSerialPorts()` - Hardware access

### 5.4 BaconJS Replacement Strategy

The original server uses BaconJS for reactive streams. For Deno compatibility:

**Option A: Simple callback subscription (recommended for Phase 1)**
```typescript
// Plugin code change: minimal
// Before (BaconJS):
app.streambundle.getSelfBus('navigation.position').onValue(pos => { ... })

// After (callback):
app.subscribeSelf('navigation.position', pos => { ... })
```

**Option B: Provide RxJS adapter**
```typescript
// For plugins that heavily use FRP patterns
import { fromSignalK } from '@signalk/deno-compat'
const position$ = fromSignalK(app, 'navigation.position')
```

### 5.5 Rust-Deno Bridge Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Rust Core Server                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ Data Model  │  │  WebSocket  │  │  Subscription Manager   │ │
│  │   (Store)   │  │   Server    │  │  (path → subscribers)   │ │
│  └──────┬──────┘  └──────┬──────┘  └────────────┬────────────┘ │
│         │                │                      │               │
│  ┌──────┴────────────────┴──────────────────────┴─────────────┐│
│  │                    Delta Processor                          ││
│  │  (ingest → chain handlers → merge → broadcast)              ││
│  └──────────────────────────┬──────────────────────────────────┘│
│                             │                                   │
│  ┌──────────────────────────┴──────────────────────────────────┐│
│  │                   Plugin Bridge (FFI/IPC)                   ││
│  │  - Message serialization (JSON or MessagePack)              ││
│  │  - Async command/response                                   ││
│  │  - Event subscriptions                                      ││
│  └──────────────────────────┬──────────────────────────────────┘│
└─────────────────────────────┼───────────────────────────────────┘
                              │
                              │ (Unix socket / stdio / FFI)
                              │
┌─────────────────────────────┼───────────────────────────────────┐
│                        Deno Runtime                             │
├─────────────────────────────┼───────────────────────────────────┤
│  ┌──────────────────────────┴──────────────────────────────────┐│
│  │                   ServerAPI Shim                            ││
│  │  - Implements ServerAPI interface                           ││
│  │  - Translates calls to bridge messages                      ││
│  │  - Manages subscriptions                                    ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │  Plugin A   │  │  Plugin B   │  │       Plugin C          │ │
│  │  (npm pkg)  │  │  (npm pkg)  │  │       (npm pkg)         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### 5.6 Bridge Communication Protocol

**Message Types (Rust → Deno):**
```typescript
type ServerMessage =
  | { type: 'delta', delta: Delta }
  | { type: 'subscription_data', path: string, value: any }
  | { type: 'put_request', context: string, path: string, value: any, requestId: string }
  | { type: 'response', requestId: string, result: any }
```

**Message Types (Deno → Rust):**
```typescript
type PluginMessage =
  | { type: 'get_path', path: string, requestId: string }
  | { type: 'handle_message', pluginId: string, delta: Delta }
  | { type: 'subscribe', path: string, subscriptionId: string }
  | { type: 'unsubscribe', subscriptionId: string }
  | { type: 'register_put_handler', context: string, path: string }
  | { type: 'put_response', requestId: string, result: ActionResult }
  | { type: 'log', level: 'debug' | 'error', pluginId: string, msg: string }
```

### 5.7 Plugin Migration Guide (for users)

**Minimal changes required:**

1. **Package.json**: Add `"type": "module"` or rename to `.mjs`

2. **Import style**:
```javascript
// Before (CommonJS)
module.exports = (app) => { ... }

// After (ESM)
export default (app) => { ... }
```

3. **Stream subscriptions**:
```javascript
// Before (BaconJS)
app.streambundle.getSelfBus('navigation.position').onValue(handler)

// After (callback-based)
const unsub = app.subscribeSelf('navigation.position', handler)
// Call unsub() in stop()
```

4. **No changes needed for:**
   - `handleMessage()` calls
   - `getSelfPath()` / `getPath()`
   - `registerPutHandler()`
   - `debug()` / `error()`
   - Configuration schema

---

## 6. Modular Rust Architecture

### 6.1 Design Principles

1. **Layered crates** - Core logic separated from runtime-specific code
2. **Trait-based abstraction** - Abstract over async runtimes and I/O
3. **Feature flags** - Enable/disable capabilities per target
4. **Shared data model** - Same SignalK types everywhere

### 6.2 Crate Structure

```
signalk-rs/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── signalk-core/             # Pure Rust, no async, no I/O
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── model.rs          # Delta, Update, Value, Source, Meta
│   │   │   ├── store.rs          # SignalKStore (in-memory data model)
│   │   │   ├── path.rs           # Path parsing, matching, wildcards
│   │   │   ├── subscription.rs   # Subscription logic (no I/O)
│   │   │   └── validation.rs     # Schema validation
│   │   └── Cargo.toml            # No runtime deps, serde only
│   │
│   ├── signalk-protocol/         # Protocol types and traits
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── messages.rs       # Hello, Subscribe, Unsubscribe, Put
│   │   │   ├── codec.rs          # Frame encoding/decoding
│   │   │   └── traits.rs         # Transport traits (abstract I/O)
│   │   └── Cargo.toml
│   │
│   ├── signalk-server/           # Server implementation (async)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── server.rs         # Main server logic
│   │   │   ├── connection.rs     # Client connection handling
│   │   │   ├── broadcast.rs      # Delta broadcasting
│   │   │   ├── delta_chain.rs    # Delta input handler chain
│   │   │   └── providers/        # Data provider traits
│   │   └── Cargo.toml            # Features: tokio, esp-idf
│   │
│   ├── signalk-plugins/          # Deno plugin bridge (Linux only)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── bridge.rs         # Rust ↔ Deno IPC
│   │   │   ├── loader.rs         # Plugin discovery and loading
│   │   │   ├── api_shim.rs       # ServerAPI implementation
│   │   │   └── lifecycle.rs      # Start/stop/restart
│   │   ├── shim/                 # TypeScript shim for Deno side
│   │   │   ├── mod.ts
│   │   │   └── server_api.ts
│   │   └── Cargo.toml            # tokio only, not esp-idf compatible
│   │
│   └── signalk-providers/        # Data source implementations
│       ├── src/
│       │   ├── lib.rs
│       │   ├── nmea0183.rs       # NMEA 0183 parser
│       │   ├── nmea2000.rs       # NMEA 2000 (canbus)
│       │   └── tcp.rs            # TCP stream input
│       └── Cargo.toml
│
├── bins/
│   ├── signalk-server-linux/     # Full-featured Linux binary
│   │   ├── src/main.rs
│   │   └── Cargo.toml            # Depends on all crates + plugins
│   │
│   └── signalk-server-esp32/     # ESP32 binary (no plugins)
│       ├── src/main.rs
│       └── Cargo.toml            # signalk-server with esp-idf feature
│
└── tests/
    └── integration/              # Spec compliance tests
```

### 6.3 Async Runtime Abstraction

**Strategy:** Use feature flags + conditional compilation, not generic traits.

```rust
// In signalk-server/Cargo.toml
[features]
default = ["tokio-runtime"]
tokio-runtime = ["tokio", "tokio-tungstenite"]
esp-idf-runtime = ["esp-idf-svc", "embedded-svc"]

// In signalk-server/src/server.rs
#[cfg(feature = "tokio-runtime")]
mod tokio_impl {
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    // ...
}

#[cfg(feature = "esp-idf-runtime")]
mod esp_impl {
    use esp_idf_svc::ws::server::EspWebSocketServer;
    // ...
}
```

**Why not generic traits?**
- Async traits are still evolving (RPITIT landed, but ecosystem catching up)
- ESP-IDF and tokio APIs are quite different
- Feature flags are simpler and compile-time zero-cost

### 6.4 Core Data Model (Runtime-Agnostic)

```rust
// signalk-core/src/model.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub updates: Vec<Update>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    #[serde(rename = "$source", skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    pub values: Vec<PathValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValue {
    pub path: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pgn: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub talker: Option<String>,
}
```

### 6.5 Store Trait (Abstract Storage)

```rust
// signalk-core/src/store.rs
pub trait SignalKStore: Send + Sync {
    /// Apply a delta to the store
    fn apply_delta(&mut self, delta: &Delta);

    /// Get value at path (e.g., "vessels.self.navigation.position")
    fn get_path(&self, path: &str) -> Option<&serde_json::Value>;

    /// Get value relative to self vessel
    fn get_self_path(&self, path: &str) -> Option<&serde_json::Value>;

    /// Get full state for a context
    fn get_context(&self, context: &str) -> Option<&serde_json::Value>;

    /// Get the self vessel identifier
    fn self_id(&self) -> &str;
}

/// Default in-memory implementation
pub struct MemoryStore {
    data: serde_json::Value,  // The full SignalK tree
    self_urn: String,
}
```

### 6.6 Server Event Loop (Shared Logic)

```rust
// signalk-server/src/server.rs
use signalk_core::{Delta, SignalKStore};

pub enum ServerEvent {
    DeltaReceived { source: String, delta: Delta },
    ClientConnected { id: u64 },
    ClientDisconnected { id: u64 },
    Subscribe { client_id: u64, paths: Vec<String> },
    Unsubscribe { client_id: u64, paths: Vec<String> },
    PutRequest { client_id: u64, context: String, path: String, value: serde_json::Value },
}

/// Core server logic - runtime agnostic
pub struct SignalKServer<S: SignalKStore> {
    store: S,
    subscriptions: SubscriptionManager,
    delta_handlers: DeltaHandlerChain,
    // ...
}

impl<S: SignalKStore> SignalKServer<S> {
    pub fn handle_event(&mut self, event: ServerEvent) -> Vec<ServerAction> {
        match event {
            ServerEvent::DeltaReceived { source, delta } => {
                // Run through delta handler chain
                let delta = self.delta_handlers.process(delta);
                // Apply to store
                self.store.apply_delta(&delta);
                // Determine who to notify
                self.subscriptions.get_broadcasts(&delta)
            }
            // ...
        }
    }
}
```

### 6.7 Platform-Specific Binaries

**Linux (full features):**
```rust
// bins/signalk-server-linux/src/main.rs
use signalk_server::SignalKServer;
use signalk_plugins::DenoPluginHost;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config()?;
    let store = MemoryStore::new(&config.self_urn);
    let server = SignalKServer::new(store);

    // Start plugin host (Deno)
    let plugins = DenoPluginHost::new(&config.plugin_dir)?;
    plugins.start_all(&server).await?;

    // Start WebSocket server
    let listener = TcpListener::bind(&config.bind_addr).await?;
    run_server(listener, server).await
}
```

**ESP32 (no plugins):**
```rust
// bins/signalk-server-esp32/src/main.rs
use signalk_server::SignalKServer;
use esp_idf_svc::wifi::EspWifi;
use esp_idf_svc::ws::server::EspWebSocketServer;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();

    let config = load_config_from_nvs()?;
    let store = MemoryStore::new(&config.self_urn);
    let server = SignalKServer::new(store);

    // Connect WiFi
    let _wifi = connect_wifi(&config)?;

    // Start WebSocket server (no plugins on ESP32)
    run_esp_server(server)
}
```

### 6.8 Shared vs Platform-Specific Code

| Component | Shared | Linux-Only | ESP32-Only |
|-----------|--------|------------|------------|
| Data model (Delta, Update, etc.) | ✓ | | |
| Store implementation | ✓ | | |
| Path matching | ✓ | | |
| Subscription logic | ✓ | | |
| Delta handler chain | ✓ | | |
| WebSocket framing | ✓ | | |
| TCP/networking | | tokio | esp-idf-svc |
| Plugin system | | ✓ (Deno) | |
| Configuration | | File-based | NVS |
| HTTP/REST API | | axum | esp-idf http |

---

## 7. Implementation Phases

### Phase 1: Workspace & Core Crate
- [ ] Create `signalk-rs` workspace with crate structure
- [ ] `signalk-core`: Data model structs (Delta, Update, Value, Source, Meta)
- [ ] `signalk-core`: Path parsing and wildcard matching
- [ ] `signalk-core`: MemoryStore implementation
- [ ] Unit tests against spec JSON examples
- [ ] CI setup (GitHub Actions for Linux + ESP32 cross-compile check)

### Phase 2: Protocol & Server Crate (Linux/tokio first)
- [ ] `signalk-protocol`: Message types (Hello, Subscribe, Put, etc.)
- [ ] `signalk-server`: WebSocket listener (tokio-tungstenite)
- [ ] Hello message on connect
- [ ] Delta broadcasting to connected clients
- [ ] Basic subscription handling (`subscribe=all/none/self`)
- [ ] Integration test with SignalK client library

### Phase 3: Full Subscription Management
- [ ] Path pattern matching with wildcards
- [ ] Per-client subscription tracking
- [ ] Policy implementation (instant/ideal/fixed)
- [ ] Period/minPeriod throttling
- [ ] Delta cache for `sendCachedValues=true`

### Phase 4: ESP32 Port (Parallel Track)
- [ ] `signalk-server`: Add `esp-idf-runtime` feature
- [ ] ESP32 WebSocket server implementation
- [ ] `signalk-server-esp32` binary crate
- [ ] WiFi configuration (NVS-based)
- [ ] Test on hardware with SignalK client

### Phase 5: Deno Plugin Bridge (Linux)
- [ ] `signalk-plugins`: Deno subprocess management
- [ ] JSON IPC protocol implementation
- [ ] TypeScript ServerAPI shim
- [ ] Plugin discovery (`signalk-node-server-plugin` keyword)
- [ ] Plugin lifecycle (start/stop/restart)

### Phase 6: Core Plugin API Implementation
- [ ] `getSelfPath()` / `getPath()` - sync data access
- [ ] `handleMessage()` - delta emission from plugins
- [ ] `subscribeSelf()` / `subscribe()` - callback subscriptions
- [ ] `registerPutHandler()` - action handlers
- [ ] `registerDeltaInputHandler()` - delta interception
- [ ] Logging, status, and config persistence
- [ ] Test with 2-3 real SignalK plugins

### Phase 7: Data Providers
- [ ] `signalk-providers`: Provider trait abstraction
- [ ] NMEA 0183 sentence parsing
- [ ] TCP/UDP stream input
- [ ] Source priority system

### Phase 8: Production Hardening
- [ ] REST API (axum on Linux, esp-idf-http on ESP32)
- [ ] HTTP routes for plugins (`registerWithRouter`)
- [ ] Security/authentication (JWT)
- [ ] Configuration file support (YAML/TOML)
- [ ] mDNS discovery
- [ ] Systemd service integration
- [ ] Documentation and examples

---

## 8. Key Technical Challenges

### 8.1 Rust-Deno Bridge Performance
- IPC overhead for high-frequency deltas
- Consider batching for subscription callbacks
- Measure latency: target <5ms for plugin round-trip
- Option: `deno_core` embedding vs subprocess (tradeoffs)

### 8.2 Plugin Compatibility
- BaconJS → callback migration path
- CommonJS → ESM module conversion
- Express router compatibility (Phase 2)
- Test with popular real-world plugins

### 8.3 Real-time Performance
- Target: <10ms delta processing latency (Rust core)
- Non-blocking async throughout
- Efficient path matching (trie or radix tree)
- Minimize allocations in hot path

### 8.4 Protocol Compatibility
- Must pass SignalK compliance tests
- Handle malformed input gracefully
- Full v1.7.0 spec compliance

---

## 9. Crate Selection

| Crate | Purpose | Notes |
|-------|---------|-------|
| `tokio` | Async runtime | Industry standard, excellent perf |
| `tokio-tungstenite` | WebSocket | Async WS on tokio |
| `axum` | HTTP server | Ergonomic, tower-based |
| `serde` / `serde_json` | Serialization | Required |
| `tracing` | Logging | Structured, async-aware |
| `dashmap` | Concurrent HashMap | For shared state |
| `regex` | Path matching | For subscription wildcards |
| `uuid` | UUID generation | For request IDs, vessel IDs |
| `chrono` | Timestamps | ISO 8601 handling |

**For Deno integration (choose one):**
| Approach | Crate/Method | Tradeoffs |
|----------|--------------|-----------|
| Subprocess | `tokio::process` | Simple, isolated, some IPC overhead |
| Embedded | `deno_core` | Tighter integration, more complex |

---

## 10. Next Steps

1. **Create new repository** - `signalk-server-rs` with clean structure
2. **Phase 1 implementation** - Data model, delta parsing, basic tests
3. **Phase 2 implementation** - WebSocket server, hello message, broadcasting
4. **Validate against spec** - Use SignalK compliance test suite
5. **Phase 4 implementation** - Deno bridge prototype

---

## 11. Open Questions

1. **Deno integration approach:** Subprocess (simpler) vs `deno_core` embedding (tighter)?
2. **BaconJS compatibility:** Provide shim or require migration to callbacks?
3. **Security:** Authentication required for initial version?
4. **Multiple vessels:** Support AIS targets or self-only initially?
5. **Config format:** YAML (like original) or TOML (Rust ecosystem)?

---

## Appendix A: Reference Links

**SignalK:**
- [SignalK Specification 1.7.0](https://signalk.org/specification/1.7.0/doc/)
- [SignalK Schema Repository](https://github.com/SignalK/signalk-schema)
- [Reference Server (Node.js)](https://github.com/SignalK/signalk-server)
- [Plugin Development Docs](https://demo.signalk.org/documentation/Developing/Plugins.html)
- [Server Plugin API](https://github.com/SignalK/signalk-server/tree/master/packages/server-api)

**Rust Ecosystem:**
- [tokio](https://tokio.rs/) - Async runtime
- [axum](https://github.com/tokio-rs/axum) - Web framework
- [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite) - WebSocket

**Deno:**
- [Deno](https://deno.com/) - JavaScript/TypeScript runtime
- [Deno Node Compatibility](https://docs.deno.com/runtime/fundamentals/node/)
- [deno_core crate](https://crates.io/crates/deno_core) - Embed Deno in Rust

---

## Appendix B: Example Messages

### Hello Response
```json
{
  "name": "signalk-server-rust",
  "version": "1.7.0",
  "self": "vessels.urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d",
  "roles": ["main"],
  "timestamp": "2024-01-17T10:30:00.000Z"
}
```

### Navigation Delta
```json
{
  "context": "vessels.self",
  "updates": [{
    "$source": "nmea0183.GP",
    "timestamp": "2024-01-17T10:30:00.500Z",
    "values": [
      {"path": "navigation.position", "value": {"latitude": 52.0987654, "longitude": 4.98765245}},
      {"path": "navigation.speedOverGround", "value": 3.85},
      {"path": "navigation.courseOverGroundTrue", "value": 1.52}
    ]
  }]
}
```

### Subscription Request
```json
{
  "context": "vessels.self",
  "subscribe": [
    {"path": "navigation.position", "period": 1000, "policy": "instant"},
    {"path": "environment.wind.*", "period": 500, "policy": "ideal"}
  ]
}
```
