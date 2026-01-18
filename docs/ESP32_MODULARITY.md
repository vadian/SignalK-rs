# ESP32 Deployment Strategy & Modularity Analysis

**Date:** January 17, 2026  
**Context:** Analyzing impact of unified Linux server architecture on ESP32 deployment

## TL;DR: Modularity is EXCELLENT ✅

The unified Axum architecture on Linux **does not hurt ESP32 deployment** because:
1. **Core business logic is fully shared** (signalk-core, signalk-protocol)
2. **Platform differences are isolated to binary crates** (main.rs files)
3. **Each platform uses appropriate tools** (Axum for Linux, esp-idf for ESP32)

This is **exactly how cross-platform Rust projects should work**.

---

## Modularity Analysis

### What's Shared (Platform-Agnostic) ✅

These crates work on **both Linux and ESP32** without modification:

| Crate | Purpose | ESP32 Ready? |
|-------|---------|--------------|
| **signalk-core** | Data model, MemoryStore, path matching | ✅ Yes - pure Rust, no async, no I/O |
| **signalk-protocol** | Message types (Delta, Hello, Subscribe) | ✅ Yes - just serde structs |
| **signalk-providers** | NMEA parsers (planned) | ✅ Yes - pure parsing logic |

**Key Design Win:**
```rust
// This code works IDENTICALLY on Linux and ESP32
let mut store = MemoryStore::new("urn:mrn:signalk:uuid:...");
store.apply_delta(&delta);
let value = store.get_path("navigation.position");
```

### What's Platform-Specific 🔧

These differ between Linux and ESP32:

| Component | Linux | ESP32 |
|-----------|-------|-------|
| **HTTP Server** | Axum | esp-idf-svc (httpd) |
| **WebSocket** | axum::extract::ws | esp-idf-svc (websocket) |
| **Async Runtime** | Tokio | esp-idf-svc (FreeRTOS) |
| **File Serving** | tower-http::ServeDir | None (no admin UI) |
| **Config Storage** | FileStorage (JSON files) | NvsStorage (flash) |
| **Plugins** | Deno runtime | None |

**This is NORMAL and EXPECTED.** ESP32 has different capabilities and constraints.

---

## Architecture Comparison

### Linux: Unified Axum Server

```
┌─────────────────────────────────────────────────────────┐
│         Axum Server (Port 3001) - Linux                 │
│  ┌─────────────────────────────────────────────────┐   │
│  │  HTTP + WebSocket Routes                        │   │
│  │  - /signalk/v1/stream (WebSocket)              │   │
│  │  - /signalk/v1/api (REST)                      │   │
│  │  - /admin/ (Static UI)                         │   │
│  └─────────────────────────────────────────────────┘   │
│              ▼                                          │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Shared Core Logic                              │   │
│  │  - signalk-core::MemoryStore                   │   │
│  │  - signalk-protocol::Delta                     │   │
│  │  - Delta processing & broadcast                │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### ESP32: Separate HTTP + WebSocket

```
┌─────────────────────────────────────────────────────────┐
│      esp-idf-svc Servers - ESP32                        │
│  ┌───────────────┐        ┌───────────────────────┐    │
│  │ HTTP Server   │        │ WebSocket Server      │    │
│  │ Port 80       │        │ Port 3000             │    │
│  │ (minimal API) │        │ (delta stream only)   │    │
│  └───────────────┘        └───────────────────────┘    │
│         ▼                            ▼                  │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Shared Core Logic (SAME CODE)                  │   │
│  │  - signalk-core::MemoryStore                   │   │
│  │  - signalk-protocol::Delta                     │   │
│  │  - Delta processing & broadcast                │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**Key Insight:** The bottom box (shared core logic) is IDENTICAL. Only the top (server infrastructure) differs.

---

## ESP32 Implementation Strategy

### 1. Minimal HTTP Server (Discovery + Simple UI)

