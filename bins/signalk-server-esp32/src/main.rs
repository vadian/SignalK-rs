//! SignalK Server for ESP32 (Xtensa)
//!
//! A minimal Signal K server implementation for ESP32 microcontrollers.
//! Uses esp-idf-svc for WiFi, HTTP, and WebSocket functionality.
//!
//! # Features
//! - WebSocket server for delta streaming
//! - HTTP server for discovery and REST API
//! - Shared signalk-core and signalk-protocol crates (same as Linux)
//!
//! # Differences from Linux Version
//! - Uses std::sync::Mutex instead of tokio::sync::RwLock
//! - Uses std::thread instead of tokio::spawn
//! - Uses esp-idf-svc HTTP server instead of Axum
//! - No admin UI (flash storage constraints)
//! - No plugin support

use esp_idf_hal::io::EspIOError;
use esp_idf_svc::sys::EspError;
#[derive(Debug)]
pub struct SignalKError {}
impl From<EspError> for SignalKError {
    fn from(err: EspError) -> Self {
        log::error!("EspError occurred: {err:?}");
        SignalKError {}
    }
}

impl From<EspIOError> for SignalKError {
    fn from(err: EspIOError) -> Self {
        log::error!("EspIOError occurred: {err:?}");
        SignalKError {}
    }
}

impl From<serde_json::Error> for SignalKError {
    fn from(err: serde_json::Error) -> Self {
        log::error!("Serde JSON Error occurred: {err:?}");
        SignalKError {}
    }
}

use anyhow::Result;
use embedded_svc::{http::Headers, ws::FrameType};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::prelude::Peripherals,
    http::server::{ws::EspHttpWsDetachedSender, Configuration as HttpConfig, EspHttpServer},
    io::Write,
};
use log::{error, info, warn};
use serde_json::json;
use signalk_core::{Delta, MemoryStore, PathValue, SignalKStore, Update};
use signalk_esp32::{
    config::ServerConfig,
    http::{
        create_discovery_json, create_hello_message, current_timestamp,
        default_subscription_for_mode, get_path_json, process_client_message, ClientSubscription,
        WsQueryParams,
    },
    wifi::connect_wifi,
};
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

// ============================================================================
// Client State Management
// ============================================================================

/// Per-client state including sender and subscription info.
struct ClientState {
    /// Detached sender for async delta broadcasting.
    sender: EspHttpWsDetachedSender,
    /// Client's subscription state.
    subscription: ClientSubscription,
}

/// Type alias for the collection of connected WebSocket clients.
/// Key is the session ID (socket fd).
type WsClients = Arc<Mutex<HashMap<i32, ClientState>>>;

/// Check if a delta should be sent, respecting throttle limits.
/// Returns a list of pattern indices that matched and should be marked as sent.
fn should_send_delta_throttled(subscription: &ClientSubscription, delta: &Delta) -> Vec<usize> {
    let mut matched_indices = Vec::new();

    // If no subscription, don't send anything
    if subscription.is_empty() {
        return matched_indices;
    }

    // Check context filter
    if !subscription.matches_context(delta.context.as_deref()) {
        return matched_indices;
    }

    // Check each path in the delta against subscription with throttle check
    for update in &delta.updates {
        for pv in &update.values {
            if let Some(idx) = subscription.should_send_path(&pv.path) {
                if !matched_indices.contains(&idx) {
                    matched_indices.push(idx);
                }
            }
        }
    }

    matched_indices
}

// WiFi credentials - set via environment variables at build time
// Example: WIFI_SSID="MyNetwork" WIFI_PASSWORD="secret" cargo build
// Falls back to "unconfigured" if not set (will fail to connect)
const WIFI_SSID: &str = match option_env!("WIFI_SSID") {
    Some(v) => v,
    None => "unconfigured",
};
const WIFI_PASSWORD: &str = match option_env!("WIFI_PASSWORD") {
    Some(v) => v,
    None => "unconfigured",
};

