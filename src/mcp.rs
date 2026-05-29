//! MCP (Model Context Protocol) server over stdio.
//!
//! Exposes tools to any MCP-compatible AI agent:
//! - `mad_exec`          execute one endpoint spec
//! - `mad_flow`          execute a pipeline
//! - `mad_author`        create or update a spec file
//! - `mad_vars`          show variable resolution for a spec
//! - `mad_search`        search specs by method/path/tag/text
//! - `mad_history`       execution history for a spec
//! - `mad_session`       get/set session context (env + vars)
//! - `mad_exec_batch`    execute multiple specs in one call
//!
//! Transport: JSON-RPC 2.0 over stdio (NDJSON, one message per line).
//! Protocol version: 2024-11-05.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{
    engine::{self, ExecOpts, EngineError},
    history::{self, HistoryEntry},
    parser::{parse_endpoint, parse_env_config, parse_pipeline},
    pipeline::{self, PipelineOpts},
    resolver::{Context, ResolveError, SourceKind},
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

// ─── Error taxonomy ───────────────────────────────────────────────────────────

fn classify_engine_error(err: &EngineError) -> (&'static str, Option<&'static str>) {
    match err {
        EngineError::UnsupportedProtocol { .. } => ("UNSUPPORTED_PROTOCOL", None),
        EngineError::Resolve { source, .. } => match source {
            ResolveError::MissingVariable { .. } => (
                "VAR_MISSING",
                Some("Define missing variables in _shared/env.md [<env>] or pass via vars: {...}"),
            ),
            _ => ("VALIDATION_ERROR", None),
        },
        EngineError::Network { .. } => (
            "NETWORK_ERROR",
            Some("Check baseUrl in env.md and ensure the server is running"),
        ),
        EngineError::InvalidRequest { .. } => ("VALIDATION_ERROR", None),
        EngineError::InvalidExpected { .. } => ("VALIDATION_ERROR", None),
        EngineError::Http { .. } => ("VALIDATION_ERROR", None),
    }
}

fn hint_for_error_type(error_type: &str) -> Option<&'static str> {
    match error_type {
        "VAR_MISSING" => Some(
            "Define missing variables in _shared/env.md [<env>] or pass via vars: {...}",
        ),
        "AUTH_FAILED" => Some("Check bearer token or credentials in _shared/env.md"),
        "NETWORK_ERROR" => Some("Check baseUrl in env.md and ensure the server is running"),
        "CONTRACT_MISMATCH" => Some(
            "Update ## Expected response in the spec to match actual, or fix the API",
        ),
        "SPEC_PARSE_ERROR" => Some(
            "Fix YAML frontmatter or markdown section structure in the spec file",
        ),
        _ => None,
    }
}

