//! SignalK Server for ESP32
//!
//! This binary requires the ESP32 Rust toolchain.
//! It will not compile with the standard Rust toolchain.
//!
//! NOTE: This is a REFERENCE IMPLEMENTATION showing how to structure
//! ESP32 deployment. The core logic (MemoryStore, Delta processing)
//! is identical to Linux - only the server infrastructure differs.

// Uncomment when ESP32 toolchain is available:
// use esp_idf_svc::eventloop::EspSystemEventLoop;
// use esp_idf_svc::http::server::{Configuration, EspHttpServer};
// use esp_idf_svc::nvs::EspDefaultNvsPartition;
// use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration as WifiConfig, EspWifi};
// use esp_idf_sys as _;

use signalk_core::{Delta, MemoryStore, PathValue, SignalKStore, Update};
use signalk_protocol::{HelloMessage, ServerMessage};
use std::sync::{Arc, Mutex};

fn main() -> anyhow::Result<()> {
    // ESP32-specific: Initialize ESP-IDF
    // esp_idf_sys::link_patches();
    
    println!("SignalK ESP32 Server - Reference Implementation");
    println!("This demonstrates how ESP32 deployment differs from Linux");
    println!();
    println!("Key differences:");
    println!("  - Uses esp-idf-svc instead of Axum");
    println!("  - Uses Mutex instead of Tokio RwLock");
    println!("  - Uses std::thread instead of tokio::spawn");
    println!("  - No admin UI (limited flash storage)");
    println!();
    println!("Shared with Linux:");
    println!("  ✓ signalk-core::MemoryStore (identical)");
    println!("  ✓ signalk-protocol::Delta (identical)");
    println!("  ✓ Business logic (delta processing)");
    println!();
    println!("To build for ESP32, you need:");
    println!("  1. ESP32 Rust toolchain: https://esp-rs.github.io/book/");
    println!("  2. Uncomment ESP-IDF dependencies in Cargo.toml");
    println!("  3. Uncomment ESP-IDF code in this file");
    println!();
    
    // REFERENCE: How ESP32 implementation would work
    reference_esp32_architecture();
    
    Ok(())
}

/// Reference architecture showing ESP32 server structure
fn reference_esp32_architecture() {
    println!("=== ESP32 Architecture Reference ===");
    println!();
    println!("1. Initialization:");
    println!("   let store = Arc::new(Mutex::new(MemoryStore::new(urn)));");
    println!("   // Same MemoryStore as Linux!");
    println!();
    
    println!("2. Delta Processing Thread:");
    println!("   std::thread::spawn(move || {{");
    println!("       while let Ok(delta) = delta_rx.recv() {{");
    println!("           store.lock().unwrap().apply_delta(&delta);");
    println!("           // Broadcast to WebSocket clients");
    println!("       }}");
    println!("   }});");
    println!();
    
    println!("3. HTTP Server (esp-idf-svc):");
    println!("   let server = EspHttpServer::new(&Default::default())?;");
    println!("   server.fn_handler(\"/signalk\", Method::Get, discovery_handler)?;");
    println!();
    
    println!("4. WebSocket Connections:");
    println!("   // Send HelloMessage (same protocol as Linux)");
    println!("   let hello = HelloMessage::new(name, version, urn);");
    println!("   let msg = ServerMessage::Hello(hello);");
    println!("   ws_send(serde_json::to_string(&msg)?);");
    println!();
    
    println!("See docs/ESP32_MODULARITY.md for complete implementation guide");
}

// Example: Shared business logic works identically on ESP32
fn example_shared_logic() {
    // This exact code works on both Linux and ESP32
    let urn = "urn:mrn:signalk:uuid:esp32-device-001";
    let mut store = MemoryStore::new(urn);
    
    // Create and apply delta (same on both platforms)
    let delta = Delta {
        context: Some("vessels.self".to_string()),
        updates: vec![Update {
            source_ref: Some("esp32.gps".to_string()),
            source: None,
            timestamp: Some("2026-01-17T10:30:00.000Z".to_string()),
            values: vec![PathValue {
                path: "navigation.position".to_string(),
                value: serde_json::json!({
                    "latitude": 60.123456,
                    "longitude": 24.987654
                }),
            }],
            meta: None,
        }],
    };
    
    store.apply_delta(&delta);
    
    // Query data (same on both platforms)
    if let Some(value) = store.get_path("vessels/urn:mrn:signalk:uuid:esp32-device-001/navigation/position") {
        println!("Position retrieved: {}", value);
    }
}

