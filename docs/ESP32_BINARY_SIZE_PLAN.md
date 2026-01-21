# ESP32 Binary Size Reduction Plan

## Current State

**Debug build size breakdown:**
```
text:    2,056,245 bytes (~2.0 MB) - code
data:      355,808 bytes (~348 KB) - initialized data
bss:        17,577 bytes (~17 KB)  - uninitialized data
Total:   2,429,630 bytes (~2.3 MB)
```

**Current issues:**
1. Building in **debug mode** (no optimizations)
2. No Cargo release profile configured
3. No ESP-IDF compiler optimizations in sdkconfig.defaults
4. Heavy Rust dependencies being pulled in
5. Many unused ESP-IDF components linked

---

## Build Configuration Strategy

We need two distinct build configurations:

| Aspect | Development | Production |
|--------|-------------|------------|
| Cargo profile | `debug` or `release` | `release` |
| Partition table | Single 3MB factory | OTA (2x 1.5MB) |
| Debug symbols | Full (`debug = 2`) | Line tables only (`debug = 1`) |
| ESP-IDF optimization | `-O2` (faster builds) | `-Os` (size) |
| Logging level | INFO | WARN |
| Stack checking | Enabled | Enabled |
| Max binary size | < 3 MB | < 1.5 MB |

### File Structure

```
signalk-rs/
├── sdkconfig.defaults              # Shared base config
├── sdkconfig.defaults.dev          # Development overrides (3MB partition, no OTA)
├── sdkconfig.defaults.release      # Production overrides (-Os, OTA partitions)
├── partitions.dev.csv              # 3MB single factory partition
├── partitions.release.csv          # OTA partition layout
└── bins/signalk-server-esp32/
    └── Cargo.toml                  # Release profile settings
```

### How sdkconfig Works

esp-idf-sys discovers `sdkconfig.defaults` from the **workspace root** (not the binary crate).

**Note:** The ESP32 crates are excluded from the main workspace (in `Cargo.toml`) so that
`cargo build` at the workspace level doesn't try to build them for Linux. However, esp-idf-sys
still looks for sdkconfig files at the workspace root.

**Selection via environment variable:**
```bash
# Development (default) - from bins/signalk-server-esp32/
cargo build

# Production - from bins/signalk-server-esp32/
ESP_IDF_SDKCONFIG_DEFAULTS=../../sdkconfig.defaults.release cargo build --release
```

**Layering:** Base `sdkconfig.defaults` is loaded first, then the file specified by
`ESP_IDF_SDKCONFIG_DEFAULTS` overrides it.

---

## Phase 0: Setup Dev/Release Configuration

### 0.1 Create Partition Tables

**`partitions.dev.csv`** (3MB factory, no OTA):
```csv
# Development partition table - large single app partition
# Name,   Type, SubType, Offset,  Size,    Flags
nvs,      data, nvs,     0x9000,  0x6000,
phy_init, data, phy,     0xf000,  0x1000,
factory,  app,  factory, 0x10000, 0x300000,
```

**`partitions.release.csv`** (OTA enabled):
```csv
# Production partition table - OTA support
# Name,   Type, SubType, Offset,  Size,    Flags
nvs,      data, nvs,     0x9000,  0x6000,
phy_init, data, phy,     0xf000,  0x1000,
ota_0,    app,  ota_0,   0x10000, 0x180000,
ota_1,    app,  ota_1,   0x190000,0x180000,
```

### 0.2 Create sdkconfig Files

**`sdkconfig.defaults`** (shared base - keep current file):
```
# Stack sizes, WebSocket, WiFi, etc. - shared between dev and release
CONFIG_ESP_MAIN_TASK_STACK_SIZE=16384
CONFIG_PTHREAD_STACK_MIN=16384
CONFIG_FREERTOS_CHECK_STACKOVERFLOW=2
CONFIG_FREERTOS_WATCHPOINT_END_OF_STACK=y
CONFIG_HTTPD_WS_SUPPORT=y
CONFIG_WS_TRANSPORT=y
CONFIG_LWIP_MAX_SOCKETS=16
# ... rest of current settings
```

