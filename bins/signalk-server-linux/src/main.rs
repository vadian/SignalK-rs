use signalk_server::{ServerConfig, ServerEvent, SignalKServer};
use signalk_core::{Delta, PathValue, Update};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,signalk_server=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("SignalK Server starting...");

    // Configuration
    let ws_addr: SocketAddr = "0.0.0.0:3000".parse()?;
    let http_addr: SocketAddr = "0.0.0.0:3001".parse()?;

    let config = ServerConfig {
        name: "signalk-server-rust".to_string(),
        version: "1.7.0".to_string(),
        bind_addr: ws_addr,
        self_urn: "urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d".to_string(),
    };

    // Start WebSocket server
    let server = SignalKServer::new(config);
    let event_tx = server.event_sender();
    
    // Clone for HTTP server
    let store = server.store();
    
    // Spawn WebSocket server
    let ws_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            tracing::error!("WebSocket server error: {}", e);
        }
    });

    // Start HTTP API server
    let http_handle = tokio::spawn(async move {
        if let Err(e) = start_http_server(http_addr, store).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    // Start demo data generator
    let demo_handle = tokio::spawn(async move {
        generate_demo_data(event_tx).await;
    });

    tracing::info!("🚀 SignalK Server ready!");
    tracing::info!("   WebSocket: ws://localhost:3000/signalk/v1/stream");
    tracing::info!("   HTTP API:  http://localhost:3001/signalk/v1/api");
    tracing::info!("   Discovery: http://localhost:3001/signalk");
    tracing::info!("");
    tracing::info!("Try these commands:");
    tracing::info!("   curl http://localhost:3001/signalk");
    tracing::info!("   curl http://localhost:3001/signalk/v1/api/vessels/self/navigation/position");
    tracing::info!("   websocat ws://localhost:3000/signalk/v1/stream");

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl+C, shutting down...");
        }
        _ = ws_handle => {
            tracing::warn!("WebSocket server stopped");
        }
        _ = http_handle => {
            tracing::warn!("HTTP server stopped");
        }
        _ = demo_handle => {
            tracing::warn!("Demo data generator stopped");
        }
    }

    tracing::info!("Shutdown complete");
    Ok(())
}

/// Start the HTTP API server
async fn start_http_server(
    addr: SocketAddr,
    store: std::sync::Arc<tokio::sync::RwLock<signalk_core::MemoryStore>>,
) -> anyhow::Result<()> {
    // Shared state
    let app_state = store;

    // Build router
    let app = Router::new()
        .route("/signalk", get(discovery_handler))
        .route("/signalk/v1/api", get(full_api_handler))
        .route("/signalk/v1/api/*path", get(path_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("HTTP server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Discovery endpoint handler
async fn discovery_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "endpoints": {
            "v1": {
                "version": "1.7.0",
                "signalk-http": "http://localhost:3001/signalk/v1/api",
                "signalk-ws": "ws://localhost:3000/signalk/v1/stream"
            }
        },
        "server": {
            "id": "signalk-server-rust",
            "version": "0.1.0"
        }
    }))
}

/// Full API handler - returns entire data model
async fn full_api_handler(
    State(store): State<std::sync::Arc<tokio::sync::RwLock<signalk_core::MemoryStore>>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use signalk_core::SignalKStore;
    let store = store.read().await;
    Ok(Json(store.full_model().clone()))
}

/// Path-based API handler
async fn path_handler(
    Path(path): Path<String>,
    State(store): State<std::sync::Arc<tokio::sync::RwLock<signalk_core::MemoryStore>>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use signalk_core::SignalKStore;
    let store = store.read().await;
    
    // Remove leading slash if present
    let path = path.strip_prefix('/').unwrap_or(&path);
    
    match store.get_path(path) {
        Some(value) => Ok(Json(value)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Generate demo data - simulated boat navigation
async fn generate_demo_data(event_tx: tokio::sync::mpsc::Sender<ServerEvent>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    let mut latitude = 52.0987654;
    let mut longitude = 4.9876545;
    let mut sog = 3.85; // Speed over ground (m/s)
    let mut cog = 1.52; // Course over ground (radians)
    
    loop {
        interval.tick().await;
        
        // Update position (move the boat)
        latitude += 0.00001;
        longitude += 0.00002;
        
        // Vary speed and course slightly
        sog = 3.85 + (tokio::time::Instant::now().elapsed().as_secs_f64().sin() * 0.5);
        cog = 1.52 + (tokio::time::Instant::now().elapsed().as_secs_f64().cos() * 0.1);
        
        // Create delta message
        let delta = Delta {
            context: Some("vessels.self".to_string()),
            updates: vec![Update {
                source_ref: Some("demo.generator".to_string()),
                source: None,
                timestamp: Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
                values: vec![
                    PathValue {
                        path: "navigation.position".to_string(),
                        value: serde_json::json!({
                            "latitude": latitude,
                            "longitude": longitude
                        }),
                    },
                    PathValue {
                        path: "navigation.speedOverGround".to_string(),
                        value: serde_json::json!(sog),
                    },
                    PathValue {
                        path: "navigation.courseOverGroundTrue".to_string(),
                        value: serde_json::json!(cog),
                    },
                ],
                meta: None,
            }],
        };
        
        // Send to server
        if event_tx.send(ServerEvent::DeltaReceived(delta)).await.is_err() {
            tracing::error!("Failed to send demo delta - server may have stopped");
            break;
        }
    }
}
