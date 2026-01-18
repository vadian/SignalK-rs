//! Protocol message types for WebSocket communication.

use serde::{Deserialize, Serialize};

/// Subscription request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub context: String,
    pub subscribe: Vec<Subscription>,
}

/// A single subscription specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<SubscriptionFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<SubscriptionPolicy>,
    #[serde(rename = "minPeriod", skip_serializing_if = "Option::is_none")]
    pub min_period: Option<u64>,
}

/// Subscription format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionFormat {
    Delta,
    Full,
}

/// Subscription policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionPolicy {
    Instant,
    Ideal,
    Fixed,
}

/// Unsubscribe request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeRequest {
    pub context: String,
    pub unsubscribe: Vec<UnsubscribeSpec>,
}

/// Unsubscribe specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeSpec {
    pub path: String,
}

/// PUT request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub put: PutSpec,
}

/// PUT specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutSpec {
    pub path: String,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// PUT response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutResponse {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub state: PutState,
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// PUT request state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PutState {
    Completed,
    Pending,
    Failed,
}
