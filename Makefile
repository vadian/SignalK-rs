# SignalK-RS Makefile
# Simplifies common development tasks

.PHONY: help test test-unit test-integration test-core test-server test-all \
        build build-release run run-release clean check fmt clippy doc \
        watch watch-test install pre-commit bench \
        build-esp build-esp-release run-esp run-esp-release clean-esp esp-size esp-size-release \
        test-ws test-ws-esp test-ws-hello test-ws-subscribe test-ws-throttle test-ws-paths test-ws-delta

# Default target
.DEFAULT_GOAL := help

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m # No Color

##@ Help

help: ## Display this help message
	@echo "$(BLUE)SignalK-RS Development Commands$(NC)"
	@echo ""
	@awk 'BEGIN {FS = ":.*##"; printf "Usage:\n  make $(GREEN)<target>$(NC)\n"} \
		/^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-18s$(NC) %s\n", $$1, $$2 } \
		/^##@/ { printf "\n$(BLUE)%s$(NC)\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

##@ Testing

test: ## Run all tests
	@echo "$(GREEN)Running all tests...$(NC)"
	@cargo test --workspace

test-unit: ## Run unit tests only
	@echo "$(GREEN)Running unit tests...$(NC)"
	@cargo test --workspace --lib

test-integration: ## Run integration tests only
	@echo "$(GREEN)Running integration tests...$(NC)"
	@cargo test --workspace --test '*'

test-core: ## Run tests for signalk-core crate
	@echo "$(GREEN)Running signalk-core tests...$(NC)"
	@cargo test -p signalk-core

test-protocol: ## Run tests for signalk-protocol crate
	@echo "$(GREEN)Running signalk-protocol tests...$(NC)"
	@cargo test -p signalk-protocol

test-server: ## Run tests for signalk-server crate
	@echo "$(GREEN)Running signalk-server tests...$(NC)"
	@cargo test -p signalk-server

test-web: ## Run tests for signalk-web crate
	@echo "$(GREEN)Running signalk-web tests...$(NC)"
	@cargo test -p signalk-web

test-all: ## Run all tests with verbose output
	@echo "$(GREEN)Running all tests (verbose)...$(NC)"
	@cargo test --workspace --verbose

test-quiet: ## Run all tests with minimal output
	@echo "$(GREEN)Running all tests (quiet)...$(NC)"
	@cargo test --workspace --quiet

##@ Building

build: ## Build all crates in debug mode
	@echo "$(GREEN)Building project (debug)...$(NC)"
	@cargo build --workspace

build-release: ## Build all crates in release mode
	@echo "$(GREEN)Building project (release)...$(NC)"
	@cargo build --workspace --release

build-server: ## Build only the Linux server binary
	@echo "$(GREEN)Building signalk-server-linux...$(NC)"
	@cargo build -p signalk-server-linux

build-server-release: ## Build server binary in release mode
	@echo "$(GREEN)Building signalk-server-linux (release)...$(NC)"
	@cargo build -p signalk-server-linux --release

##@ Running

run: ## Run the server in debug mode
	@echo "$(GREEN)Starting SignalK server (debug)...$(NC)"
	@cargo run -p signalk-server-linux

run-release: ## Run the server in release mode
	@echo "$(GREEN)Starting SignalK server (release)...$(NC)"
	@cargo run -p signalk-server-linux --release

##@ Development

check: ## Check code without building (fast)
	@echo "$(GREEN)Checking code...$(NC)"
	@cargo check --workspace

check-all: ## Check code with all features and targets
	@echo "$(GREEN)Checking all configurations...$(NC)"
	@cargo check --workspace --all-targets --all-features

fmt: ## Format code with rustfmt
	@echo "$(GREEN)Formatting code...$(NC)"
	@cargo fmt --all

fmt-check: ## Check code formatting without modifying
	@echo "$(GREEN)Checking code formatting...$(NC)"
	@cargo fmt --all -- --check

clippy: ## Run clippy linter
	@echo "$(GREEN)Running clippy...$(NC)"
	@cargo clippy --workspace -- -D warnings

clippy-fix: ## Run clippy with automatic fixes
	@echo "$(GREEN)Running clippy with fixes...$(NC)"
	@cargo clippy --workspace --fix --allow-dirty --allow-staged

##@ Documentation

doc: ## Generate and open documentation
	@echo "$(GREEN)Generating documentation...$(NC)"
	@cargo doc --workspace --no-deps --open

