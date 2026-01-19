use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use signalk_core::{Delta, MemoryStore, PathValue, SignalKStore, Update};
use signalk_server::{ServerConfig, ServerEvent};
use signalk_web::{
    DebugSettings, LoginStatus, ServerEvent as WebServerEvent, ServerStatistics, SourcePriorities,
    VesselInfoData, WebConfig, WebState,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

type SharedStore = Arc<RwLock<MemoryStore>>;

#[derive(Clone)]
struct AppState {
    store: SharedStore,
    delta_tx: broadcast::Sender<Delta>,
    config: ServerConfig,
    web_state: Arc<WebState>,
}

#[derive(Debug, Deserialize)]
struct StreamQuery {
    #[serde(default)]
    subscribe: Option<String>,
    #[serde(default)]
    serverevents: Option<String>,
    #[serde(rename = "sendCachedValues", default)]
    send_cached_values: Option<bool>,
    #[serde(rename = "sendMeta", default)]
    send_meta: Option<String>,
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
    let addr: SocketAddr = "0.0.0.0:4000".parse()?;

    let config = ServerConfig {
        name: "signalk-server-rust".to_string(),
        version: "1.7.0".to_string(),
        bind_addr: addr,
        // self_urn must include "vessels." prefix per Signal K spec
        self_urn: "vessels.urn:mrn:signalk:uuid:c0d79334-4e25-4245-8892-54e8ccc8021d".to_string(),
    };

    // Create server components
    let store = Arc::new(RwLock::new(MemoryStore::new(&config.self_urn)));
    let (delta_tx, _delta_rx) = broadcast::channel::<Delta>(1024);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<ServerEvent>(1024);

    // Create web state for Admin UI
    let web_config = WebConfig {
        name: config.name.clone(),
        version: config.version.clone(),
        self_urn: config.self_urn.clone(),
    };
    let web_state = Arc::new(WebState::new(store.clone(), web_config));

    // Clone for processors
    let store_clone = store.clone();
    let delta_tx_clone = delta_tx.clone();
    let web_state_clone = web_state.clone();

    // Spawn delta processor
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                ServerEvent::DeltaReceived(delta) => {
                    // Record in statistics
                    web_state_clone.statistics.record_delta();

                    // Store delta
                    {
                        let mut st = store_clone.write().await;
                        st.apply_delta(&delta);

                        // Update path count
                        web_state_clone.statistics.set_active_paths(st.path_count());
                    }
                    // Broadcast to WebSocket clients
                    let _ = delta_tx_clone.send(delta);
                }
            }
        }
    });

    // Spawn statistics broadcaster (1 Hz)
    let web_state_stats = web_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            interval.tick().await;

            // Update rate calculation
            web_state_stats.statistics.update_rate();

            // Broadcast statistics to admin UI clients
            let stats = web_state_stats.statistics.snapshot();
            web_state_stats.broadcast_event(WebServerEvent::ServerStatistics {
                from: "signalk-server".to_string(),
                data: stats,
            });
        }
    });

    let app_state = AppState {
        store,
        delta_tx,
        config: config.clone(),
        web_state,
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
    tracing::info!("   Admin UI:    http://localhost:4000/admin/");
    tracing::info!("   REST API:    http://localhost:4000/signalk/v1/api");
    tracing::info!("   WebSocket:   ws://localhost:4000/signalk/v1/stream");
    tracing::info!("   Settings:    http://localhost:4000/skServer/settings");
    tracing::info!("");
    tracing::info!("Open http://localhost:4000/admin/ in your browser!");

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

async fn start_unified_server(addr: SocketAddr, state: AppState) -> anyhow::Result<()> {
    // Serve admin UI from reference implementation
    let admin_ui_path = "/home/vadian/signalk-server/packages/server-admin-ui/public";
    let documentation_path = "/home/vadian/signalk-server/public";

    // Build router with all routes defined inline
    let app = Router::new()
        // WebSocket endpoint (handles both deltas and server events)
        .route("/signalk/v1/stream", get(websocket_handler))
        // REST API endpoints for SignalK data
        .route("/signalk/v1/api", get(full_api_handler))
        .route("/signalk/v1/api/*path", get(path_handler))
        // Discovery endpoint
        .route("/signalk", get(discovery_handler))
        // Sources list endpoint (for Data Browser)
        .route("/sources", get(sources_list_handler))
        // Admin UI REST API endpoints
        .route("/skServer/loginStatus", get(login_status_handler))
        .route(
            "/skServer/settings",
            get(get_settings_handler).put(put_settings_handler),
        )
        .route(
            "/skServer/vessel",
            get(get_vessel_handler).put(put_vessel_handler),
        )
        .route("/skServer/plugins", get(get_plugins_handler))
        .route("/skServer/webapps", get(get_webapps_handler))
        .route(
            "/skServer/security/config",
            get(get_security_config_handler),
        )
        .route("/skServer/security/users", get(get_users_handler))
        .route("/skServer/security/devices", get(get_devices_handler))
        .route(
            "/skServer/backup",
            axum::routing::post(create_backup_handler),
        )
        .route("/skServer/restart", axum::routing::put(restart_handler))
        .route("/skServer/debugKeys", get(debug_keys_handler))
        .route("/skServer/addons", get(get_addons_handler))
        .route(
            "/skServer/appstore/available",
            get(get_appstore_available_handler),
        )
        .route(
            "/skServer/security/access/requests",
            get(get_access_requests_handler),
        )
        .route("/signalk/v1/apps/list", get(app_list_handler))
        // Admin UI (React SPA)
        .nest_service("/admin", ServeDir::new(admin_ui_path))
        // Documentation
        .nest_service("/documentation", ServeDir::new(documentation_path))
        // Redirect root to admin UI
        .route(
            "/",
            get(|| async { axum::response::Redirect::permanent("/admin/") }),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

// ============================================================================
// REST API Handlers for Admin UI
// ============================================================================

async fn discovery_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "endpoints": {
            "v1": {
                "version": "1.7.0",
                "signalk-http": "http://localhost:4000/signalk/v1/api",
                "signalk-ws": "ws://localhost:4000/signalk/v1/stream"
            }
        },
        "server": {
            "id": state.config.name,
            "version": "0.1.0"
        }
    }))
}