**`sdkconfig.defaults.dev`** (development overrides):
```
# Development configuration
# - Large partition for unoptimized binaries
# - Full debugging support
# - Faster compile times (no -Os)

# Use 3MB single partition (no OTA)
CONFIG_PARTITION_TABLE_CUSTOM=y
CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="partitions.dev.csv"

# Keep default optimization (-O2) for faster builds
# (don't set CONFIG_COMPILER_OPTIMIZATION_SIZE)

# Full logging for debugging
CONFIG_LOG_DEFAULT_LEVEL_INFO=y

# Disable unused components to speed up builds
CONFIG_ETH_ENABLED=n
CONFIG_MQTT_ENABLE=n
CONFIG_FATFS_ENABLED=n
```

**`sdkconfig.defaults.release`** (production overrides):
```
# Production configuration
# - OTA-compatible partition layout
# - Size optimizations
# - Reduced logging

# Use OTA partition layout
CONFIG_PARTITION_TABLE_CUSTOM=y
CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="partitions.release.csv"

# Optimize for size
CONFIG_COMPILER_OPTIMIZATION_SIZE=y
CONFIG_COMPILER_CXX_EXCEPTIONS=n
CONFIG_COMPILER_CXX_RTTI=n

# Reduce logging (WARN level)
CONFIG_LOG_DEFAULT_LEVEL_WARN=y

# Disable unused components
CONFIG_ETH_ENABLED=n
CONFIG_MQTT_ENABLE=n
CONFIG_FATFS_ENABLED=n
CONFIG_ESP_DRIVER_MCPWM_ENABLED=n
CONFIG_LCD_ENABLE=n
CONFIG_SDMMC_ENABLED=n
CONFIG_I2S_ENABLE=n
```

### 0.3 Add Cargo Release Profile

Add to `bins/signalk-server-esp32/Cargo.toml`:

```toml
[profile.release]
opt-level = "s"          # Optimize for size
lto = true               # Link-time optimization
codegen-units = 1        # Better optimization
panic = "abort"          # Smaller than unwinding
debug = 1                # Line tables only (keeps backtraces readable)
```

### 0.4 Makefile Targets ✅

The following targets are available in the workspace `Makefile`:

```bash
# Build and flash in one step (recommended)
make run-esp           # Dev build (3MB partition, full debugging)
make run-esp-release   # Release build (OTA partitions, size-optimized)

# Build only
make build-esp         # Dev build
make build-esp-release # Release build

# Check binary size
make esp-size          # Dev binary size
make esp-size-release  # Release binary size
```

**Implementation:**
```makefile
run-esp: ## Build and flash ESP32 (dev, 3MB partition)
	cd bins/signalk-server-esp32 && \
	ESP_IDF_SDKCONFIG_DEFAULTS="../../sdkconfig.defaults;../../sdkconfig.defaults.dev" \
	cargo run

run-esp-release: ## Build and flash ESP32 (release, OTA partitions)
	cd bins/signalk-server-esp32 && \
	ESP_IDF_SDKCONFIG_DEFAULTS="../../sdkconfig.defaults;../../sdkconfig.defaults.release" \
	cargo run --release
```

---

## Phase 1: Low-Hanging Fruit (Expected: -40-50%)

### 1.1 Add Release Profile Configuration

Create release profile in `bins/signalk-server-esp32/Cargo.toml`:

```toml
[profile.release]
opt-level = "s"          # Optimize for size
lto = true               # Link-time optimization
codegen-units = 1        # Better optimization, slower compile
panic = "abort"          # Smaller than unwinding
# strip = true           # Don't strip - keeps symbols for stack traces
debug = 1                # Line tables only (minimal debug info for backtraces)
```

**Rationale:** Debug builds include full debug info and no optimizations. Release with LTO can reduce binary size by 40-60%.

**Note on debuggability:** We keep `debug = 1` (line tables) and avoid `strip = true` so that:
- Stack traces show function names and line numbers
- `espflash monitor` output is readable
- Panic messages are meaningful

If size is still critical, `strip = true` saves ~50-100KB but loses symbol names in crashes.

### 1.2 Add ESP-IDF Compiler Optimizations

Add to `sdkconfig.defaults`:

```
# Compiler optimization for size (use -Os instead of -O2)
CONFIG_COMPILER_OPTIMIZATION_SIZE=y

# Disable C++ features we don't use
CONFIG_COMPILER_CXX_EXCEPTIONS=n
CONFIG_COMPILER_CXX_RTTI=n

# KEEP stack checking enabled for debugging!
# CONFIG_COMPILER_STACK_CHECK_MODE_NONE=y  # Don't disable - we need this

# Keep logging at INFO level for debugging - only reduce in production
# CONFIG_LOG_DEFAULT_LEVEL_WARN=y  # Optional: uncomment for production
```

**Expected savings:** 10-20% reduction in ESP-IDF component sizes

### 1.3 Strip Unused ESP-IDF Components

Add to `sdkconfig.defaults` to disable unused components.

**IMPORTANT:** Test each change individually - some components have hidden dependencies.

```
# =============================================================================
# SAFE TO DISABLE - These are clearly unused
# =============================================================================

# Disable unused peripheral drivers
CONFIG_ETH_ENABLED=n                    # No Ethernet (we use WiFi)
CONFIG_ESP_DRIVER_MCPWM_ENABLED=n       # No motor PWM
CONFIG_LCD_ENABLE=n                     # No LCD support
CONFIG_SDMMC_ENABLED=n                  # No SD card
CONFIG_I2S_ENABLE=n                     # No I2S audio

# Disable unused protocols
CONFIG_MQTT_ENABLE=n                    # No MQTT (we use WebSocket)

# Disable unused filesystem support (unless storing config files)
CONFIG_FATFS_ENABLED=n                  # No FAT filesystem

# =============================================================================
# TEST CAREFULLY - May have dependencies
# =============================================================================

# WiFi provisioning - only if not using SmartConfig/BLE provisioning
# CONFIG_WIFI_PROV_ENABLE=n             # Test this - may affect WiFi setup

# SPIFFS - only disable if not storing any files
# CONFIG_SPIFFS_ENABLED=n               # Keep if storing HTML/config

# Console - disable if not using USB/UART console commands
# CONFIG_CONSOLE_ENABLE=n               # May affect log output - test first

# =============================================================================
# DO NOT DISABLE - Required for core functionality
# =============================================================================
# CONFIG_WIFI_ENABLED - Required!
# CONFIG_HTTPD_WS_SUPPORT - Required for WebSocket!
# CONFIG_LWIP_* - Required for networking!
# CONFIG_NVS_* - Required for config storage!
```

**Note:** The library sizes shown (e.g., "-891 KB") are the full static library sizes. The linker only includes functions actually called, so actual savings may be smaller. But disabling components prevents accidental inclusion and reduces build time.

---

## Phase 2: Dependency Optimization (Expected: -15-25%)

### 2.1 Replace `regex` with Lightweight Alternative

**Current:** `regex` crate (~200KB compiled) used only for simple wildcard matching in `PathPattern`

**Solution:** Replace with manual wildcard matching or use `regex-lite` crate

The current usage in `signalk-core/src/path.rs` converts patterns like:
- `navigation.*` → match any suffix
- `propulsion.*.revolutions` → match one segment in middle
- `*` → match everything

This can be implemented without regex in ~50 lines of code.

**Option A:** Feature flag `regex` behind `#[cfg(feature = "full-regex")]`
**Option B:** Create `signalk-core-embedded` variant without regex
**Option C (Recommended):** Replace with simple glob matching function

```rust
// Simple glob match without regex
fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('.').collect();
    let path_parts: Vec<&str> = path.split('.').collect();

    glob_match_parts(&pattern_parts, &path_parts)
}
```

**Expected savings:** ~150-200 KB

### 2.2 Replace `chrono` with Lightweight Alternative

**Current:** `chrono` crate (~100KB) used only for `Utc::now().to_rfc3339()`

**Solution:** Use ESP-IDF's built-in time functions + manual RFC3339 formatting

```rust
// In signalk-esp32 crate
pub fn current_timestamp() -> String {
    // Already implemented! Uses esp_idf_svc time
}
```