/// Return an ISO-8601 UTC timestamp string for the current moment.
fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Minimal ISO-8601 UTC   good enough for history logs.
    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn epoch_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let total_min = secs / 60;
    let mi = total_min % 60;
    let total_h = total_min / 60;
    let h = total_h % 24;
    let total_days = total_h / 24;
    // Gregorian calendar approximation.
    let (y, mo, d) = days_to_ymd(total_days);
    (y, mo, d, h, mi, s)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    let mut rem = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if rem < days_in_year {
            break;
        }
        rem -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let months = [
        31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut mo = 1u64;
    for days_in_month in months {
        if rem < days_in_month {
            break;
        }
        rem -= days_in_month;
        mo += 1;
    }
    (y, mo, rem + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

// ─── Session helpers ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Session {
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    vars: std::collections::BTreeMap<String, String>,
}

fn session_path() -> std::path::PathBuf {
    std::env::temp_dir().join("mad-session.json")
}

fn read_session() -> Session {
    std::fs::read_to_string(session_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_session(session: &Session) {
    let _ = serde_json::to_string_pretty(session)
        .ok()
        .and_then(|s| std::fs::write(session_path(), s).ok());
}

/// Resolve the effective env, preferring explicit arg → session → default "dev".
fn resolve_env<'a>(args: &'a Value, session: &'a Session) -> &'a str {
    args.get("env")
        .and_then(|v| v.as_str())
        .or(session.env.as_deref())
        .unwrap_or("dev")
}

/// Build a context, merging session vars (lower priority) then explicit vars.
fn build_context(args: &Value, session: &Session) -> Context {
    let mut context = Context::default();
    // Session vars are low-priority.
    for (k, v) in &session.vars {
        context.insert(SourceKind::Env, k, v);
    }
    // Explicit vars override session vars.
    if let Some(vars) = args.get("vars").and_then(|v| v.as_object()) {
        for (k, v) in vars {
            if let Some(val) = v.as_str() {
                context.insert(SourceKind::Cli, k, val);
            }
        }
    }
    context
}

// ─── Collection root detection ────────────────────────────────────────────────

/// Find the collection root by walking up from a spec path looking for `api-docs` or `apis` dir.
fn collection_root_for(spec_path: &str) -> std::path::PathBuf {
    let p = Path::new(spec_path);
    // Walk up until we find a directory containing `_shared/` or until we run out of ancestors.
    if let Some(parent) = p.parent() {
        let mut candidate = parent.to_path_buf();
        loop {
            if candidate.join("_shared").exists() {
                return candidate;
            }
            // Also accept if the dir is named `api-docs` or `apis`.
            if let Some(name) = candidate.file_name() {
                let n = name.to_string_lossy();
                if n == "api-docs" || n == "apis" {
                    return candidate;
                }
            }
            match candidate.parent() {
                Some(p) => candidate = p.to_path_buf(),
                None => break,
            }
        }
        // Fallback: two levels up from spec file.
        if let Some(ancestor) = Path::new(spec_path).ancestors().nth(2) {
            return ancestor.to_path_buf();
        }
    }
    Path::new(".").to_path_buf()
}

/// Compute a path relative to a root for use in history.
fn spec_rel(spec_path: &str, root: &Path) -> String {
    Path::new(spec_path)
        .strip_prefix(root)
        .unwrap_or_else(|_| Path::new(spec_path))
        .to_string_lossy()
        .to_string()
}

// ─── Tool schemas ─────────────────────────────────────────────────────────────

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "mad_exec",
                "description": "Execute a MarkApiDown endpoint spec and return the HTTP result.",
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
                        },
                        "verbose": {
                            "type": "boolean",
                            "description": "When true, include full request and response objects (default: false)."
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "When true, resolve variables and return request without sending (default: false)."
                        },
                        "infer_expected": {
                            "type": "boolean",
                            "description": "When true, include inferred_expected block formatted for pasting into the spec (default: false)."
                        }
                    },
                    "required": ["spec_path"]
                }
            },
            {
                "name": "mad_flow",
                "description": "Execute a MarkApiDown pipeline and return per-step results.",
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
                        },
                        "verbose": {
                            "type": "boolean",
                            "description": "When true, include full execution objects per step (default: false)."
                        }
                    },
                    "required": ["pipeline_path"]
                }
            },
            {
                "name": "mad_author",
                "description": "Create a new MarkApiDown endpoint spec file. Validates the content before writing. Refuses to overwrite unless overwrite: true.\n\nRequired frontmatter fields: resource, protocol (http), method (GET/POST/PUT/PATCH/DELETE), path (/res/:param), version (1).\nRequired sections in order: ## Request (```http block), ## Expected response (```http block).\nOptional sections: ## Error responses (reference only, not executed), ## Assertions (structured rules), ## Tests (```agent-task block), ## Notes.\nVariables use {{name}} syntax in http blocks. Path params use :param in the path field and URL.\nSee api-docs/_shared/env.md for available environments and variable names.",
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
            },
            {
                "name": "mad_vars",
                "description": "Show which variables a spec requires and which are resolved in the current env.",
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
                        }
                    },
                    "required": ["spec_path"]
                }
            },
            {
                "name": "mad_search",
                "description": "Search endpoint specs by method, path, tag, or text.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "q": {
                            "type": "string",
                            "description": "Text to search in title or description."
                        },
                        "method": {
                            "type": "string",
                            "description": "HTTP method filter, e.g. \"GET\"."
                        },
                        "path": {
                            "type": "string",
                            "description": "URL path substring to filter on."
                        },
                        "tag": {
                            "type": "string",
                            "description": "Tag to filter on."
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "mad_history",
                "description": "Return recent execution history for a spec.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "spec_path": {
                            "type": "string",
                            "description": "Path to the endpoint .md spec file."
                        },
                        "last": {
                            "type": "integer",
                            "description": "Number of recent entries to return (default: 10)."
                        }
                    },
                    "required": ["spec_path"]
                }
            },
            {
                "name": "mad_session",
                "description": "Get or set the session context (env + vars) used as defaults by exec/flow.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["get", "set"],
                            "description": "\"get\" returns the current session; \"set\" writes env and/or vars."
                        },
                        "env": {
                            "type": "string",
                            "description": "Environment name to store in session."
                        },
                        "vars": {
                            "type": "object",
                            "description": "Variables to store in session.",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["action"]
                }
            },
            {
                "name": "mad_exec_batch",
                "description": "Execute multiple endpoint specs sequentially and return a summary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "specs": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of spec file paths to execute."
                        },
                        "env": {
                            "type": "string",
                            "description": "Environment name (default: \"dev\")."
                        },
                        "vars": {
                            "type": "object",
                            "description": "Variable overrides.",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["specs"]
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

    let session = read_session();
    let env = resolve_env(args, &session);
    let verbose = args.get("verbose").and_then(|v| v.as_bool()).unwrap_or(false);
    let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
    let infer_expected = args
        .get("infer_expected")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let context = build_context(args, &session);

    // Parse spec   return structured error on failure, not RPC error.
    let source = match std::fs::read_to_string(spec_path) {
        Ok(s) => s,
        Err(e) => {
            return Err((-32000, format!("{spec_path}: {e}")));
        }
    };

    let endpoint = match parse_endpoint(&source, Path::new(spec_path)) {
        Ok(ep) => ep,
        Err(e) => {
            let text = serde_json::to_string_pretty(&json!({
                "passed": false,
                "error_type": "SPEC_PARSE_ERROR",
                "hint": hint_for_error_type("SPEC_PARSE_ERROR"),
                "message": e.to_string(),
            }))
            .unwrap_or_default();
            return Ok(json!({ "content": [{ "type": "text", "text": text }] }));
        }
    };

    let execution_result = engine::execute(
        &endpoint,
        env,
        ExecOpts {
            context,
            timeout_ms: None,
            dry_run,
        },
    )
    .await;

    // Determine collection root for history.
    let root = collection_root_for(spec_path);
    let rel = spec_rel(spec_path, &root);

    let output = match execution_result {
        Ok(execution) => {
            // Check for auth failure.
            let status = execution.response.as_ref().map(|r| r.status);
            let error_type: Option<&str> = if matches!(status, Some(401) | Some(403)) {
                Some("AUTH_FAILED")
            } else if !execution.diff.passed {
                Some("CONTRACT_MISMATCH")
            } else {
                None
            };

            let passed = error_type.is_none();
            let duration_ms = execution.duration_ms;

            // Write history.
            let hist_entry = HistoryEntry {
                timestamp: now_iso8601(),
                passed,
                status,
                duration_ms,
                error_type: error_type.map(str::to_string),
            };
            history::write_entry(&root, &rel, hist_entry);

            // Build compact or verbose output.
            let mut result = if verbose {
                json!({
                    "passed": passed,
                    "status": status,
                    "duration_ms": duration_ms,
                    "error_type": error_type,
                    "diff": execution.diff,
                    "request": execution.request,
                    "response": execution.response,
                    "assertion_results": execution.assertion_results,
                })
            } else {
                json!({
                    "passed": passed,
                    "status": status,
                    "duration_ms": duration_ms,
                    "error_type": error_type,
                    "diff": execution.diff,
                    "assertion_results": execution.assertion_results,
                })
            };

            // Add hint when there's an error.
            if let Some(et) = error_type {
                result["hint"] = json!(hint_for_error_type(et));
                // Tier 3: fix hints for CONTRACT_MISMATCH.
                if et == "CONTRACT_MISMATCH" {
                    let mut hints: Vec<String> = Vec::new();
                    if execution.diff.status.is_some() {
                        if let Some(s) = status {
                            hints.push(format!(
                                "Update ## Expected response: change status line to HTTP/1.1 {s} <reason>"
                            ));
                        }
                    }
                    if !execution.diff.headers.is_empty() {
                        hints.push("Add missing header to ## Expected response or remove it from the assertion".to_string());
                    }
                    if execution.diff.body.is_some() {
                        hints.push("Run with infer_expected: true to get the actual response as an Expected Response block".to_string());
                    }
                    if !hints.is_empty() {
                        result["hints"] = json!(hints);
                    }
                }
            }

            // Tier 3: infer_expected.
            if infer_expected {
                if let Some(resp) = &execution.response {
                    let mut inferred = format!(
                        "HTTP/1.1 {} {}\n",
                        resp.status,
                        http_reason(resp.status)
                    );
                    if let Some(ct) = resp.headers.get("content-type") {
                        inferred.push_str(&format!("Content-Type: {ct}\n"));
                    }
                    inferred.push('\n');
                    inferred.push_str(&resp.body);
                    result["inferred_expected"] = json!(inferred);
                }
            }

            result
        }
        Err(engine_error) => {
            let (error_type, hint) = classify_engine_error(&engine_error);
            let hist_entry = HistoryEntry {
                timestamp: now_iso8601(),
                passed: false,
                status: None,
                duration_ms: 0,
                error_type: Some(error_type.to_string()),
            };
            history::write_entry(&root, &rel, hist_entry);
            json!({
                "passed": false,
                "status": null,
                "duration_ms": 0,
                "error_type": error_type,
                "hint": hint,
                "message": engine_error.to_string(),
                "diff": { "passed": false, "status": null, "headers": [], "body": null },
            })
        }
    };

    let text = serde_json::to_string_pretty(&output).map_err(|e| (-32000, e.to_string()))?;
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

    let session = read_session();
    let env = resolve_env(args, &session);
    let verbose = args.get("verbose").and_then(|v| v.as_bool()).unwrap_or(false);

    let source = match std::fs::read_to_string(pipeline_path) {
        Ok(s) => s,
        Err(e) => {
            let text = serde_json::to_string_pretty(&json!({
                "passed": false,
                "error_type": "SPEC_PARSE_ERROR",
                "hint": hint_for_error_type("SPEC_PARSE_ERROR"),
                "message": format!("{pipeline_path}: {e}"),
            }))
            .unwrap_or_default();
            return Ok(json!({ "content": [{ "type": "text", "text": text }] }));
        }
    };

    let pipeline = match parse_pipeline(&source, Path::new(pipeline_path)) {
        Ok(p) => p,
        Err(e) => {
            let text = serde_json::to_string_pretty(&json!({
                "passed": false,
                "error_type": "SPEC_PARSE_ERROR",
                "hint": hint_for_error_type("SPEC_PARSE_ERROR"),
                "message": e.to_string(),
            }))
            .unwrap_or_default();
            return Ok(json!({ "content": [{ "type": "text", "text": text }] }));
        }
    };

    let root = Path::new(pipeline_path)
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("api-docs"))
        .to_path_buf();

    let exec_opts = build_context(args, &session);

    let result = match pipeline::run(
        &pipeline,
        env,
        PipelineOpts {
            root: root.clone(),
            exec: ExecOpts {
                context: exec_opts,
                timeout_ms: None,
                dry_run: false,
            },
        },
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            let text = serde_json::to_string_pretty(&json!({
                "passed": false,
                "error_type": "NETWORK_ERROR",
                "message": e.to_string(),
            }))
            .unwrap_or_default();
            return Ok(json!({ "content": [{ "type": "text", "text": text }] }));
        }
    };

    let output = if verbose {
        serde_json::to_value(&result).unwrap_or(Value::Null)
    } else {
        // Compact: per-step summary + captures + passed.
        let steps: Vec<Value> = result
            .steps
            .iter()
            .map(|step| {
                let status = step.execution.as_ref().and_then(|e| e.response.as_ref()).map(|r| r.status);
                let passed = step.execution.as_ref().map(|e| e.diff.passed).unwrap_or(false) && step.error.is_none();
                let error_type: Option<&str> = if step.error.is_some() {
                    Some("NETWORK_ERROR")
                } else if matches!(status, Some(401) | Some(403)) {
                    Some("AUTH_FAILED")
                } else if !passed {
                    Some("CONTRACT_MISMATCH")
                } else {
                    None
                };
                json!({
                    "name": step.name,
                    "endpoint": step.endpoint,
                    "passed": passed,
                    "status": status,
                    "error_type": error_type,
                })
            })
            .collect();
        json!({
            "passed": result.passed,
            "captures": result.captures,
            "steps": steps,
        })
    };

    let text = serde_json::to_string_pretty(&output).map_err(|e| (-32000, e.to_string()))?;
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

