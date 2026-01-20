//! Build script for SignalK ESP32 Server
//!
//! This script sets up the ESP-IDF environment variables needed for compilation.

fn main() {
    // Output ESP-IDF environment configuration
    // This is required for the esp-idf-svc crate to find the IDF toolchain
    embuild::espidf::sysenv::output();
}