doc-all: ## Generate documentation with dependencies
	@echo "$(GREEN)Generating full documentation...$(NC)"
	@cargo doc --workspace --open

##@ Cleaning

clean: ## Remove build artifacts
	@echo "$(RED)Cleaning build artifacts...$(NC)"
	@cargo clean

clean-target: ## Remove only target directory
	@echo "$(RED)Removing target directory...$(NC)"
	@rm -rf target/

##@ Watching (requires cargo-watch)

watch: ## Watch for changes and rebuild
	@echo "$(GREEN)Watching for changes...$(NC)"
	@cargo watch -x check

watch-test: ## Watch for changes and run tests
	@echo "$(GREEN)Watching and testing...$(NC)"
	@cargo watch -x test

watch-run: ## Watch for changes and restart server
	@echo "$(GREEN)Watching and running server...$(NC)"
	@cargo watch -x 'run -p signalk-server-linux'

##@ CI/CD

ci: fmt-check clippy test ## Run all CI checks (format, lint, test)
	@echo "$(GREEN)All CI checks passed!$(NC)"

pre-commit: fmt clippy test-quiet ## Run pre-commit checks
	@echo "$(GREEN)Pre-commit checks completed!$(NC)"

##@ Benchmarking

bench: ## Run benchmarks
	@echo "$(GREEN)Running benchmarks...$(NC)"
	@cargo bench --workspace

##@ Installation

install: ## Install cargo tools needed for development
	@echo "$(GREEN)Installing development tools...$(NC)"
	@cargo install cargo-watch cargo-edit cargo-outdated cargo-audit
	@echo "$(GREEN)Tools installed:$(NC)"
	@echo "  - cargo-watch: Watch for changes"
	@echo "  - cargo-edit: Add/remove dependencies easily"
	@echo "  - cargo-outdated: Check for outdated dependencies"
	@echo "  - cargo-audit: Security audit"

deps-check: ## Check for outdated dependencies
	@echo "$(GREEN)Checking dependencies...$(NC)"
	@cargo outdated

deps-audit: ## Run security audit on dependencies
	@echo "$(GREEN)Running security audit...$(NC)"
	@cargo audit

##@ Statistics

lines: ## Count lines of code
	@echo "$(BLUE)Lines of code:$(NC)"
	@find crates bins -name '*.rs' -not -path '*/target/*' | xargs wc -l | tail -1

stats: ## Show project statistics
	@echo "$(BLUE)Project Statistics:$(NC)"
	@echo ""
	@echo "$(GREEN)Source files:$(NC)"
	@find crates bins -name '*.rs' -not -path '*/target/*' | wc -l
	@echo ""
	@echo "$(GREEN)Lines of Rust code:$(NC)"
	@find crates bins -name '*.rs' -not -path '*/target/*' | xargs wc -l | tail -1
	@echo ""
	@echo "$(GREEN)Test count:$(NC)"
	@cargo test --workspace -- --list 2>/dev/null | grep -c ": test" || echo "0"
	@echo ""
	@echo "$(GREEN)Crates:$(NC)"
	@ls -1 crates/ | wc -l
	@echo ""
	@echo "$(GREEN)Binaries:$(NC)"
	@ls -1 bins/ | wc -l

##@ Release

release-check: ## Check if ready for release
	@echo "$(GREEN)Checking release readiness...$(NC)"
	@cargo build --release --workspace
	@cargo test --release --workspace
	@echo "$(GREEN)Release checks passed!$(NC)"

version: ## Show current version
	@echo "$(BLUE)Current version:$(NC)"
	@cargo metadata --no-deps --format-version 1 | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4

##@ ESP32 (requires esp-idf toolchain)

# ESP32 sdkconfig files - used for dependency tracking
ESP_SDKCONFIG_BASE := sdkconfig.defaults
ESP_SDKCONFIG_DEV := sdkconfig.defaults.dev
ESP_SDKCONFIG_RELEASE := sdkconfig.defaults.release
ESP_TARGET_DIR := bins/signalk-server-esp32/target

# Helper to check if sdkconfig changed and clean is needed
define check_esp_sdkconfig
	@if [ -d "$(ESP_TARGET_DIR)" ]; then \
		for cfg in $(1); do \
			if [ "$$cfg" -nt "$(ESP_TARGET_DIR)/.sdkconfig_stamp" ] 2>/dev/null; then \
				echo "$(YELLOW)sdkconfig changed, cleaning ESP32 build...$(NC)"; \
				rm -rf $(ESP_TARGET_DIR); \
				break; \
			fi; \
		done; \
	fi
	@mkdir -p $(ESP_TARGET_DIR) && touch $(ESP_TARGET_DIR)/.sdkconfig_stamp