/// Show which variables a spec requires and which are resolved.
async fn handle_vars(args: &Value) -> Result<Value, (i32, String)> {
    let spec_path = args
        .get("spec_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: spec_path is required".to_string()))?;

    let session = read_session();
    let env = resolve_env(args, &session);

    let source =
        std::fs::read_to_string(spec_path).map_err(|e| (-32000, format!("{spec_path}: {e}")))?;
    let endpoint = parse_endpoint(&source, Path::new(spec_path))
        .map_err(|e| (-32000, e.to_string()))?;

    // Extract template vars {{varName}} from request block.
    let template_re =
        regex::Regex::new(r"\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}").expect("valid var regex");
    // Extract path params :paramName from first line of request block.
    let path_re =
        regex::Regex::new(r":([A-Za-z_][A-Za-z0-9_]*)").expect("valid path param regex");

    let mut vars: std::collections::BTreeMap<String, &'static str> = std::collections::BTreeMap::new();

    for caps in template_re.captures_iter(&endpoint.request) {
        vars.insert(caps[1].to_string(), "template");
    }
    // Path params from first line only.
    if let Some(first_line) = endpoint.request.lines().next() {
        for caps in path_re.captures_iter(first_line) {
            vars.entry(caps[1].to_string()).or_insert("path_param");
        }
    }

    // Build a context from env.md + .env.local + OS env vars.
    let mut context = Context::default();

    // Load env.md if present.
    let root = collection_root_for(spec_path);
    let env_md = root.join("_shared").join("env.md");
    if let Ok(env_source) = std::fs::read_to_string(&env_md) {
        if let Ok(env_config) = parse_env_config(&env_source, &env_md) {
            if let Some(values) = env_config.envs.get(env) {
                for (k, v) in values {
                    context.insert(SourceKind::Env, k, v);
                }
            }
        }
    }

    // Load .env.local (best-effort).
    let dot_env = root.join(".env.local");
    if let Ok(dot_source) = std::fs::read_to_string(&dot_env) {
        for line in dot_source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                context.insert(SourceKind::DotEnvLocal, k.trim(), v.trim());
            }
        }
    }

    // Load MAD_* OS env vars (best-effort).
    for (k, v) in std::env::vars() {
        if let Some(stripped) = k.strip_prefix("MAD_") {
            context.insert(SourceKind::OsEnv, stripped, v);
        }
    }

    // Build variable list.
    let mut all_resolved = true;
    let variables: Vec<Value> = vars
        .iter()
        .map(|(name, kind)| {
            let resolved = context.get(name).is_some();
            if !resolved {
                all_resolved = false;
            }
            let source_label = if resolved {
                Some(if context.get(name).is_some() { "env.md" } else { "vars" })
            } else {
                None
            };
            let hint = if !resolved {
                Some(if *kind == "path_param" {
                    format!("Pass as vars: {{\"{name}\": \"...\"}}")
                } else {
                    format!(
                        "Set in _shared/env.md [{env}] or pass as vars: {{\"{name}\": \"...\"}}"
                    )
                })
            } else {
                None
            };
            json!({
                "name": name,
                "kind": kind,
                "resolved": resolved,
                "source": source_label,
                "hint": hint,
            })
        })
        .collect();

    let result = json!({
        "spec": spec_path,
        "env": env,
        "ready": all_resolved,
        "variables": variables,
    });

    let text = serde_json::to_string_pretty(&result).map_err(|e| (-32000, e.to_string()))?;
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

