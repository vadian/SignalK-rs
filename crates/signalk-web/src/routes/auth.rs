//! Authentication routes.
//!
//! These endpoints handle user authentication and access requests,
//! matching the TypeScript SignalK server API for Admin UI compatibility.
//!
//! # Authentication Flow
//!
//! 1. Client checks `/skServer/loginStatus` to see if auth is required
//! 2. If security enabled, client posts credentials to `/signalk/v1/auth/login`
//! 3. Server returns JWT token on success
//! 4. Client includes token in subsequent requests via `Authorization: Bearer <token>`
//!
//! # Device Access Flow
//!
//! 1. Device posts to `/signalk/v1/access/requests` with client ID
//! 2. Admin approves/denies via Admin UI
//! 3. Device polls `/signalk/v1/requests/:id` for status
//! 4. On approval, device receives permanent token
//!
//! # Endpoints
//!
//! ## Login Status
//!
//! ### `GET /skServer/loginStatus`
//! Returns the current authentication status.
//!
//! **Response (security disabled):**
//! ```json
//! {
//!   "status": "notLoggedIn",
//!   "readOnlyAccess": false,
//!   "authenticationRequired": false,
//!   "allowNewUserRegistration": false,
//!   "allowDeviceAccessRequests": true
//! }
//! ```
//!
//! **Response (logged in):**
//! ```json
//! {
//!   "status": "loggedIn",
//!   "username": "admin",
//!   "userLevel": "admin"
//! }
//! ```
//!
//! ## Login/Logout
//!
//! ### `POST /signalk/v1/auth/login`
//! Authenticate with username and password.
//!
//! **Request:**
//! ```json
//! {
//!   "username": "admin",
//!   "password": "secret"
//! }
//! ```
//!
//! **Response (success):**
//! ```json
//! {
//!   "token": "eyJhbGciOiJIUzI1NiIs..."
//! }
//! ```
//!
//! **Response (failure):** `401 Unauthorized`
//!
//! ### `PUT /signalk/v1/auth/logout`
//! Invalidate the current session.
//!
//! **Response:** `200 OK`
//!
//! ## Device Access Requests
//!
//! ### `POST /signalk/v1/access/requests`
//! Request access for a new device.
//!
//! **Request:**
//! ```json
//! {
//!   "clientId": "device-uuid",
//!   "description": "My GPS device"
//! }
//! ```
//!
//! **Response:**
//! ```json
//! {
//!   "requestId": "request-uuid",
//!   "href": "/signalk/v1/requests/request-uuid"
//! }
//! ```
//!
//! ### `GET /signalk/v1/requests/:id`
//! Check status of an access request.
//!
//! **Response (pending):**
//! ```json
//! {
//!   "state": "PENDING",
//!   "requestId": "request-uuid"
//! }
//! ```
//!
//! **Response (approved):**
//! ```json
//! {
//!   "state": "COMPLETED",
//!   "requestId": "request-uuid",
//!   "accessRequest": {
//!     "permission": "readwrite",
//!     "token": "eyJhbGciOiJIUzI1NiIs..."
//!   }
//! }
//! ```

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Login status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus {
    pub status: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_level: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_access: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_required: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_new_user_registration: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_device_access_requests: Option<bool>,
}

/// Login request.
#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response.
#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub token: String,
}

/// Device access request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessRequest {
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Access request response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessRequestResponse {
    pub request_id: String,
    pub href: String,
}

/// Access request status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestStatus {
    pub state: String,
    pub request_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_request: Option<AccessGranted>,
}

/// Granted access details.
#[derive(Debug, Clone, Serialize)]
pub struct AccessGranted {
    pub permission: String,
    pub token: String,
}

/// Create authentication routes for /skServer/*.
pub fn server_routes() -> Router<AppState> {
    Router::new().route("/loginStatus", get(get_login_status))
}

/// Create authentication routes for /signalk/v1/auth/*.
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(post_login))
        .route("/logout", put(put_logout))
}

/// Create access request routes for /signalk/v1/*.
pub fn access_routes() -> Router<AppState> {
    Router::new()
        .route("/access/requests", post(post_access_request))
        .route("/requests/:id", get(get_request_status))
}

/// GET /skServer/loginStatus
async fn get_login_status(State(_state): State<AppState>) -> Json<LoginStatus> {
    // TODO: Check actual authentication state
    Json(LoginStatus {
        status: "notLoggedIn".to_string(),
        username: None,
        user_level: None,
        read_only_access: Some(false),
        authentication_required: Some(false),
        allow_new_user_registration: Some(false),
        allow_device_access_requests: Some(true),
    })
}

/// POST /signalk/v1/auth/login
async fn post_login(
    State(_state): State<AppState>,
    Json(_request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // TODO: Implement authentication
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// PUT /signalk/v1/auth/logout
async fn put_logout(State(_state): State<AppState>) -> StatusCode {
    // TODO: Invalidate session
    StatusCode::OK
}

/// POST /signalk/v1/access/requests
async fn post_access_request(
    State(_state): State<AppState>,
    Json(request): Json<AccessRequest>,
) -> Json<AccessRequestResponse> {
    // TODO: Create pending access request
    let request_id = uuid::Uuid::new_v4().to_string();
    Json(AccessRequestResponse {
        href: format!("/signalk/v1/requests/{}", request_id),
        request_id,
    })
}

/// GET /signalk/v1/requests/:id
async fn get_request_status(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Json<RequestStatus> {
    // TODO: Look up actual request status
    Json(RequestStatus {
        state: "PENDING".to_string(),
        request_id: id,
        access_request: None,
    })
}