endef

build-esp: ## Build ESP32 (dev, 3MB partition)
	$(call check_esp_sdkconfig,$(ESP_SDKCONFIG_BASE) $(ESP_SDKCONFIG_DEV))
	cd bins/signalk-server-esp32 && \
	ESP_IDF_SDKCONFIG_DEFAULTS="../../sdkconfig.defaults;../../sdkconfig.defaults.dev" \
	cargo build

build-esp-release: ## Build ESP32 (release, OTA partitions)
	$(call check_esp_sdkconfig,$(ESP_SDKCONFIG_BASE) $(ESP_SDKCONFIG_RELEASE))
	cd bins/signalk-server-esp32 && \
	ESP_IDF_SDKCONFIG_DEFAULTS="../../sdkconfig.defaults;../../sdkconfig.defaults.release" \
	cargo build --release

run-esp: ## Build and flash ESP32 (dev, 3MB partition)
	$(call check_esp_sdkconfig,$(ESP_SDKCONFIG_BASE) $(ESP_SDKCONFIG_DEV))
	cd bins/signalk-server-esp32 && \
	ESP_IDF_SDKCONFIG_DEFAULTS="../../sdkconfig.defaults;../../sdkconfig.defaults.dev" \
	cargo run

run-esp-release: ## Build and flash ESP32 (release, OTA partitions)
	$(call check_esp_sdkconfig,$(ESP_SDKCONFIG_BASE) $(ESP_SDKCONFIG_RELEASE))
	cd bins/signalk-server-esp32 && \
	ESP_IDF_SDKCONFIG_DEFAULTS="../../sdkconfig.defaults;../../sdkconfig.defaults.release" \
	cargo run --release

clean-esp: ## Clean ESP32 build artifacts
	@echo "$(RED)Cleaning ESP32 build...$(NC)"
	@rm -rf $(ESP_TARGET_DIR)

esp-size: ## Show ESP32 binary size (dev)
	@echo "$(BLUE)ESP32 binary size (dev):$(NC)"
	@xtensa-esp32-elf-size bins/signalk-server-esp32/target/xtensa-esp32-espidf/debug/signalk-server-esp32

esp-size-release: ## Show ESP32 binary size (release)
	@echo "$(BLUE)ESP32 binary size (release):$(NC)"
	@xtensa-esp32-elf-size bins/signalk-server-esp32/target/xtensa-esp32-espidf/release/signalk-server-esp32

##@ WebSocket Integration Tests (requires running server)

test-ws: ## Run all WebSocket integration tests (requires server on localhost:4000)
	@echo "$(GREEN)Running WebSocket tests against localhost:4000...$(NC)"
	@bash tests/integration/run_all.sh

test-ws-esp: ## Run WebSocket tests against ESP32 (set SIGNALK_HOST)
	@if [ -z "$(SIGNALK_HOST)" ]; then \
		echo "$(RED)ERROR: Set SIGNALK_HOST to your ESP32 IP address$(NC)"; \
		echo "  Example: make test-ws-esp SIGNALK_HOST=192.168.1.100"; \
		exit 1; \
	fi
	@echo "$(GREEN)Running integration tests against $(SIGNALK_HOST):$(SIGNALK_PORT:-80)...$(NC)"
	@SIGNALK_HOST=$(SIGNALK_HOST) SIGNALK_PORT=$(or $(SIGNALK_PORT),80) bash tests/integration/run_all.sh

test-ws-hello: ## Quick test: WebSocket hello message
	@echo "$(GREEN)Testing WebSocket hello message...$(NC)"
	@bash tests/integration/01_hello.sh

test-ws-subscribe: ## Quick test: Subscription messages
	@echo "$(GREEN)Testing subscriptions...$(NC)"
	@bash tests/integration/02_subscriptions.sh

test-ws-throttle: ## Quick test: Throttling/period
	@echo "$(GREEN)Testing throttling...$(NC)"
	@bash tests/integration/03_throttling.sh

test-ws-paths: ## Quick test: Path patterns
	@echo "$(GREEN)Testing path patterns...$(NC)"
	@bash tests/integration/04_path_subscriptions.sh

test-ws-delta: ## Quick test: Sending deltas
	@echo "$(GREEN)Testing delta sending...$(NC)"
	@bash tests/integration/05_send_delta.sh
