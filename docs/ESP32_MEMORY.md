# ESP32 Memory Architecture

This document covers memory management, binary size optimization, and heap constraints for the ESP32 SignalK server.

## Memory Constraints

| Resource | Available | Notes |
|----------|-----------|-------|
| Flash | 4 MB | Partitioned for app + OTA |
| Heap | ~320 KB | ~280 KB after WiFi init |
| Stack | 16 KB | Per task (FreeRTOS) |

## Build Configurations

Two build modes with different partition layouts:

| Aspect | Development | Release |
|--------|-------------|---------|
| Cargo profile | `debug` | `release` |
| Partition | 3MB single factory | 2x 1.5MB OTA |
| Optimization | `-O2` | `-Os` + LTO |
| Logging | INFO | WARN |
| Max binary | < 3 MB | < 1.5 MB |

### Build Commands

```bash
make build-esp         # Dev build (3MB partition)
make build-esp-release # Release build (OTA partitions)
make run-esp           # Build and flash dev
make run-esp-release   # Build and flash release
make esp-size          # Show binary size dev
make esp-size-release  # Show binary size release
```

### Configuration Files

```
signalk-rs/
├── sdkconfig.defaults          # Shared base config
├── sdkconfig.defaults.dev      # Dev overrides (3MB, INFO logging)
├── sdkconfig.defaults.release  # Release overrides (-Os, OTA, WARN logging)
├── partitions.dev.csv          # 3MB single partition
└── partitions.release.csv      # OTA partition layout
```

## Heap Optimizations (Completed)

### Problem: Regex Memory Allocation

The original `PathPattern` implementation used the `regex` crate, which allocates ~200KB to compile even simple patterns like `*`. This exceeded available heap when a WebSocket client connected.

```
regex_automata::nfa::thompson::compiler::Utf8State::clear
  -> alloc::vec::from_elem (200000 bytes)
  -> signalk_core::path::PathPattern::new
```

### Solution: Simple Glob Matching

Replaced regex with segment-based glob matching in `crates/signalk-core/src/path.rs`:

```rust
enum PatternSegment {
    Literal(String),   // exact match
    Wildcard,          // * matches one or more segments
}

pub struct PathPattern {
    raw: String,
    segments: Vec<PatternSegment>,
    trailing_wildcard: bool,
}
```

**Benefits:**
- Zero heap allocation for pattern compilation
- Same code works on Linux and ESP32 (no feature flags needed)
- Supports all SignalK patterns: `*`, `navigation.*`, `propulsion.*.revolutions`

**Result:** WebSocket connections now work without memory allocation failures.

## Flash Size Optimizations

### Cargo Release Profile

`bins/signalk-server-esp32/Cargo.toml`:

```toml
[profile.release]
opt-level = "s"          # Optimize for size
lto = true               # Link-time optimization
codegen-units = 1        # Better optimization
panic = "abort"          # Smaller than unwinding
debug = 1                # Line tables only (keeps backtraces)
```

### ESP-IDF sdkconfig

Key size optimizations in `sdkconfig.defaults.release`:

```
# Compiler optimization
CONFIG_COMPILER_OPTIMIZATION_SIZE=y
CONFIG_COMPILER_CXX_EXCEPTIONS=n
CONFIG_COMPILER_CXX_RTTI=n

# Disable unused components
CONFIG_ETH_ENABLED=n
CONFIG_MQTT_ENABLE=n
CONFIG_FATFS_ENABLED=n
CONFIG_ESP_DRIVER_MCPWM_ENABLED=n
CONFIG_LCD_ENABLE=n
CONFIG_SDMMC_ENABLED=n
CONFIG_I2S_ENABLE=n
```

### Dependencies Removed

| Dependency | Size | Replacement |
|------------|------|-------------|
| `regex` | ~200 KB | Simple glob matching |

## Partition Layouts

### Development (3MB, No OTA)

```
partitions.dev.csv:
nvs,      data, nvs,     0x9000,  0x6000
phy_init, data, phy,     0xf000,  0x1000
factory,  app,  factory, 0x10000, 0x300000   # 3MB app
```

### Release (OTA Enabled)

```
partitions.release.csv:
nvs,      data, nvs,     0x9000,  0x6000
phy_init, data, phy,     0xf000,  0x1000
ota_0,    app,  ota_0,   0x10000, 0x180000   # 1.5MB
ota_1,    app,  ota_1,   0x190000,0x180000   # 1.5MB
```

## Runtime Memory Usage

### Stack Sizes

Configured in `sdkconfig.defaults`:

```
CONFIG_ESP_MAIN_TASK_STACK_SIZE=16384   # 16KB main task
CONFIG_PTHREAD_STACK_MIN=16384           # 16KB min for threads
```

Required for:
- JSON serialization (serde_json)
- WebSocket message handling
- Delta processing

### Per-Thread Stack

Threads spawned with explicit stack size:

```rust
std::thread::Builder::new()
    .name("delta-proc".into())
    .stack_size(16 * 1024)  // Must match CONFIG_PTHREAD_STACK_MIN
    .spawn(...)
```

## Verification Commands

```bash
# Check binary size
make esp-size          # Dev
make esp-size-release  # Release

# Detailed analysis
xtensa-esp32-elf-size target/xtensa-esp32-espidf/release/signalk-server-esp32
xtensa-esp32-elf-nm --size-sort target/.../signalk-server-esp32 | tail -50
```

## Remaining TODO

### sendCachedValues Streaming

Currently disabled on ESP32 due to heap constraints. Serializing the full model requires ~200KB allocation.

**Potential solutions:**
1. Stream JSON directly to WebSocket using fragmented frames
2. Use `serde_json::to_writer()` with a streaming writer
3. Limit model size on ESP32
4. Only send individual path values, not full model

**Priority:** Medium - clients receive deltas immediately after connecting, so cached values are nice-to-have.

### REST API Streaming

Same issue as sendCachedValues for `/signalk/v1/api`.

**Workaround:** Use path queries instead (`/signalk/v1/api/vessels/self/navigation/position`)

**Priority:** Low - path queries work fine.
