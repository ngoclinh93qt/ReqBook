//! MCP (Model Context Protocol) server over stdio.
//!
//! Exposes tools to any MCP-compatible AI agent:
//! - `rqb_exec`          execute one endpoint spec
//! - `rqb_diagnose`      diagnose one endpoint failure
//! - `rqb_flow`          execute a pipeline
//! - `rqb_author`        create or update a spec file
//! - `rqb_vars`          show variable resolution for a spec
//! - `rqb_search`        search specs by method/path/tag/text
//! - `rqb_history`       execution history for a spec
//! - `rqb_session`       get/set session context (env + vars)
//! - `rqb_exec_batch`    execute multiple specs in one call
//!
//! Transport: JSON-RPC 2.0 over stdio (NDJSON, one message per line).
//! Protocol version: 2024-11-05.

mod dispatch;
mod resources;
mod session;
mod tools;
mod types;
mod util;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use dispatch::dispatch;
use types::{McpRequest, McpResponse};

// Re-export internals needed by tests via `use super::*`.
#[cfg(test)]
use tools::tools_list_result;

/// Run the MCP server until stdin is closed (EOF).
///
/// Reads newline-delimited JSON-RPC 2.0 messages from stdin and writes
/// responses to stdout. Designed to be launched by an MCP client as a
/// subprocess using the stdio transport.
pub async fn run_mcp_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<McpRequest>(&line) {
            Err(e) => {
                let resp =
                    McpResponse::err(serde_json::Value::Null, -32700, format!("Parse error: {e}"));
                serde_json::to_string(&resp).unwrap_or_default()
            }
            Ok(req) => dispatch(req).await,
        };

        if response.is_empty() {
            continue;
        }

        stdout.write_all(response.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn make_req(id: i64, method: &str, params: Value) -> McpRequest {
        McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(id)),
            method: method.to_string(),
            params,
        }
    }

    fn make_notification(method: &str) -> McpRequest {
        McpRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.to_string(),
            params: json!({}),
        }
    }

    // ── Serialisation ──

    #[test]
    fn response_ok_excludes_error_field() {
        let resp = McpResponse::ok(json!(1), json!({"answer": 42}));
        let v: Value = serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["result"]["answer"], 42);
        assert!(v.get("error").map(|e| e.is_null()).unwrap_or(true));
    }

    #[test]
    fn response_err_excludes_result_field() {
        let resp = McpResponse::err(json!(2), -32601, "Method not found");
        let v: Value = serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["error"]["code"], -32601);
        assert_eq!(v["error"]["message"], "Method not found");
        assert!(v.get("result").map(|e| e.is_null()).unwrap_or(true));
    }

    // ── tools/list ──

    #[test]
    fn tools_list_has_ten_tools() {
        let list = tools_list_result();
        let tools = list["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 10, "expected 10 tools, got {}", tools.len());
    }

    #[test]
    fn tools_list_names_are_correct() {
        let list = tools_list_result();
        let names: Vec<&str> = list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"rqb_exec"));
        assert!(names.contains(&"rqb_diagnose"));
        assert!(names.contains(&"rqb_flow"));
        assert!(names.contains(&"rqb_author"));
        assert!(names.contains(&"rqb_vars"));
        assert!(names.contains(&"rqb_search"));
        assert!(names.contains(&"rqb_history"));
        assert!(names.contains(&"rqb_session"));
        assert!(names.contains(&"rqb_exec_batch"));
        assert!(names.contains(&"rqb_context"));
    }

    #[test]
    fn each_tool_has_input_schema_with_required() {
        let list = tools_list_result();
        for tool in list["tools"].as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            assert!(
                tool.get("inputSchema").is_some(),
                "{name}: missing inputSchema"
            );
            assert!(
                tool["inputSchema"].get("required").is_some(),
                "{name}: inputSchema missing required"
            );
        }
    }

    // ── dispatch: initialize ──

    #[tokio::test]
    async fn dispatch_initialize_returns_protocol_version() {
        let req = make_req(1, "initialize", json!({}));
        let s = dispatch(req).await;
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(v["result"]["serverInfo"]["name"], "rqb");
        assert!(v["result"]["capabilities"]["tools"].is_object());
    }

    // ── dispatch: tools/list ──

    #[tokio::test]
    async fn dispatch_tools_list_returns_tools() {
        let req = make_req(2, "tools/list", json!({}));
        let s = dispatch(req).await;
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v["result"]["tools"].as_array().unwrap().len() >= 8);
    }

    // ── dispatch: notifications ──

    #[tokio::test]
    async fn notification_initialized_returns_no_response() {
        let req = make_notification("notifications/initialized");
        assert!(dispatch(req).await.is_empty());
    }

    #[tokio::test]
    async fn notification_arbitrary_returns_no_response() {
        let req = make_notification("notifications/progress");
        assert!(dispatch(req).await.is_empty());
    }

    // ── dispatch: unknown method ──

    #[tokio::test]
    async fn unknown_method_returns_32601() {
        let req = make_req(3, "unknown/method", json!({}));
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32601);
    }

    // ── dispatch: tools/call   missing params ──

    #[tokio::test]
    async fn exec_missing_spec_path_returns_32602() {
        let req = make_req(
            4,
            "tools/call",
            json!({"name": "rqb_exec", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn diagnose_missing_spec_path_returns_32602() {
        let req = make_req(
            42,
            "tools/call",
            json!({"name": "rqb_diagnose", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn diagnose_loads_env_file_for_spec_path() {
        let tmp = tempfile::tempdir().unwrap();
        let api_docs = tmp.path().join("api-docs");
        let apis = api_docs.join("apis/health");
        let shared = api_docs.join("_shared");
        std::fs::create_dir_all(&apis).unwrap();
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            shared.join("env.md"),
            "# Environments\n\n## dev\n\n```yaml\nbaseUrl: http://127.0.0.1:1\n```\n",
        )
        .unwrap();
        let spec_path = apis.join("get-health.md");
        std::fs::write(
            &spec_path,
            "---\nresource: health\nprotocol: http\nmethod: GET\npath: /health\nversion: 1\nenv: [dev]\n---\n# Get health\n\n## Request\n\n```http\nGET {{baseUrl}}/health\n```\n\n## Expected response\n\n```http\nHTTP/1.1 200 OK\n\n{\"ok\":true}\n```\n",
        )
        .unwrap();

        let req = make_req(
            44,
            "tools/call",
            json!({
                "name": "rqb_diagnose",
                "arguments": {
                    "spec_path": spec_path.to_str().unwrap(),
                    "env": "dev",
                    "timeout_ms": 100
                }
            }),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
        assert_eq!(
            v["result"]["structuredContent"]["error_type"],
            "NETWORK_ERROR"
        );
    }

    #[tokio::test]
    async fn flow_missing_pipeline_path_returns_32602() {
        let req = make_req(
            5,
            "tools/call",
            json!({"name": "rqb_flow", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    // ── dispatch: tools/call   path errors ──

    #[tokio::test]
    async fn exec_nonexistent_file_returns_32000() {
        let req = make_req(
            7,
            "tools/call",
            json!({"name": "rqb_exec", "arguments": {"spec_path": "/no/such/file.md"}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
    }

    // ── dispatch: tools/call   unknown tool ──

    #[tokio::test]
    async fn unknown_tool_returns_32601() {
        let req = make_req(
            9,
            "tools/call",
            json!({"name": "nonexistent_tool", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32601);
    }

    // ── rqb_author ──

    #[tokio::test]
    async fn author_missing_params_returns_32602() {
        let req = make_req(
            40,
            "tools/call",
            json!({"name": "rqb_author", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn author_invalid_content_returns_32000() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.md");
        let req = make_req(
            41,
            "tools/call",
            json!({
                "name": "rqb_author",
                "arguments": {
                    "spec_path": path.to_str().unwrap(),
                    "content": "not a valid rqb spec"
                }
            }),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn author_valid_spec_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("new-spec.md");
        let content = "---\nresource: ping\nprotocol: http\nmethod: GET\npath: /ping\ntags: [ping]\nversion: 1\nenv: [dev]\nauth: none\ntimeout: 5000\nretry:\n  attempts: 0\n  backoff: fixed\n---\n# Get ping\n\nA health check endpoint.\n\n## Request\n\n```http\nGET {{baseUrl}}/ping\n```\n\n## Expected response\n\n```http\nHTTP/1.1 200 OK\nContent-Type: application/json\n\n{\"pong\":true}\n```\n";
        let req = make_req(
            42,
            "tools/call",
            json!({
                "name": "rqb_author",
                "arguments": {
                    "spec_path": path.to_str().unwrap(),
                    "content": content
                }
            }),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
        assert!(path.exists(), "file should have been created");
    }

    #[tokio::test]
    async fn author_refuses_overwrite_without_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("existing.md");
        std::fs::write(&path, "existing content").unwrap();
        let content = "# GET /ping\n\nA health check endpoint.\n\n## Request\n\n```\nGET /ping HTTP/1.1\n```\n\n## Expected response\n\nHTTP/1.1 200 OK\n\n{\"pong\":true}\n";
        let req = make_req(
            43,
            "tools/call",
            json!({
                "name": "rqb_author",
                "arguments": {
                    "spec_path": path.to_str().unwrap(),
                    "content": content
                }
            }),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, "existing content");
    }

    // ── resources/list ──

    #[tokio::test]
    async fn resources_list_returns_array() {
        let req = make_req(50, "resources/list", json!({}));
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["error"].is_null(), "unexpected error: {v}");
        assert!(v["result"]["resources"].is_array());
    }

    // ── resources/read ──

    #[tokio::test]
    async fn resources_read_unknown_scheme_returns_32000() {
        let req = make_req(60, "resources/read", json!({"uri": "unknown://foo/bar.md"}));
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
    }

    #[tokio::test]
    async fn resources_read_missing_uri_returns_32602() {
        let req = make_req(61, "resources/read", json!({}));
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn resources_read_nonexistent_file_returns_32000() {
        let req = make_req(
            62,
            "resources/read",
            json!({"uri": "rqb://spec/no/such/file.md"}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
    }

    // ── rqb_vars ──

    #[tokio::test]
    async fn vars_missing_spec_path_returns_32602() {
        let req = make_req(
            70,
            "tools/call",
            json!({"name": "rqb_vars", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn vars_returns_variable_list() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("test.md");
        let content = "---\nresource: users\nprotocol: http\nmethod: GET\npath: /users/:id\nversion: 1\n---\n# Get user\n\nFetches a user.\n\n## Request\n\n```http\nGET {{baseUrl}}/users/:userId\nAuthorization: Bearer {{authToken}}\n```\n\n## Expected response\n\n```http\nHTTP/1.1 200 OK\n\n{\"id\":\"1\"}\n```\n";
        std::fs::write(&spec_path, content).unwrap();
        let req = make_req(
            71,
            "tools/call",
            json!({"name": "rqb_vars", "arguments": {"spec_path": spec_path.to_str().unwrap()}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        let data: Value = serde_json::from_str(text).unwrap();
        assert!(data["variables"].is_array());
        let names: Vec<&str> = data["variables"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"baseUrl"));
        assert!(names.contains(&"authToken"));
    }

    // ── rqb_search ──

    #[tokio::test]
    async fn search_returns_results_structure() {
        let req = make_req(
            80,
            "tools/call",
            json!({"name": "rqb_search", "arguments": {"method": "GET"}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        let data: Value = serde_json::from_str(text).unwrap();
        assert!(data["count"].is_number());
        assert!(data["results"].is_array());
    }

    // ── rqb_history ──

    #[tokio::test]
    async fn history_missing_spec_path_returns_32602() {
        let req = make_req(
            90,
            "tools/call",
            json!({"name": "rqb_history", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn history_returns_entries_and_trend() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("test.md");
        std::fs::write(&spec_path, "placeholder").unwrap();
        let req = make_req(
            91,
            "tools/call",
            json!({"name": "rqb_history", "arguments": {"spec_path": spec_path.to_str().unwrap()}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        let data: Value = serde_json::from_str(text).unwrap();
        assert!(data["entries"].is_array());
        assert!(data["trend"].is_string());
    }

    // ── rqb_session ──

    #[tokio::test]
    async fn session_missing_action_returns_32602() {
        let req = make_req(
            100,
            "tools/call",
            json!({"name": "rqb_session", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn session_get_returns_current_session() {
        let req = make_req(
            101,
            "tools/call",
            json!({"name": "rqb_session", "arguments": {"action": "get"}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
    }

    // ── rqb_exec_batch ──

    #[tokio::test]
    async fn exec_batch_missing_specs_returns_32602() {
        let req = make_req(
            110,
            "tools/call",
            json!({"name": "rqb_exec_batch", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn exec_batch_returns_summary() {
        let req = make_req(
            111,
            "tools/call",
            json!({"name": "rqb_exec_batch", "arguments": {"specs": ["/no/such/file.md"]}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        let data: Value = serde_json::from_str(text).unwrap();
        assert!(data["summary"].is_object());
        assert_eq!(data["summary"]["total"], 1);
        assert_eq!(data["summary"]["failed"], 1);
        assert!(data["results"].is_array());
    }
}
