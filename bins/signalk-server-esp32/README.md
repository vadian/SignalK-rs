# SignalK Server for ESP32

A minimal Signal K server implementation for ESP32 (Xtensa) microcontrollers with real-time delta streaming.

## Features

- **Real-time WebSocket streaming** at `ws://<ip>/signalk/v1/stream`
  - Hello message on connect
  - Full model sent on connect (sendCachedValues)
  - Live delta broadcasting via `EspHttpWsDetachedSender`
  - Subscribe/unsubscribe message support for filtering
  - Path pattern matching with wildcards (e.g., `navigation.*`)
  - **Rate limiting with minPeriod/period** to prevent socket overload
- **REST API**
  - `GET /signalk/v1/api` - Full data model
  - `GET /signalk/v1/api/vessels/self/navigation/position` - Path queries
- **Discovery endpoint** at `http://<ip>/signalk`
- **Demo data generator** for testing (position, SOG, COG updates every second)

## Prerequisites

1. **Install Rust ESP toolchain** using `espup`:
   ```bash
   cargo install espup
   espup install
   ```

2. **Source the ESP environment** (add to your shell profile):
   ```bash
   . $HOME/export-esp.sh
   ```

   Or add to `~/.bashrc` / `~/.zshrc`:
   ```bash
   [ -f "$HOME/export-esp.sh" ] && . "$HOME/export-esp.sh"
   ```

3. **Install flash tool**:
   ```bash
   cargo install espflash cargo-espflash
   ```

## Building and Running

**Important:** You must run these commands from the `bins/signalk-server-esp32` directory, not from the workspace root. The directory contains a `rust-toolchain.toml` that selects the ESP Rust toolchain automatically.

```bash
cd bins/signalk-server-esp32

# Set WiFi credentials (required at build time)
export WIFI_SSID="YourNetworkName"
export WIFI_PASSWORD="YourPassword"

# Build, flash, and monitor in one command
cargo run --release
```

The `cargo run` command uses `cargo-espflash` (configured in `.cargo/config.toml`) to:
1. Build the project
2. Flash the binary to the connected ESP32
3. Open a serial monitor to view logs

### Manual Flashing

If you prefer to flash manually or use different options:

```bash
# Build first
cargo build --release

# Flash with espflash directly
espflash flash --baud=921600 --monitor target/xtensa-esp32-espidf/release/signalk-server-esp32

# Or just monitor without flashing
espflash monitor
```

## Configuration

### WiFi Credentials

WiFi credentials are set at build time via environment variables:

```bash
export WIFI_SSID="YourNetwork"
export WIFI_PASSWORD="YourPassword"
```

### sdkconfig.defaults

ESP-IDF settings are in `sdkconfig.defaults` **at the workspace root** (see Known Issues below):

| Setting | Value | Purpose |
|---------|-------|---------|
| `CONFIG_ESP_MAIN_TASK_STACK_SIZE` | 16384 | Main task stack (Rust needs more than C default) |
| `CONFIG_PTHREAD_STACK_MIN` | 16384 | Minimum pthread stack for spawned threads |
| `CONFIG_HTTPD_WS_SUPPORT` | y | Enable WebSocket support |
| `CONFIG_LWIP_MAX_SOCKETS` | 16 | Allow multiple connections |

### Known Issue: sdkconfig.defaults Location

**The `esp-idf-sys` build system uses `sdkconfig.defaults` from the workspace root, NOT from this directory.**

This is because `esp-idf-sys` resolves relative paths from the "workspace directory" (the directory containing `Cargo.lock` and `target/`). Even when building from within `bins/signalk-server-esp32/`, the workspace root's `sdkconfig.defaults` is used.

To work around this:
- The ESP-IDF configuration is in `/sdkconfig.defaults` at the workspace root
- You can override with `ESP_IDF_SDKCONFIG_DEFAULTS` environment variable:
  ```bash
  export ESP_IDF_SDKCONFIG_DEFAULTS="bins/signalk-server-esp32/sdkconfig.defaults"
  cargo build --release
  ```