async fn sources_list_handler() -> Json<Vec<serde_json::Value>> {
    // Return empty array of sources for now
    // TODO: Populate with actual data sources when providers are implemented
    Json(vec![])
}

async fn login_status_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "notLoggedIn",
        "readOnlyAccess": false,
        "authenticationRequired": false,
        "allowNewUserRegistration": false,
        "allowDeviceAccessRequests": true
    }))
}

async fn get_settings_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let settings = state.web_state.settings.read().await;
    Json(serde_json::json!({
        "interfaces": {
            "appstore": true,
            "plugins": true,
            "rest": true,
            "signalk-ws": true,
            "tcp": false,
            "webapps": true
        },
        "port": settings.port.unwrap_or(4000),
        "ssl": settings.ssl.unwrap_or(false),
        "wsCompression": false,
        "accessLogging": false,
        "mdns": true,
        "pruneContextsMinutes": 60,
        "loggingDirectory": "~/.signalk/logs",
        "keepMostRecentLogsOnly": true,
        "logCountToKeep": 24,
        "enablePluginLogging": true
    }))
}

async fn put_settings_handler() -> StatusCode {
    StatusCode::OK
}

async fn get_vessel_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let vessel = state.web_state.vessel_info.read().await;
    Json(serde_json::json!({
        "name": vessel.name,
        "mmsi": vessel.mmsi,
        "uuid": state.config.self_urn
    }))
}

async fn put_vessel_handler() -> StatusCode {
    StatusCode::OK
}

async fn get_plugins_handler() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn get_webapps_handler() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn get_security_config_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "allowReadOnly": false,
        "expiration": "1d",
        "allowNewUserRegistration": false,
        "allowDeviceAccessRequests": true
    }))
}

async fn get_users_handler() -> Json<Vec<serde_json::Value>> {
    Json(vec![serde_json::json!({
        "userId": "admin",
        "type": "admin"
    })])
}

async fn get_devices_handler() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn create_backup_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "href": "/skServer/backup"
    }))
}

async fn restart_handler() -> StatusCode {
    StatusCode::OK
}