/// Search specs in the collection.
async fn handle_search(args: &Value) -> Result<Value, (i32, String)> {
    let q = args.get("q").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
    let method_filter = args
        .get("method")
        .and_then(|v| v.as_str())
        .map(|s| s.to_uppercase());
    let path_filter = args.get("path").and_then(|v| v.as_str());
    let tag_filter = args.get("tag").and_then(|v| v.as_str());

    // Find the collection root by looking for `api-docs` or `apis` in cwd.
    let search_root = {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let candidates = ["api-docs", "apis"];
        candidates
            .iter()
            .map(|c| cwd.join(c))
            .find(|p| p.exists())
            .unwrap_or(cwd)
    };

    let mut results: Vec<Value> = Vec::new();

    if search_root.exists() {
        walk_specs(&search_root, &search_root, |file_path, rel_path| {
            let Ok(source) = std::fs::read_to_string(file_path) else {
                return;
            };
            let Ok(ep) = parse_endpoint(&source, file_path) else {
                return;
            };

            // Apply filters.
            if let Some(ref m) = method_filter {
                if ep.schema.method.as_str() != m {
                    return;
                }
            }
            if let Some(pf) = path_filter {
                if !ep.schema.path.contains(pf) {
                    return;
                }
            }
            if let Some(tf) = tag_filter {
                if !ep.schema.tags.iter().any(|t| t == tf) {
                    return;
                }
            }
            if !q.is_empty() {
                let haystack = format!(
                    "{} {}",
                    ep.title.to_lowercase(),
                    ep.description.to_lowercase()
                );
                if !haystack.contains(&q) {
                    return;
                }
            }

            results.push(json!({
                "file": rel_path,
                "method": ep.schema.method.as_str(),
                "path": ep.schema.path,
                "title": ep.title,
                "tags": ep.schema.tags,
            }));
        });
    }

    let output = json!({
        "count": results.len(),
        "results": results,
    });

    let text = serde_json::to_string_pretty(&output).map_err(|e| (-32000, e.to_string()))?;
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

fn walk_specs(
    root: &Path,
    dir: &Path,
    mut cb: impl FnMut(&Path, String),
) {
    walk_specs_inner(root, dir, &mut cb);
}

fn walk_specs_inner(root: &Path, dir: &Path, cb: &mut impl FnMut(&Path, String)) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if !matches!(name.as_ref(), "_shared" | "flows" | "pipelines") {
                walk_specs_inner(root, &p, cb);
            }
        } else if p.extension().is_some_and(|e| e == "md") {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if matches!(name.as_ref(), "README.md" | "mad.md" | "env.md") {
                continue;
            }
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            cb(&p, rel);
        }
    }
}

