use signalk_server::{ServerConfig, ServerEvent};
use signalk_core::{Delta, PathValue, Update, SignalKStore, MemoryStore};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use axum::{
    extract::{Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Json, IntoResponse},
    routing::get,
    Router,
};
use axum::extract::ws::{WebSocket, Message};
use tower_http::services::ServeDir;
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::{broadcast, RwLock};

type SharedStore = Arc<RwLock<MemoryStore>>;

#[derive(Clone)]
struct AppState {
    store: SharedStore,
    delta_tx: broadcast::Sender<Delta>,
    config: ServerConfig,
}

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

    // Configuration - single port for everything
    let addr: SocketAddr = "0.0.0.0:3001".parse()?;

    let config = ServerConfig {
        name: "signalk-server-rust".to_string(),
        version: "1.7.0".to_string(),
        bind_addr: addr,
        self_urn: "urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d".to_string(),
    };

    // Create server components
    let store = Arc::new(RwLock::new(MemoryStore::new(&config.self_urn)));
    let (delta_tx, _delta_rx) = broadcast::channel::<Delta>(1024);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<ServerEvent>(1024);
    
    // Clone for processors
    let store_clone = store.clone();
    let delta_tx_clone = delta_tx.clone();
    
    // Spawn delta processor
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                ServerEvent::DeltaReceived(delta) => {
                    // Store delta
                    {
                        let mut st = store_clone.write().await;
                        st.apply_delta(&delta);
                    }
                    // Broadcast to WebSocket clients
                    let _ = delta_tx_clone.send(delta);
                }
            }
        }
    });

    let app_state = AppState {
        store,
        delta_tx,
        config: config.clone(),
    };

    // Start unified HTTP + WebSocket server
    let http_handle = tokio::spawn(async move {
        if let Err(e) = start_unified_server(addr, app_state).await {
            tracing::error!("Server error: {}", e);
        }
    });

    // Start demo data generator
    let demo_handle = tokio::spawn(async move {
        generate_demo_data(event_tx).await;
    });

    tracing::info!("Server ready!");
    tracing::info!("");
    tracing::info!("   Admin UI:    http://localhost:3001/admin/");
    tracing::info!("   REST API:    http://localhost:3001/signalk/v1/api");
    tracing::info!("   WebSocket:   ws://localhost:3001/signalk/v1/stream");
    tracing::info!("   Docs:        http://localhost:3001/documentation/rapidoc.html");
    tracing::info!("");
    tracing::info!("Open http://localhost:3001/admin/ in your browser!");

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl+C, shutting down...");
        }
        _ = http_handle => {
            tracing::warn!("Server stopped");
        }
        _ = demo_handle => {
            tracing::warn!("Demo data generator stopped");
        }
    }

    tracing::info!("Shutdown complete");
    Ok(())
}

async fn start_unified_server(
    addr: SocketAddr,
    state: AppState,
) -> anyhow::Result<()> {
    // Serve admin UI from reference implementation
    let admin_ui_path = "/home/vadian/signalk-server/packages/server-admin-ui/public";
    let documentation_path = "/home/vadian/signalk-server/public";
    
    // Build router
    let app = Router::new()
        // WebSocket endpoint
        .route("/signalk/v1/stream", get(websocket_handler))
        // Discovery endpoint
        .route("/signalk", get(discovery_handler))
        // REST API endpoints
        .route("/signalk/v1/api", get(full_api_handler))
        .route("/signalk/v1/api/*path", get(path_handler))
        // Admin UI (React SPA)
        .nest_service("/admin", ServeDir::new(admin_ui_path))
        // Documentation
        .nest_service("/documentation", ServeDir::new(documentation_path))
        // Redirect root to admin UI
        .route("/", get(|| async { axum::response::Redirect::permanent("/admin/") }))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut delta_rx = state.delta_tx.subscribe();
    
    // Send Hello message
    let hello = signalk_protocol::HelloMessage {
        name: state.config.name.clone(),
        version: state.config.version.clone(),
        self_urn: state.config.self_urn.clone(),
        roles: vec!["master".to_string(), "main".to_string()],
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    };
    
    let hello_msg = signalk_protocol::ServerMessage::Hello(hello);
    if let Ok(json) = serde_json::to_string(&hello_msg) {
        if sender.send(Message::Text(json)).await.is_err() {
            return;
        }
    }
    
    // Spawn task to send deltas to client
    let mut send_task = tokio::spawn(async move {
        while let Ok(delta) = delta_rx.recv().await {
            let msg = signalk_protocol::ServerMessage::Delta(delta);
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });
    
    // Receive messages from client
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                tracing::debug!("Received: {}", text);
                // Handle subscribe/unsubscribe messages here
            } else if let Message::Close(_) = msg {
                break;
            }
        }
    });
    
    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
    
    tracing::debug!("WebSocket connection closed");
}

async fn discovery_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "endpoints": {
            "v1": {
                "version": "1.7.0",
                "signalk-http": "http://localhost:3001/signalk/v1/api",
                "signalk-ws": "ws://localhost:3001/signalk/v1/stream"
            }
        },
        "server": {
            "id": state.config.name,
            "version": "0.1.0"
        }
    }))
}

async fn full_api_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = state.store.read().await;
    Ok(Json(store.full_model().clone()))
}

async fn path_handler(
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = state.store.read().await;
    
    // Remove leading slash if present
    let path = path.strip_prefix('/').unwrap_or(&path);
    
    match store.get_path(path) {
        Some(value) => Ok(Json(value)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn generate_demo_data(event_tx: tokio::sync::mpsc::Sender<ServerEvent>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    let mut latitude = 52.0987654;
    let mut longitude = 4.9876545;
    
    loop {
        interval.tick().await;
        
        // Update position (move the boat)
        latitude += 0.00001;
        longitude += 0.00002;
        
        // Vary speed and course slightly
        let sog = 3.85 + (tokio::time::Instant::now().elapsed().as_secs_f64().sin() * 0.5);
        let cog = 1.52 + (tokio::time::Instant::now().elapsed().as_secs_f64().cos() * 0.1);
        
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