async fn debug_keys_handler() -> Json<Vec<String>> {
    Json(vec![
        "signalk-server:*".to_string(),
        "signalk-server:interfaces:*".to_string(),
        "signalk-server:providers:*".to_string(),
    ])
}

async fn app_list_handler() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn get_addons_handler() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn get_appstore_available_handler() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn get_access_requests_handler() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

// ============================================================================
// WebSocket Handlers
// ============================================================================

async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<StreamQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let subscribe_mode = query
        .subscribe
        .clone()
        .unwrap_or_else(|| "self".to_string());
    let send_cached_values = query.send_cached_values.unwrap_or(true);
    let send_server_events = query.serverevents.as_deref() == Some("all");

    ws.on_upgrade(move |socket| {
        handle_websocket(
            socket,
            state,
            subscribe_mode,
            send_cached_values,
            send_server_events,
        )
    })
}

async fn handle_websocket(
    socket: WebSocket,
    state: AppState,
    _subscribe_mode: String,
    _send_cached_values: bool,
    send_server_events: bool,
) {
    let (mut sender, mut receiver) = socket.split();

    // Track client connection
    state.web_state.statistics.client_connected();

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
            state.web_state.statistics.client_disconnected();
            return;
        }
    }

    // Send initial server events if requested (for Admin UI Dashboard)
    if send_server_events {
        // Extract UUID from self_urn (remove "vessels." prefix)
        let uuid = state
            .config
            .self_urn
            .strip_prefix("vessels.")
            .unwrap_or(&state.config.self_urn)
            .to_string();

        // Get vessel name from state
        let vessel_name = state.web_state.vessel_info.read().await.name.clone();

        // Send VESSEL_INFO
        let vessel_info = WebServerEvent::VesselInfo {
            data: VesselInfoData {
                name: vessel_name,
                uuid,
            },
        };
        if let Ok(json) = serde_json::to_string(&vessel_info) {
            if sender.send(Message::Text(json)).await.is_err() {
                state.web_state.statistics.client_disconnected();
                return;
            }
        }

        // Send PROVIDERSTATUS (empty for now)
        let provider_status = WebServerEvent::ProviderStatus {
            from: "signalk-server".to_string(),
            data: vec![],
        };
        if let Ok(json) = serde_json::to_string(&provider_status) {
            let _ = sender.send(Message::Text(json)).await;
        }

        // Send SERVERSTATISTICS
        let stats = state.web_state.statistics.snapshot();
        let server_stats = WebServerEvent::ServerStatistics {
            from: "signalk-server".to_string(),
            data: stats,
        };
        if let Ok(json) = serde_json::to_string(&server_stats) {
            let _ = sender.send(Message::Text(json)).await;
        }

        // Send DEBUG_SETTINGS
        let debug_settings = WebServerEvent::DebugSettings {
            data: DebugSettings::default(),
        };
        if let Ok(json) = serde_json::to_string(&debug_settings) {
            let _ = sender.send(Message::Text(json)).await;
        }

        // Send RECEIVE_LOGIN_STATUS
        let login_status = WebServerEvent::LoginStatus {
            data: LoginStatus::default(),
        };
        if let Ok(json) = serde_json::to_string(&login_status) {
            let _ = sender.send(Message::Text(json)).await;
        }

        // Send SOURCEPRIORITIES
        let source_priorities = WebServerEvent::SourcePriorities {
            data: SourcePriorities::default(),
        };
        if let Ok(json) = serde_json::to_string(&source_priorities) {
            let _ = sender.send(Message::Text(json)).await;
        }
    }

    // Normal delta streaming mode
    let mut delta_rx = state.delta_tx.subscribe();

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

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    state.web_state.statistics.client_disconnected();
    tracing::debug!("WebSocket connection closed");
}

// ============================================================================
// SignalK Data API Handlers
// ============================================================================

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

    // Convert URL path separators to SignalK dot notation
    let path = path.replace('/', ".");

    match store.get_path(&path) {
        Some(value) => Ok(Json(value)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// ============================================================================
// Demo Data Generator
// ============================================================================

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
                timestamp: Some(
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                ),
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
        if event_tx
            .send(ServerEvent::DeltaReceived(delta))
            .await
            .is_err()
        {
            tracing::error!("Failed to send demo delta - server may have stopped");
            break;
        }
    }
}
