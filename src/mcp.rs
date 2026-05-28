//! MCP (Model Context Protocol) server over stdio.
//!
//! Exposes three tools to any MCP-compatible AI agent:
//! - `trellis_exec`    — execute one endpoint spec
//! - `trellis_flow`    — execute a pipeline
//! - `trellis_author`  — create or update a spec file
//!
//! Transport: JSON-RPC 2.0 over stdio (NDJSON, one message per line).
//! Protocol version: 2024-11-05.
//!
//! To use, add to `~/.claude.json` or equivalent MCP config:
//! ```json
//! { "command": "trellis", "args": ["mcp"] }
//! ```

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{
    engine::{self, ExecOpts},
    parser::{parse_endpoint, parse_pipeline},
    pipeline::{self, PipelineOpts},
    resolver::{Context, SourceKind},
};

// ─── JSON-RPC 2.0 message types ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    /// Absent for notifications; present for requests that require a response.
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl McpResponse {
    fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
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

// ─── Tool schemas ─────────────────────────────────────────────────────────────

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "trellis_exec",
                "description": "Execute a Trellis endpoint spec and return the HTTP result.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "spec_path": {
                            "type": "string",
                            "description": "Path to the endpoint .md spec file."
                        },
                        "env": {
                            "type": "string",
                            "description": "Environment name (default: \"dev\")."
                        },
                        "vars": {
                            "type": "object",
                            "description": "Variable overrides as key/value pairs.",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["spec_path"]
                }
            },
            {
                "name": "trellis_flow",
                "description": "Execute a Trellis pipeline and return per-step results.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pipeline_path": {
                            "type": "string",
                            "description": "Path to the pipeline .md file."
                        },
                        "env": {
                            "type": "string",
                            "description": "Environment name (default: \"dev\")."
                        }
                    },
                    "required": ["pipeline_path"]
                }
            },
            {
                "name": "trellis_author",
                "description": "Create a new Trellis endpoint spec file. Validates the content before writing. Refuses to overwrite unless overwrite: true.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "spec_path": {
                            "type": "string",
                            "description": "Destination file path, e.g. \"api-docs/users/get-user-by-id.md\"."
                        },
                        "content": {
                            "type": "string",
                            "description": "Full markdown content of the endpoint spec, including YAML frontmatter."
                        },
                        "overwrite": {
                            "type": "boolean",
                            "description": "If true, replace an existing file (default: false)."
                        }
                    },
                    "required": ["spec_path", "content"]
                }
            }
        ]
    })
}

// ─── Tool handlers ────────────────────────────────────────────────────────────

/// Returns `Ok(content_value)` or `Err((rpc_code, message))`.
async fn handle_exec(args: &Value) -> Result<Value, (i32, String)> {
    let spec_path = args
        .get("spec_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: spec_path is required".to_string()))?;

    let env = args.get("env").and_then(|v| v.as_str()).unwrap_or("dev");

    let mut context = Context::default();
    if let Some(vars) = args.get("vars").and_then(|v| v.as_object()) {
        for (k, v) in vars {
            if let Some(val) = v.as_str() {
                context.insert(SourceKind::Cli, k, val);
            }
        }
    }

    let source =
        std::fs::read_to_string(spec_path).map_err(|e| (-32000, format!("{spec_path}: {e}")))?;
    let endpoint =
        parse_endpoint(&source, Path::new(spec_path)).map_err(|e| (-32000, e.to_string()))?;
    let execution = engine::execute(
        &endpoint,
        env,
        ExecOpts {
            context,
            timeout_ms: None,
            dry_run: false,
        },
    )
    .await
    .map_err(|e| (-32000, e.to_string()))?;

    let text = serde_json::to_string_pretty(&execution).map_err(|e| (-32000, e.to_string()))?;
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