/// Return recent execution history for a spec.
async fn handle_history(args: &Value) -> Result<Value, (i32, String)> {
    let spec_path = args
        .get("spec_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: spec_path is required".to_string()))?;

    let last = args
        .get("last")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;

    let root = collection_root_for(spec_path);
    let rel = spec_rel(spec_path, &root);
    let mut entries = history::read_history(&root, &rel);
    // Return the most recent `last` entries.
    if entries.len() > last {
        let drop = entries.len() - last;
        entries.drain(..drop);
    }

    let trend = history::compute_trend(&entries);

    let output = json!({
        "spec": spec_path,
        "entries": entries,
        "trend": trend,
    });

    let text = serde_json::to_string_pretty(&output).map_err(|e| (-32000, e.to_string()))?;
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

/// Get or set the session context.
fn handle_session(args: &Value) -> Result<Value, (i32, String)> {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: action is required".to_string()))?;

    match action {
        "get" => {
            let session = read_session();
            let text = serde_json::to_string_pretty(&json!({
                "env": session.env,
                "vars": session.vars,
            }))
            .map_err(|e| (-32000, e.to_string()))?;
            Ok(json!({ "content": [{ "type": "text", "text": text }] }))
        }
        "set" => {
            let mut session = read_session();
            if let Some(env) = args.get("env").and_then(|v| v.as_str()) {
                session.env = Some(env.to_string());
            }
            if let Some(vars) = args.get("vars").and_then(|v| v.as_object()) {
                for (k, v) in vars {
                    if let Some(val) = v.as_str() {
                        session.vars.insert(k.clone(), val.to_string());
                    }
                }
            }
            write_session(&session);
            let text = serde_json::to_string_pretty(&json!({
                "saved": true,
                "env": session.env,
                "vars": session.vars,
            }))
            .map_err(|e| (-32000, e.to_string()))?;
            Ok(json!({ "content": [{ "type": "text", "text": text }] }))
        }
        other => Err((-32602, format!("Invalid params: unknown action \"{other}\""))),
    }
}