ESP32 has limited resources, so we'll implement:
- Discovery endpoint (`/signalk`) - required by spec
- Optional basic API endpoint (`/signalk/v1/api`)
- **Simple HTML UI** (11KB) - our lightweight admin page ✅
- NO full React UI (34MB - too large) ❌
- See [ESP32_WEB_UI_ANALYSIS.md](ESP32_WEB_UI_ANALYSIS.md) for size analysis

**Good news:** Our simple HTML UI (11KB) fits comfortably on any ESP32 variant and provides all essential features:
- Live navigation data
- WebSocket status
- API access
- Loads in <0.1 seconds

### 2. WebSocket-Only Data Streaming

Focus on what ESP32 does best:
- Accept WebSocket connections
- Stream delta updates
- Handle subscriptions (no complex filtering yet)
- Low memory footprint

### 3. Shared Business Logic

All the hard work is in shared crates:
```rust
// signalk-core (works everywhere)
pub trait SignalKStore {
    fn apply_delta(&mut self, delta: &Delta);
    fn get_path(&self, path: &str) -> Option<Value>;
    // ... etc
}

// signalk-protocol (works everywhere)
pub struct Delta { ... }
pub struct HelloMessage { ... }
```

---

## Code Structure

### Linux Binary (`bins/signalk-server-linux/src/main.rs`)

```rust
// Platform-specific imports
use axum::{Router, routing::get};
use tower_http::services::ServeDir;
use tokio::sync::{broadcast, RwLock};

// Shared imports
use signalk_core::{Delta, MemoryStore, SignalKStore};
use signalk_protocol::{HelloMessage, ServerMessage};

#[tokio::main]
async fn main() {
    // Linux-specific: Unified Axum server
    let app = Router::new()
        .route("/signalk/v1/stream", get(websocket_handler))
        .route("/signalk/v1/api", get(full_api_handler))
        .nest_service("/admin", ServeDir::new(admin_ui_path));
    
    // Shared: Store and delta processing
    let store = Arc::new(RwLock::new(MemoryStore::new(urn)));
    let (delta_tx, _) = broadcast::channel::<Delta>(1024);
    
    // Platform-specific: Tokio async
    tokio::spawn(async move {
        while let Some(delta) = event_rx.recv().await {
            store.write().await.apply_delta(&delta);
            delta_tx.send(delta);
        }
    });
}
```

### ESP32 Binary (`bins/signalk-server-esp32/src/main.rs`)

```rust
// Platform-specific imports
use esp_idf_svc::http::server::EspHttpServer;
use esp_idf_svc::ws::server::EspWsServer;
use std::sync::{Arc, Mutex};

// Shared imports (SAME AS LINUX!)
use signalk_core::{Delta, MemoryStore, SignalKStore};
use signalk_protocol::{HelloMessage, ServerMessage};

fn main() {
    // ESP32-specific: esp-idf initialization
    esp_idf_sys::link_patches();
    
    // Shared: Store and delta processing (SAME LOGIC!)
    let store = Arc::new(Mutex::new(MemoryStore::new(urn)));
    let (delta_tx, delta_rx) = std::sync::mpsc::channel::<Delta>();
    
    // ESP32-specific: FreeRTOS task
    std::thread::spawn(move || {
        while let Ok(delta) = delta_rx.recv() {
            store.lock().unwrap().apply_delta(&delta);
            // Broadcast to WebSocket clients
        }
    });
    
    // ESP32-specific: HTTP server
    let server = EspHttpServer::new(&Default::default())?;
    server.fn_handler("/signalk", Method::Get, |req| {
        // Discovery endpoint
    })?;
    
    // ESP32-specific: WebSocket server
    let ws_server = EspWsServer::new()?;
    ws_server.set_on_connect(|client_id| {
        // Send HelloMessage (using shared protocol types!)
    });
}
```

---

## What Changed with Unified Linux Architecture?