See [esp-idf-sys BUILD-OPTIONS.md](https://github.com/esp-rs/esp-idf-sys/blob/master/BUILD-OPTIONS.md) for details.

## Architecture

This binary uses the same shared crates as the Linux version:

| Crate | Description |
|-------|-------------|
| `signalk-core` | Data model, MemoryStore (platform-agnostic) |
| `signalk-protocol` | Message types, Delta, HelloMessage (platform-agnostic) |
| `signalk-esp32` | ESP32-specific WiFi, config, HTTP utilities |

### Delta Broadcast Architecture

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
│  │  WS  /signalk/v1/stream - WebSocket (register)     │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Key Implementation Details

- **Delta Broadcast**: Uses `EspHttpWsDetachedSender` to push deltas from the processor thread to WebSocket clients. This leverages ESP-IDF's `httpd_ws_send_frame_async` under the hood.
- **Client Tracking**: Connected clients are stored in `Arc<Mutex<HashMap<i32, EspHttpWsDetachedSender>>>` keyed by socket fd.
- **Thread Stack Sizes**: All spawned threads use 16KB stack (`std::thread::Builder::stack_size`) to match `CONFIG_PTHREAD_STACK_MIN`.
- **HTTP Handler Stack**: Set to 16KB to accommodate JSON serialization of the full model.

### Differences from Linux

| Component | Linux | ESP32 |
|-----------|-------|-------|
| HTTP Server | Axum (async) | esp-idf-svc (blocking) |
| Delta Broadcast | `broadcast::channel` | `EspHttpWsDetachedSender` |
| Concurrency | `tokio::spawn` | `std::thread::Builder` |
| Sync primitives | `RwLock` | `Mutex` |
| Config Storage | Filesystem | NVS (planned) |
| Admin UI | Full React | None |
| Port | 4000 | 80 |

## Memory Usage

Approximate usage on ESP32:

- **Binary size**: ~500KB (release, optimized for size)
- **RAM usage**: ~80KB typical
- **Stack**: 16KB main task, 16KB per spawned thread
- **Per-client subscription overhead**: ~200 bytes base + ~40 bytes per throttled pattern

## Troubleshooting

### Build fails with "esp channel not found"

Make sure you've sourced the ESP environment:
```bash
. $HOME/export-esp.sh
```

### Thread spawn fails with assertion error (left: 22, right: 0)

This means the thread stack size is below `CONFIG_PTHREAD_STACK_MIN`. Ensure:
1. `sdkconfig.defaults` has `CONFIG_PTHREAD_STACK_MIN=16384`
2. All `std::thread::Builder` calls use `.stack_size(16 * 1024)`

### WiFi connection fails

1. Check SSID/password are correct
2. Ensure network is 2.4GHz (ESP32 doesn't support 5GHz)
3. Check serial monitor for detailed error messages

### WebSocket clients can't connect

1. Verify IP address in serial monitor output
2. Check firewall settings
3. Ensure ESP32 and client are on same network

### Stack overflow / Guru Meditation errors

Increase stack sizes in `sdkconfig.defaults`:
```
CONFIG_ESP_MAIN_TASK_STACK_SIZE=16384
CONFIG_PTHREAD_STACK_MIN=16384
```

Also ensure HTTP handler stack is sufficient:
```rust
let http_config = HttpConfig {
    stack_size: 16384,
    ..Default::default()
};
```

## Project Structure

```
bins/signalk-server-esp32/
├── .cargo/
│   └── config.toml       # Build target, runner (espflash), linker settings
├── src/
│   └── main.rs           # Application entry point
├── build.rs              # ESP-IDF build script
├── Cargo.toml            # Dependencies
├── rust-toolchain.toml   # ESP Rust toolchain selector
├── sdkconfig.defaults    # ESP-IDF configuration (NOTE: workspace root used!)
└── README.md             # This file
```

## Testing

### Basic Connection

Connect with websocat to verify delta streaming:

```bash
websocat ws://<esp32-ip>/signalk/v1/stream
```

You should see:
1. Hello message with server info
2. Full model with current state
3. Delta updates every second (from demo generator)

### Subscription Filtering

After connecting, you can send subscribe/unsubscribe messages to filter deltas:

```bash
# Connect
websocat ws://<esp32-ip>/signalk/v1/stream

# Then send subscription message (paste into websocat):
{"context":"vessels.self","subscribe":[{"path":"navigation.position"}]}

# Now you'll only receive position updates, not SOG/COG

# Unsubscribe from all:
{"context":"*","unsubscribe":[{"path":"*"}]}
```

Path patterns support wildcards:
- `navigation.*` - All navigation paths
- `propulsion.*.revolutions` - Any engine's revolutions
- `*` - Everything

### Rate Limiting (Throttling)

To prevent socket overload on the ESP32, subscriptions support `minPeriod` and `period` parameters that limit how often updates are sent for a given path:

```bash
# Subscribe to position updates at most once per second (1000ms)
{"context":"vessels.self","subscribe":[{"path":"navigation.position","minPeriod":1000}]}

# Subscribe to multiple paths with different rates
{"context":"vessels.self","subscribe":[
  {"path":"navigation.position","minPeriod":1000},
  {"path":"navigation.speedOverGround","minPeriod":500},
  {"path":"environment.*","minPeriod":5000}
]}
```

| Parameter | Description |
|-----------|-------------|
| `minPeriod` | Minimum milliseconds between updates (rate limit) |
| `period` | Desired milliseconds between updates (hint) |

**Why throttling matters on ESP32:**
- Limited socket buffers
- Single-core or dual-core with limited RAM
- Prevents overwhelming slow network connections
- Recommended: Use `minPeriod: 1000` (1 second) for most data on embedded

### REST API Testing

```bash
# Full model
curl http://<esp32-ip>/signalk/v1/api

# Specific path (use / instead of . in URL)
curl http://<esp32-ip>/signalk/v1/api/vessels/self/navigation/position

# Discovery
curl http://<esp32-ip>/signalk
```

## Future Work

- [ ] NVS configuration storage
- [ ] SNTP time synchronization
- [ ] mDNS service discovery
- [ ] NMEA 0183 input (UART)
- [ ] NMEA 2000 input (CAN)
- [ ] Simple HTML status page
