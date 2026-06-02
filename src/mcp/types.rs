//! JSON-RPC 2.0 message types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct McpRequest {
    #[allow(dead_code)]
    pub(super) jsonrpc: String,
    /// Absent for notifications; present for requests that require a response.
    pub(super) id: Option<Value>,
    pub(super) method: String,
    #[serde(default)]
    pub(super) params: Value,
}

#[derive(Debug, Serialize)]
pub(super) struct McpResponse {
    jsonrpc: &'static str,
    pub(super) id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
pub(super) struct RpcError {
    pub(super) code: i32,
    pub(super) message: String,
}

impl McpResponse {
    pub(super) fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    pub(super) fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}