async fn handle_flow(args: &Value) -> Result<Value, (i32, String)> {
    let pipeline_path = args
        .get("pipeline_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                -32602,
                "Invalid params: pipeline_path is required".to_string(),
            )
        })?;

    let env = args.get("env").and_then(|v| v.as_str()).unwrap_or("dev");

    let source = std::fs::read_to_string(pipeline_path)
        .map_err(|e| (-32000, format!("{pipeline_path}: {e}")))?;
    let pipeline =
        parse_pipeline(&source, Path::new(pipeline_path)).map_err(|e| (-32000, e.to_string()))?;

    let root = Path::new(pipeline_path)
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("api-docs"))
        .to_path_buf();

    let result = pipeline::run(
        &pipeline,
        env,
        PipelineOpts {
            root,
            exec: ExecOpts::default(),
        },
    )
    .await
    .map_err(|e| (-32000, e.to_string()))?;

    let text = serde_json::to_string_pretty(&result).map_err(|e| (-32000, e.to_string()))?;
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

fn handle_author(args: &Value) -> Result<Value, (i32, String)> {
    let spec_path = args
        .get("spec_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: spec_path is required".to_string()))?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: content is required".to_string()))?;
    let overwrite = args
        .get("overwrite")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let path = Path::new(spec_path);

    if path.exists() && !overwrite {
        return Err((
            -32000,
            format!("{spec_path}: file already exists. Pass overwrite: true to replace it."),
        ));
    }

    // Validate content before touching the filesystem.
    let ep = parse_endpoint(content, path)
        .map_err(|e| (-32000, format!("spec content is invalid: {e}")))?;

    // Create parent directories if needed.
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                (
                    -32000,
                    format!("failed to create {}: {e}", parent.display()),
                )
            })?;
        }
    }

    std::fs::write(path, content)
        .map_err(|e| (-32000, format!("{spec_path}: write failed: {e}")))?;

    let text = json!({
        "created": true,
        "file":    spec_path,
        "method":  ep.schema.method.as_str(),
        "path":    ep.schema.path,
        "title":   ep.title,
    })
    .to_string();
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

// ─── MCP Resources protocol ───────────────────────────────────────────────────

/// `resources/list` — enumerate all endpoint spec files as MCP resources.
/// URIs use the scheme `trellis://spec/<path-relative-to-api-docs>`.
fn handle_resources_list() -> Value {
    let root = Path::new("api-docs");
    let mut resources: Vec<Value> = Vec::new();
    if root.exists() {
        collect_resource_uris(root, root, &mut resources);
    }
    json!({ "resources": resources })
}

fn collect_resource_uris(root: &Path, dir: &Path, out: &mut Vec<Value>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();

    for p in paths {
        if p.is_dir() {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if !matches!(name.as_ref(), "_shared" | "flows" | "pipelines") {
                collect_resource_uris(root, &p, out);
            }
        } else if p.extension().is_some_and(|e| e == "md") {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if matches!(name.as_ref(), "README.md" | "trellis.md" | "env.md") {
                continue;
            }
            let rel = p.strip_prefix(root).unwrap_or(&p);
            let uri = format!("trellis://spec/{}", rel.display());
            // Best-effort description from parsed spec.
            let description = std::fs::read_to_string(&p)
                .ok()
                .and_then(|s| parse_endpoint(&s, &p).ok())
                .map(|ep| format!("{} {}", ep.schema.method.as_str(), ep.schema.path))
                .unwrap_or_default();
            out.push(json!({
                "uri":         uri,
                "name":        p.file_name().unwrap_or_default().to_string_lossy(),
                "mimeType":    "text/markdown",
                "description": description,
            }));
        }
    }
}

/// `resources/read` — return the markdown content of one spec by URI.
fn handle_resources_read(params: &Value) -> Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: uri is required".to_string()))?;

    let rel = uri
        .strip_prefix("trellis://spec/")
        .ok_or_else(|| (-32000, format!("unsupported URI scheme: {uri}")))?;

    let file_path = Path::new("api-docs").join(rel);
    if !file_path.exists() {
        return Err((
            -32000,
            format!("{}: resource not found", file_path.display()),
        ));
    }

    let text = std::fs::read_to_string(&file_path)
        .map_err(|e| (-32000, format!("{}: {e}", file_path.display())))?;

    Ok(json!({
        "contents": [{
            "uri":      uri,
            "mimeType": "text/markdown",
            "text":     text,
        }]
    }))
}