/// Execute multiple specs sequentially.
async fn handle_exec_batch(args: &Value) -> Result<Value, (i32, String)> {
    let specs = args
        .get("specs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| (-32602, "Invalid params: specs array is required".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect::<Vec<_>>();

    let session = read_session();
    let env = resolve_env(args, &session);
    let context = build_context(args, &session);

    let started = std::time::Instant::now();
    let mut results: Vec<Value> = Vec::new();
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;

    for spec_path in &specs {
        let source = match std::fs::read_to_string(spec_path) {
            Ok(s) => s,
            Err(e) => {
                total_failed += 1;
                results.push(json!({
                    "spec": spec_path,
                    "passed": false,
                    "status": null,
                    "duration_ms": 0,
                    "error_type": "SPEC_PARSE_ERROR",
                    "hint": hint_for_error_type("SPEC_PARSE_ERROR"),
                    "message": e.to_string(),
                }));
                continue;
            }
        };

        let endpoint = match parse_endpoint(&source, Path::new(spec_path.as_str())) {
            Ok(ep) => ep,
            Err(e) => {
                total_failed += 1;
                results.push(json!({
                    "spec": spec_path,
                    "passed": false,
                    "status": null,
                    "duration_ms": 0,
                    "error_type": "SPEC_PARSE_ERROR",
                    "hint": hint_for_error_type("SPEC_PARSE_ERROR"),
                    "message": e.to_string(),
                }));
                continue;
            }
        };

        let step_ctx = context.clone();
        let exec_result = engine::execute(
            &endpoint,
            env,
            ExecOpts {
                context: step_ctx,
                timeout_ms: None,
                dry_run: false,
            },
        )
        .await;

        let root = collection_root_for(spec_path);
        let rel = spec_rel(spec_path, &root);

        match exec_result {
            Ok(execution) => {
                let status = execution.response.as_ref().map(|r| r.status);
                let error_type: Option<&str> = if matches!(status, Some(401) | Some(403)) {
                    Some("AUTH_FAILED")
                } else if !execution.diff.passed {
                    Some("CONTRACT_MISMATCH")
                } else {
                    None
                };
                let passed = error_type.is_none();
                if passed {
                    total_passed += 1;
                } else {
                    total_failed += 1;
                }
                history::write_entry(
                    &root,
                    &rel,
                    HistoryEntry {
                        timestamp: now_iso8601(),
                        passed,
                        status,
                        duration_ms: execution.duration_ms,
                        error_type: error_type.map(str::to_string),
                    },
                );
                let mut entry = json!({
                    "spec": spec_path,
                    "passed": passed,
                    "status": status,
                    "duration_ms": execution.duration_ms,
                    "error_type": error_type,
                });
                if let Some(et) = error_type {
                    entry["hint"] = json!(hint_for_error_type(et));
                }
                results.push(entry);
            }
            Err(e) => {
                let (error_type, hint) = classify_engine_error(&e);
                total_failed += 1;
                history::write_entry(
                    &root,
                    &rel,
                    HistoryEntry {
                        timestamp: now_iso8601(),
                        passed: false,
                        status: None,
                        duration_ms: 0,
                        error_type: Some(error_type.to_string()),
                    },
                );
                results.push(json!({
                    "spec": spec_path,
                    "passed": false,
                    "status": null,
                    "duration_ms": 0,
                    "error_type": error_type,
                    "hint": hint,
                    "message": e.to_string(),
                }));
            }
        }
    }

    let total_duration_ms = started.elapsed().as_millis();
    let output = json!({
        "summary": {
            "total": specs.len(),
            "passed": total_passed,
            "failed": total_failed,
            "duration_ms": total_duration_ms,
        },
        "results": results,
    });

    let text = serde_json::to_string_pretty(&output).map_err(|e| (-32000, e.to_string()))?;
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

/// Common HTTP reason phrases.
fn http_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}

// ─── MCP Resources protocol ───────────────────────────────────────────────────

/// `resources/list`   enumerate all endpoint spec files as MCP resources.
/// URIs use the scheme `mad://spec/<path-relative-to-api-docs>`.
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
            if matches!(name.as_ref(), "README.md" | "mad.md" | "env.md") {
                continue;
            }
            let rel = p.strip_prefix(root).unwrap_or(&p);
            let uri = format!("mad://spec/{}", rel.display());
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

/// `resources/read`   return the markdown content of one spec by URI.
fn handle_resources_read(params: &Value) -> Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: uri is required".to_string()))?;

    let rel = uri
        .strip_prefix("mad://spec/")
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

        // Notifications return empty string   do not write anything back.
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
    fn tools_list_has_at_least_eight_tools() {
        let list = tools_list_result();
        let tools = list["tools"].as_array().unwrap();
        assert!(tools.len() >= 8, "expected at least 8 tools, got {}", tools.len());
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
        assert!(names.contains(&"mad_exec"));
        assert!(names.contains(&"mad_flow"));
        assert!(names.contains(&"mad_author"));
        assert!(names.contains(&"mad_vars"));
        assert!(names.contains(&"mad_search"));
        assert!(names.contains(&"mad_history"));
        assert!(names.contains(&"mad_session"));
        assert!(names.contains(&"mad_exec_batch"));
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
        assert_eq!(v["result"]["serverInfo"]["name"], "mad");
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
            json!({"name": "mad_exec", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn flow_missing_pipeline_path_returns_32602() {
        let req = make_req(
            5,
            "tools/call",
            json!({"name": "mad_flow", "arguments": {}}),
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
            json!({"name": "mad_exec", "arguments": {"spec_path": "/no/such/file.md"}}),
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

    // ── mad_author ──

    #[tokio::test]
    async fn author_missing_params_returns_32602() {
        let req = make_req(
            40,
            "tools/call",
            json!({"name": "mad_author", "arguments": {}}),
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
                "name": "mad_author",
                "arguments": {
                    "spec_path": path.to_str().unwrap(),
                    "content": "not a valid mad spec"
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
                "name": "mad_author",
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
                "name": "mad_author",
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
            json!({"uri": "mad://spec/no/such/file.md"}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32000);
    }

    // ── mad_vars ──

    #[tokio::test]
    async fn vars_missing_spec_path_returns_32602() {
        let req = make_req(
            70,
            "tools/call",
            json!({"name": "mad_vars", "arguments": {}}),
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
            json!({"name": "mad_vars", "arguments": {"spec_path": spec_path.to_str().unwrap()}}),
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

    // ── mad_search ──

    #[tokio::test]
    async fn search_returns_results_structure() {
        let req = make_req(
            80,
            "tools/call",
            json!({"name": "mad_search", "arguments": {"method": "GET"}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        let data: Value = serde_json::from_str(text).unwrap();
        assert!(data["count"].is_number());
        assert!(data["results"].is_array());
    }

    // ── mad_history ──

    #[tokio::test]
    async fn history_missing_spec_path_returns_32602() {
        let req = make_req(
            90,
            "tools/call",
            json!({"name": "mad_history", "arguments": {}}),
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
            json!({"name": "mad_history", "arguments": {"spec_path": spec_path.to_str().unwrap()}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        let data: Value = serde_json::from_str(text).unwrap();
        assert!(data["entries"].is_array());
        assert!(data["trend"].is_string());
    }

    // ── mad_session ──

    #[tokio::test]
    async fn session_missing_action_returns_32602() {
        let req = make_req(
            100,
            "tools/call",
            json!({"name": "mad_session", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn session_get_returns_current_session() {
        let req = make_req(
            101,
            "tools/call",
            json!({"name": "mad_session", "arguments": {"action": "get"}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert!(v["result"].is_object(), "expected result, got: {v}");
    }

    // ── mad_exec_batch ──

    #[tokio::test]
    async fn exec_batch_missing_specs_returns_32602() {
        let req = make_req(
            110,
            "tools/call",
            json!({"name": "mad_exec_batch", "arguments": {}}),
        );
        let v: Value = serde_json::from_str(&dispatch(req).await).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn exec_batch_returns_summary() {
        // Run with a nonexistent spec to exercise the error path.
        let req = make_req(
            111,
            "tools/call",
            json!({"name": "mad_exec_batch", "arguments": {"specs": ["/no/such/file.md"]}}),
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