### Before (Planned)

```
Linux: SignalKServer (tokio-tungstenite) + separate HTTP server
ESP32: SignalKServer (esp-idf) - reuse the same abstraction
```

### After (Current)

```
Linux: Integrated Axum server (better for Linux use case)
ESP32: Custom esp-idf server (better for ESP32 use case)
```

### Impact: ✅ POSITIVE

**Old approach:**
- Tried to share server infrastructure across platforms
- Would require complex abstractions and feature flags
- Neither platform would be optimal

**New approach:**
- Share business logic (data model, delta processing)
- Each platform uses its best tools
- Simpler, cleaner, more maintainable

This is **better engineering** - you want platform-specific code at the edges (I/O, networking) and shared code in the core (business logic).

---

## Modularity Checklist

| Concern | Status | Notes |
|---------|--------|-------|
| **Can ESP32 use signalk-core?** | ✅ Yes | No dependencies, pure Rust |
| **Can ESP32 use signalk-protocol?** | ✅ Yes | Just serde structs |
| **Does ESP32 need Axum?** | ❌ No | Uses esp-idf-svc instead |
| **Does ESP32 need Tokio?** | ❌ No | Uses FreeRTOS threads |
| **Can ESP32 process deltas?** | ✅ Yes | Same MemoryStore code |
| **Can ESP32 speak SignalK protocol?** | ✅ Yes | Same message types |
| **Will ESP32 need custom main.rs?** | ✅ Yes | This is normal and expected |

---

## Next Steps for ESP32 Implementation

### Phase 1: Basic WebSocket Server (1-2 weeks)

1. Set up ESP32 dev environment
2. Implement minimal `main.rs` with esp-idf-svc
3. WebSocket server that:
   - Sends HelloMessage on connect
   - Broadcasts delta updates
   - No subscriptions yet (send everything)
4. Test with demo data generator

### Phase 2: HTTP Discovery (1 week)

1. Add HTTP server with `/signalk` endpoint
2. Implement discovery JSON response
3. Optional: `/signalk/v1/api` for basic data access

### Phase 3: NMEA Input (2 weeks)

1. Serial port reading (UART)
2. NMEA 0183 sentence parsing
3. Convert to Delta messages
4. Feed into shared store

### Phase 4: Configuration (1 week)

1. Implement NvsStorage (flash storage)
2. WiFi credentials
3. Server settings

---

## Recommendations

### ✅ Keep Current Structure

The unified Axum server for Linux is the right choice:
- Simpler deployment
- Better developer experience
- Matches SignalK reference implementation

### ✅ Create ESP32-Specific Binary

Don't try to share server code between Linux and ESP32:
- Different HTTP servers
- Different async models
- Different resource constraints

### ✅ Maximize Shared Core Logic

Focus on keeping these crates platform-agnostic:
- signalk-core
- signalk-protocol
- signalk-providers (parsers)

### 📝 Document Platform Differences

Update ARCHITECTURE.md to show both deployment modes:
- Linux: Unified server approach
- ESP32: Minimal WebSocket-focused approach

---

## Conclusion

**The modularity is excellent.** ✅

Your architectural change to use unified Axum on Linux does not hurt ESP32 deployment at all. In fact, it's better because:

1. **Each platform uses optimal tools** - Axum for Linux, esp-idf for ESP32
2. **Core business logic is fully shared** - MemoryStore, Delta processing, protocol types
3. **No complex abstractions** - Simple, maintainable code

The key insight: **Modularity isn't about sharing server infrastructure code.** It's about sharing business logic and data structures. You're doing that perfectly.

When you're ready to implement ESP32, you'll:
1. Copy the store/delta processing logic pattern from Linux main.rs
2. Wrap it in esp-idf HTTP/WebSocket servers instead of Axum
3. Use the exact same signalk-core and signalk-protocol crates

**This is how it should be done.** 🎯