// ─── Dispatch ─────────────────────────────────────────────────────────────────

/// Dispatch one parsed JSON-RPC request and return the serialised response.
/// Returns an empty string for notifications (which must not be responded to).
async fn dispatch(req: McpRequest) -> String {
    // Notifications have no `id` and must not receive a response.
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
                    "name": "trellis",
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
                "trellis_exec" => match handle_exec(&args).await {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "trellis_flow" => match handle_flow(&args).await {
                    Ok(r) => McpResponse::ok(id, r),
                    Err((code, msg)) => McpResponse::err(id, code, msg),
                },
                "trellis_author" => match handle_author(&args) {
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
        // Catch-all for unknown methods (that are not notifications)
        other => McpResponse::err(id, -32601, format!("Method not found: \"{other}\"")),
    };

    serde_json::to_string(&resp).unwrap_or_default()
}

// ─── Main server loop ──────────────────────────────────────────────────────────

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
                let resp = McpResponse::err(Value::Null, -32700, format!("Parse error: {e}"));
                serde_json::to_string(&resp).unwrap_or_default()
            }
            Ok(req) => dispatch(req).await,
        };

        // Notifications return empty string — do not write anything back.
        if response.is_empty() {
            continue;
        }

        stdout.write_all(response.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
    fn tools_list_has_exactly_three_tools() {
        let list = tools_list_result();
        let tools = list["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
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
        assert!(names.contains(&"trellis_exec"));
        assert!(names.contains(&"trellis_flow"));
        assert!(names.contains(&"trellis_author"));
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
        assert_eq!(v["result"]["serverInfo"]["name"], "trellis");
        assert!(v["result"]["capabilities"]["tools"].is_object());
    }

    // ── dispatch: tools/list ──

    #[tokio::test]
    async fn dispatch_tools_list_returns_tools() {
        let req = make_req(2, "tools/list", json!({}));
        let s = dispatch(req).await;
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["result"]["tools"].as_array().unwrap().len(), 3);
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

    // ── dispatch: tools/call — missing params ──

    #[tokio::test]
    async fn exec_missing_spec_path_returns_32602() {
        let req = make_req(
            4,
            "tools/call",
            json!({"name": "trellis_exec", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn flow_missing_pipeline_path_returns_32602() {
        let req = make_req(
            5,
            "tools/call",
            json!({"name": "trellis_flow", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    // ── dispatch: tools/call — path errors ──

    #[tokio::test]
    async fn exec_nonexistent_file_returns_32000() {
        let req = make_req(
            7,
            "tools/call",
            json!({"name": "trellis_exec", "arguments": {"spec_path": "/no/such/file.md"}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
    }

    // ── dispatch: tools/call — unknown tool ──

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

    // ── trellis_author ──

    #[tokio::test]
    async fn author_missing_params_returns_32602() {
        let req = make_req(
            40,
            "tools/call",
            json!({"name": "trellis_author", "arguments": {}}),
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
                "name": "trellis_author",
                "arguments": {
                    "spec_path": path.to_str().unwrap(),
                    "content": "not a valid trellis spec"
                }
            }),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
        // File must NOT have been written
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
                "name": "trellis_author",
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
                "name": "trellis_author",
                "arguments": {
                    "spec_path": path.to_str().unwrap(),
                    "content": content
                }
            }),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
        // Original content must be preserved
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, "existing content");
    }

    // ── resources/list ──

    #[tokio::test]
    async fn resources_list_returns_array() {
        // Even with no api-docs dir present this must return a valid (possibly
        // empty) resources array rather than an error.
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
            json!({"uri": "trellis://spec/no/such/file.md"}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
    }
}
