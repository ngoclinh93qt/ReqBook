//! JSON-RPC dispatch: routes incoming requests to the appropriate handler.

use serde_json::{json, Value};

use super::{
    resources::{handle_resources_list, handle_resources_read},
    tools::{
        handle_author, handle_exec, handle_exec_batch, handle_flow, handle_history, handle_search,
        handle_session, handle_vars, tools_list_result,
    },
    types::{McpRequest, McpResponse},
};

/// Dispatch one parsed JSON-RPC request and return the serialised response.
/// Returns an empty string for notifications (which must not be responded to).
pub(super) async fn dispatch(req: McpRequest) -> String {
    if req.id.is_none() && (req.method.starts_with("notifications/") || req.method == "initialized")
    {
        return String::new();
    }

    let id = req.id.clone().unwrap_or(Value::Null);

    let resp = match req.method.as_str() {
        "initialize" => McpResponse::ok(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": {
                    "name": "mad",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        ),
        "tools/list" => McpResponse::ok(id, tools_list_result()),
        "tools/call" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Object(Default::default()));
            match name {
                "mad_exec" => match handle_exec(&args).await {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "mad_flow" => match handle_flow(&args).await {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "mad_author" => match handle_author(&args) {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "mad_vars" => match handle_vars(&args).await {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "mad_search" => match handle_search(&args).await {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "mad_history" => match handle_history(&args).await {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "mad_session" => match handle_session(&args) {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "mad_exec_batch" => match handle_exec_batch(&args).await {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                other => McpResponse::err(
                    id,
                    -32601,
                    format!("Method not found: unknown tool \"{other}\""),
                ),
            }
        }
        "resources/list" => McpResponse::ok(id, handle_resources_list()),
        "resources/read" => match handle_resources_read(&req.params) {
            Ok(r) => McpResponse::ok(id, r),
            Err((code, msg)) => McpResponse::err(id, code, msg),
        },
        other => McpResponse::err(id, -32601, format!("Method not found: \"{other}\"")),
    };

    serde_json::to_string(&resp).unwrap_or_default()
}