fn main() -> Result<()> {
    // Initialize ESP-IDF patches
    esp_idf_svc::sys::link_patches();

    // Initialize logging
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("========================================");
    info!("  SignalK Server for ESP32 (Xtensa)");
    info!("========================================");

    // Take peripherals
    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;

    // Initialize WiFi using shared crate
    info!("Initializing WiFi...");
    let (_wifi, ip_addr) =
        connect_wifi(WIFI_SSID, WIFI_PASSWORD, peripherals.modem, sysloop.clone())?;

    // Server configuration using shared crate
    let config = ServerConfig::new_with_uuid();
    info!("Server URN: {}", config.self_urn);

    // Create shared store (same as Linux, but with Mutex instead of RwLock)
    let store = Arc::new(Mutex::new(MemoryStore::new(&config.self_urn)));

    // Create shared collection of WebSocket clients for delta broadcasting
    let ws_clients: WsClients = Arc::new(Mutex::new(HashMap::new()));

    // Channel for delta events
    let (delta_tx, delta_rx) = mpsc::channel::<Delta>();

    // Clone store and clients for delta processor
    let store_processor = Arc::clone(&store);
    let clients_processor: WsClients = Arc::clone(&ws_clients);

    // Spawn delta processor thread
    // Note: Must use Builder with explicit stack_size to avoid TLS initialization issues
    // on ESP-IDF. Stack must be >= CONFIG_PTHREAD_STACK_MIN (16KB in sdkconfig.defaults).
    std::thread::Builder::new()
        .name("delta-proc".into())
        .stack_size(16 * 1024) // 16KB - must match CONFIG_PTHREAD_STACK_MIN
        .spawn(move || {
            info!("Delta processor started");
            while let Ok(delta) = delta_rx.recv() {
                // Apply delta to store
                if let Ok(mut store) = store_processor.lock() {
                    store.apply_delta(&delta);
                }

                // Broadcast delta to subscribed WebSocket clients with throttling
                if let Ok(json) = serde_json::to_string(&delta) {
                    if let Ok(mut clients) = clients_processor.lock() {
                        // Collect failed client IDs for removal
                        let mut failed_clients = Vec::new();

                        for (client_id, client_state) in clients.iter_mut() {
                            // Check subscription filter with throttling
                            let matched_indices =
                                should_send_delta_throttled(&client_state.subscription, &delta);

                            // Skip if no patterns matched (either not subscribed or throttled)
                            if matched_indices.is_empty() {
                                continue;
                            }

                            // Send the delta
                            if let Err(e) = client_state
                                .sender
                                .send(FrameType::Text(false), json.as_bytes())
                            {
                                warn!("Failed to send delta to client {}: {:?}", client_id, e);
                                failed_clients.push(*client_id);
                            } else {
                                // Mark matched patterns as sent (update throttle timers)
                                for idx in matched_indices {
                                    client_state.subscription.mark_sent(idx);
                                }
                            }
                        }

                        // Remove failed clients
                        for client_id in failed_clients {
                            clients.remove(&client_id);
                            info!("Removed disconnected client {}", client_id);
                        }
                    }
                }
            }
            warn!("Delta processor stopped");
        })
        .expect("Failed to spawn delta processor thread");

    // Start HTTP server with WebSocket support
    let _server = start_http_server(&config, Arc::clone(&store), Arc::clone(&ws_clients))?;

    // Start demo data generator
    let delta_tx_demo = delta_tx.clone();
    std::thread::Builder::new()
        .name("demo-gen".into())
        .stack_size(16 * 1024) // 16KB - must match CONFIG_PTHREAD_STACK_MIN
        .spawn(move || {
            generate_demo_data(delta_tx_demo);
        })
        .expect("Failed to spawn demo generator thread");

    info!("========================================");
    info!("          Server Ready!");
    info!("========================================");
    info!("Discovery: http://{}/signalk", ip_addr);
    info!("REST API:  http://{}/signalk/v1/api", ip_addr);
    info!("WebSocket: ws://{}/signalk/v1/stream", ip_addr);
    info!("========================================");

    // Keep server alive
    // The server handle must be kept alive for the server to run
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

/// Start HTTP server with REST and WebSocket endpoints
fn start_http_server(
    config: &ServerConfig,
    store: Arc<Mutex<MemoryStore>>,
    ws_clients: WsClients,
) -> Result<EspHttpServer<'static>> {
    let http_config = HttpConfig {
        http_port: config.http_port,
        // Increase stack size for HTTP handlers - JSON serialization needs room
        stack_size: 16384, // 16KB (default is ~4KB which is too small for serde_json)
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&http_config)?;

    // Clone config values for handlers
    let config_name = config.name.clone();
    let config_version = config.version.clone();
    let config_self_urn = config.self_urn.clone();
    let config_port = config.http_port;

    // Discovery endpoint: GET /signalk
    server.fn_handler("/signalk", esp_idf_svc::http::Method::Get, move |req| {
        // Get local IP from the request
        let host = req.host().unwrap_or("localhost");

        let json = create_discovery_json(host, config_port)?;

        let mut response = req.into_ok_response()?;
        response.write_all(json.as_bytes())?;
        Ok::<(), SignalKError>(())
    })?;

    // REST API: GET /signalk/v1/api (full model)
    let api_store = Arc::clone(&store);
    server.fn_handler(
        "/signalk/v1/api",
        esp_idf_svc::http::Method::Get,
        move |req| {
            let json = if let Ok(store) = api_store.lock() {
                serde_json::to_string(store.full_model())?
            } else {
                r#"{"error": "Store locked"}"#.to_string()
            };

            let mut response = req.into_ok_response()?;
            response.write_all(json.as_bytes())?;
            Ok::<(), SignalKError>(())
        },
    )?;

    // REST API: GET /signalk/v1/api/* (path query)
    // Note: esp-idf-svc requires explicit wildcard routes
    let api_path_store = Arc::clone(&store);
    server.fn_handler(
        "/signalk/v1/api/*",
        esp_idf_svc::http::Method::Get,
        move |req| {
            // Extract path after /signalk/v1/api/
            let uri = req.uri();
            let path = uri
                .strip_prefix("/signalk/v1/api/")
                .unwrap_or("")
                .split('?')
                .next()
                .unwrap_or("");

            if path.is_empty() {
                // Should have been handled by the exact route above
                let json = if let Ok(store) = api_path_store.lock() {
                    serde_json::to_string(store.full_model())?
                } else {
                    r#"{"error": "Store locked"}"#.to_string()
                };
                let mut response = req.into_ok_response()?;
                response.write_all(json.as_bytes())?;
                return Ok::<(), SignalKError>(());
            }

            // Convert URL path (with /) to SignalK path (with .)
            let sk_path = path.replace('/', ".");

            match get_path_json(&api_path_store, &sk_path) {
                Ok(json) => {
                    let mut response = req.into_ok_response()?;
                    response.write_all(json.as_bytes())?;
                }
                Err(_) => {
                    // Return 404 for unknown paths
                    let error_json = format!(r#"{{"error": "Path not found: {}"}}"#, sk_path);
                    let mut response = req.into_response(404, Some("Not Found"), &[])?;
                    response.write_all(error_json.as_bytes())?;
                }
            }

            Ok::<(), SignalKError>(())
        },
    )?;

    // WebSocket endpoint: GET /signalk/v1/stream
    let ws_name = config_name.clone();
    let ws_version = config_version.clone();
    let ws_self_urn = config_self_urn.clone();
    let ws_store = Arc::clone(&store);
    let ws_clients_handler: WsClients = Arc::clone(&ws_clients);

    server.ws_handler("/signalk/v1/stream", move |ws| {
        let client_id = ws.session();

        // Handle new connection
        if ws.is_new() {
            // Note: esp-idf-svc doesn't expose URI on WebSocket connections,
            // so we use default query params (subscribe=self, sendCachedValues=true).
            // Clients can modify subscriptions via subscribe/unsubscribe messages.
            let query_params = WsQueryParams::default();

            info!(
                "WebSocket client {} connected (subscribe={:?}, sendCachedValues={})",
                client_id, query_params.subscribe, query_params.send_cached_values
            );

            // Send hello message using shared helper
            let hello_msg = create_hello_message(&ws_name, &ws_version, &ws_self_urn);

            if let Ok(json) = serde_json::to_string(&hello_msg) {
                if let Err(e) = ws.send(FrameType::Text(false), json.as_bytes()) {
                    error!("Failed to send hello: {:?}", e);
                    return Ok::<(), SignalKError>(());
                }
            }

            // Send current state if sendCachedValues is true (default)
            if query_params.send_cached_values {
                if let Ok(store) = ws_store.lock() {
                    let full_model = store.full_model();
                    if let Ok(json) = serde_json::to_string(&full_model) {
                        let _ = ws.send(FrameType::Text(false), json.as_bytes());
                    }
                }
            }

            // Create default subscription based on query parameter
            let subscription = default_subscription_for_mode(query_params.subscribe);

            // Create detached sender for this client and register it
            // This allows the delta processor thread to push updates to this client
            match ws.create_detached_sender() {
                Ok(sender) => {
                    if let Ok(mut clients) = ws_clients_handler.lock() {
                        clients.insert(
                            client_id,
                            ClientState {
                                sender,
                                subscription,
                            },
                        );
                        info!(
                            "Registered client {} for delta streaming ({} total)",
                            client_id,
                            clients.len()
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to create detached sender for client {}: {:?}",
                        client_id, e
                    );
                }
            }

            // Return after handling new connection - don't try to recv yet
            return Ok::<(), SignalKError>(());
        }

        // Handle closed connection
        if ws.is_closed() {
            // Remove client from broadcast list
            if let Ok(mut clients) = ws_clients_handler.lock() {
                clients.remove(&client_id);
                info!(
                    "WebSocket client {} disconnected ({} remaining)",
                    client_id,
                    clients.len()
                );
            }
            return Ok::<(), SignalKError>(());
        }

        // Handle incoming data - use a reasonably sized buffer
        // The handler is called when there's data available
        let mut buf = [0u8; 1024];
        let (frame_type, len) = match ws.recv(&mut buf) {
            Ok(result) => result,
            Err(e) => {
                // This can happen on connection close or timeout - not always an error
                warn!("WebSocket recv: {:?}", e);
                return Ok::<(), SignalKError>(());
            }
        };

        match frame_type {
            FrameType::Ping => {
                let _ = ws.send(FrameType::Pong, &[]);
            }
            FrameType::Text(_) if len > 0 => {
                if let Ok(text) = std::str::from_utf8(&buf[..len]) {
                    info!("Received from client {}: {}", client_id, text);

                    // Try to parse and process subscription messages
                    if let Ok(mut clients) = ws_clients_handler.lock() {
                        if let Some(client_state) = clients.get_mut(&client_id) {
                            if let Some(new_sub) =
                                process_client_message(text, &client_state.subscription)
                            {
                                info!(
                                    "Client {} subscription updated: context={:?}, patterns={}",
                                    client_id,
                                    new_sub.context,
                                    new_sub.patterns.len()
                                );
                                client_state.subscription = new_sub;
                            }
                        }
                    }
                }
            }
            FrameType::Close => {
                info!("WebSocket close frame received from client {}", client_id);
                // Remove client from broadcast list
                if let Ok(mut clients) = ws_clients_handler.lock() {
                    clients.remove(&client_id);
                }
            }
            _ => {}
        }

        Ok::<(), SignalKError>(())
    })?;

    info!("HTTP server started on port {}", config.http_port);
    Ok(server)
}

/// Generate demo navigation data
fn generate_demo_data(delta_tx: mpsc::Sender<Delta>) {
    info!("Demo data generator started");

    let mut latitude = 52.0987654;
    let mut longitude = 4.9876545;
    let mut counter: u64 = 0;

    loop {
        thread::sleep(Duration::from_secs(1));

        // Update position
        latitude += 0.00001;
        longitude += 0.00002;

        // Vary speed and course
        let sog = 3.85 + (counter as f64 * 0.1).sin() * 0.5;
        let cog = 1.52 + (counter as f64 * 0.1).cos() * 0.1;

        // Create delta (same structure as Linux version!)
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("demo.generator".to_string()),
                source: None,
                timestamp: Some(current_timestamp()),
                values: vec![
                    PathValue {
                        path: "navigation.position".to_string(),
                        value: json!({
                            "latitude": latitude,
                            "longitude": longitude
                        }),
                    },
                    PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: json!(sog),
                    },
                    PathValue {
                        path: "navigation.courseOverGroundTrue".to_string(),
                        value: json!(cog),
                    },
                ],
                meta: None,
            }],
        };

        if delta_tx.send(delta).is_err() {
            error!("Failed to send demo delta");
            break;
        }

        counter += 1;
    }
}
