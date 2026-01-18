//! Security management routes.
//!
//! These endpoints manage users, devices, and security configuration,
//! matching the TypeScript SignalK server API for Admin UI compatibility.
//!
//! # Security Model
//!
//! SignalK supports multiple security strategies. The default is token-based:
//! - Users authenticate with username/password, receive JWT
//! - Devices request access, admin approves, device receives permanent token
//! - Tokens carry permission level (admin, readwrite, readonly)
//!
//! # Endpoints
//!
//! ## Security Configuration
//!
//! ### `GET /skServer/security/config`
//! Get current security configuration.
//!
//! **Response:**
//! ```json
//! {
//!   "allowReadOnly": false,
//!   "expiration": "1d",
//!   "allowNewUserRegistration": false,
//!   "allowDeviceAccessRequests": true
//! }
//! ```
//!
//! ### `PUT /skServer/security/config`
//! Update security configuration.
//!
//! ## User Management
//!
//! ### `GET /skServer/security/users`
//! List all users.
//!
//! **Response:**
//! ```json
//! [
//!   { "userId": "admin", "type": "admin" },
//!   { "userId": "guest", "type": "readonly" }
//! ]
//! ```
//!
//! ### `POST /skServer/security/users/:id`
//! Create a new user.
//!
//! **Request:**
//! ```json
//! {
//!   "userId": "newuser",
//!   "type": "readwrite",
//!   "password": "secret"
//! }
//! ```
//!
//! ### `PUT /skServer/security/users/:id`
//! Update a user.
//!
//! ### `DELETE /skServer/security/users/:username`
//! Delete a user.
//!
//! ### `PUT /skServer/security/user/:username/password`
//! Change a user's password.
//!
//! **Request:**
//! ```json
//! {
//!   "password": "newpassword"
//! }
//! ```
//!
//! ## Device Management
//!
//! ### `GET /skServer/security/devices`
//! List all authorized devices.
//!
//! **Response:**
//! ```json
//! [
//!   {
//!     "clientId": "device-uuid",
//!     "description": "Chart plotter",
//!     "permissions": "readwrite"
//!   }
//! ]
//! ```
//!
//! ### `PUT /skServer/security/devices/:uuid`
//! Update device permissions.
//!
//! ### `DELETE /skServer/security/devices/:uuid`
//! Remove a device.
//!
//! ## Access Requests
//!
//! ### `GET /skServer/security/access/requests`
//! List pending access requests.
//!
//! **Response:**
//! ```json
//! [
//!   {
//!     "requestId": "request-uuid",
//!     "clientId": "device-uuid",
//!     "description": "New device",
//!     "timestamp": "2024-01-17T10:00:00Z"
//!   }
//! ]
//! ```
//!
//! ### `PUT /skServer/security/access/requests/:id/:status`
//! Approve or deny an access request.
//!
//! - `/skServer/security/access/requests/{id}/approved` - Grant access
//! - `/skServer/security/access/requests/{id}/denied` - Deny access
//!
//! ## Initial Setup
//!
//! ### `POST /skServer/enableSecurity`
//! Enable security on a fresh install.
//!
//! **Request:**
//! ```json
//! {
//!   "userId": "admin",
//!   "password": "secret",
//!   "type": "admin"
//! }
//! ```

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Security configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_read_only: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_new_user_registration: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_device_access_requests: Option<bool>,
}

/// User information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub user_id: String,

    #[serde(rename = "type")]
    pub user_type: String,

    /// Password - only used in requests, never returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Password change request.
#[derive(Debug, Clone, Deserialize)]
pub struct PasswordChange {
    pub password: String,
}

/// Device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub client_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub permissions: String,
}

/// Pending access request.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRequest {
    pub request_id: String,
    pub client_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub timestamp: String,
}

/// Create security routes for /skServer/security/*.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/config", get(get_config).put(put_config))
        .route("/users", get(get_users))
        .route("/users/:id", post(create_user).put(update_user))
        .route("/users/:username", delete(delete_user))
        .route("/user/:username/password", put(change_password))
        .route("/devices", get(get_devices))
        .route("/devices/:uuid", put(update_device).delete(delete_device))
        .route("/access/requests", get(get_access_requests))
        .route("/access/requests/:id/:status", put(handle_access_request))
}

/// Create route for /skServer/enableSecurity.
pub fn enable_security_route() -> Router<AppState> {
    Router::new().route("/enableSecurity", post(enable_security))
}

/// GET /skServer/security/config
async fn get_config(State(_state): State<AppState>) -> Json<SecurityConfig> {
    Json(SecurityConfig {
        allow_read_only: Some(false),
        expiration: Some("1d".to_string()),
        allow_new_user_registration: Some(false),
        allow_device_access_requests: Some(true),
    })
}

/// PUT /skServer/security/config
async fn put_config(
    State(_state): State<AppState>,
    Json(_config): Json<SecurityConfig>,
) -> StatusCode {
    // TODO: Save security configuration
    StatusCode::OK
}

/// GET /skServer/security/users
async fn get_users(State(_state): State<AppState>) -> Json<Vec<User>> {
    // TODO: Load users from security file
    Json(vec![User {
        user_id: "admin".to_string(),
        user_type: "admin".to_string(),
        password: None,
    }])
}

/// POST /skServer/security/users/:id
async fn create_user(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
    Json(_user): Json<User>,
) -> StatusCode {
    // TODO: Create user
    StatusCode::CREATED
}

/// PUT /skServer/security/users/:id
async fn update_user(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
    Json(_user): Json<User>,
) -> StatusCode {
    // TODO: Update user
    StatusCode::OK
}

/// DELETE /skServer/security/users/:username
async fn delete_user(State(_state): State<AppState>, Path(_username): Path<String>) -> StatusCode {
    // TODO: Delete user
    StatusCode::OK
}

/// PUT /skServer/security/user/:username/password
async fn change_password(
    State(_state): State<AppState>,
    Path(_username): Path<String>,
    Json(_password): Json<PasswordChange>,
) -> StatusCode {
    // TODO: Change password
    StatusCode::OK
}

/// GET /skServer/security/devices
async fn get_devices(State(_state): State<AppState>) -> Json<Vec<Device>> {
    // TODO: Load devices from security file
    Json(vec![])
}

/// PUT /skServer/security/devices/:uuid
async fn update_device(
    State(_state): State<AppState>,
    Path(_uuid): Path<String>,
    Json(_device): Json<Device>,
) -> StatusCode {
    // TODO: Update device
    StatusCode::OK
}

/// DELETE /skServer/security/devices/:uuid
async fn delete_device(State(_state): State<AppState>, Path(_uuid): Path<String>) -> StatusCode {
    // TODO: Delete device
    StatusCode::OK
}

/// GET /skServer/security/access/requests
async fn get_access_requests(State(_state): State<AppState>) -> Json<Vec<PendingRequest>> {
    // TODO: Load pending requests
    Json(vec![])
}

/// PUT /skServer/security/access/requests/:id/:status
async fn handle_access_request(
    State(_state): State<AppState>,
    Path((id, status)): Path<(String, String)>,
) -> StatusCode {
    // TODO: Approve or deny request
    match status.as_str() {
        "approved" => {
            // Grant access
            StatusCode::OK
        }
        "denied" => {
            // Deny access
            StatusCode::OK
        }
        _ => StatusCode::BAD_REQUEST,
    }
}

/// POST /skServer/enableSecurity
async fn enable_security(State(_state): State<AppState>, Json(_user): Json<User>) -> StatusCode {
    // TODO: Enable security with initial admin user
    StatusCode::OK
}