The ESP32 crate already has `current_timestamp()` in `http.rs`. Remove `chrono` from `signalk-protocol` by:
1. Making timestamp a `String` parameter passed in
2. Or using conditional compilation `#[cfg(not(target_os = "espidf"))]`

**Expected savings:** ~80-100 KB

### 2.3 Optimize `serde_json`

**Current:** Full serde_json with all features

**Options:**
- Use `serde_json` with `default-features = false` + only needed features
- Consider `miniserde` for simpler cases (but likely not compatible)

```toml
serde_json = { version = "1.0", default-features = false, features = ["alloc"] }
```

**Expected savings:** ~20-50 KB

### 2.4 Review `uuid` Usage

**Current:** `uuid` with `v4` feature (requires random number generation)

**Solution:** ESP32 already generates UUID from hardware in `ServerConfig::new_with_uuid()`.
Could use simpler approach or hardware RNG directly.

```toml
# Remove if not strictly needed, or use:
uuid = { version = "1.0", default-features = false, features = ["v4"] }
```

---

## Phase 3: Architecture Changes (If Needed)

### 3.1 Feature Flags for ESP32

Add feature flags to shared crates:

```toml
# In signalk-core/Cargo.toml
[features]
default = ["std", "regex"]
std = []
regex = ["dep:regex"]
embedded = []  # Lightweight mode

[dependencies]
regex = { version = "1.0", optional = true }
```

### 3.2 Separate `signalk-core-lite`

If feature flags get complex, consider a minimal embedded variant:
- No regex (simple glob matching)
- No chrono (string timestamps)
- Minimal allocations

### 3.3 Consider `esp-idf-svc` Component Selection

The `esp-idf-svc` crate can be configured to only include needed components:

```toml
esp-idf-svc = {
    version = "0.51",
    default-features = false,
    features = [
        "std",
        "binstart",
        "experimental",
        # Only include what we need:
        "wifi",
        "httpd",
        "nvs",
    ]
}
```

---

## Implementation Order

1. **Quick wins (Phase 1):** Add release profile + sdkconfig optimizations
   - Effort: 30 minutes
   - Expected result: ~1.0-1.2 MB release binary

2. **Test release build** - measure actual savings before proceeding

3. **If still too large, Phase 2:** Replace regex/chrono
   - Effort: 2-4 hours
   - Expected result: ~800 KB - 1.0 MB

4. **Phase 3 only if necessary**

---

## Verification Commands

```bash
# Build release
cd bins/signalk-server-esp32
cargo build --release

# Check size
xtensa-esp32-elf-size target/xtensa-esp32-espidf/release/signalk-server-esp32

# Detailed section analysis
xtensa-esp32-elf-objdump -h target/xtensa-esp32-espidf/release/signalk-server-esp32

# Find largest symbols
xtensa-esp32-elf-nm --size-sort target/xtensa-esp32-espidf/release/signalk-server-esp32 | tail -50
```

---

## Target Flash Layout (4MB ESP32)

### Development (No OTA) - `partitions.dev.csv`
```
Partition Table:
├── bootloader:     32 KB  @ 0x1000
├── partition-table: 4 KB  @ 0x8000
├── nvs:            24 KB  @ 0x9000
├── phy_init:        4 KB  @ 0xF000
└── factory:        3 MB   @ 0x10000   ← Single app partition (plenty of room)
```

### Production (OTA Enabled) - `partitions.release.csv`
```
Partition Table:
├── bootloader:     32 KB  @ 0x1000
├── partition-table: 4 KB  @ 0x8000
├── nvs:            24 KB  @ 0x9000
├── phy_init:        4 KB  @ 0xF000
├── ota_0:         1.5 MB  @ 0x10000   ← App partition (target: < 1.5 MB)
├── ota_1:         1.5 MB  @ 0x190000  ← OTA update partition
└── nvs_key:         4 KB  @ 0x310000
```

**Goals:**
- Development: < 3 MB (comfortable margin, full debugging)
- Production: < 1.5 MB (enables OTA updates)

---

## Notes

- The 32MB debug ELF size is misleading - it includes debug symbols
- Actual flash usage is `text + data` sections only
- BSS doesn't count toward flash (uninitialized, allocated at runtime)
- ESP-IDF linker only includes used functions from static libraries