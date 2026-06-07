//! Tool schemas and handler implementations.

use std::path::Path;

use serde_json::{json, Value};

use crate::{
    engine::{self, ExecOpts},
    history::{self, HistoryEntry},
    parser::{parse_endpoint, parse_env_config, parse_pipeline},
    pipeline::{self, PipelineOpts},
    resolver::SourceKind,
};

use super::{
    session::{build_context, read_session, resolve_env},
    util::{
        classify_engine_error, collection_root_for, hint_for_error_type, http_reason, now_iso8601,
        spec_rel, walk_specs,
    },
};

pub(super) fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "rqb_exec",
                "description": "Execute a Reqbook endpoint spec and return the HTTP result.",
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
                        },
                        "strict_assertions": {
                            "type": "boolean",
                            "description": "When true, failing structured assertions make the tool result fail (default: false)."
                        }
                    },
                    "required": ["spec_path"]
                }
            },
            {
                "name": "rqb_flow",
                "description": "Execute a Reqbook pipeline and return per-step results.",
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
                "name": "rqb_author",
                "description": "Create a new Reqbook endpoint spec file. Validates the content before writing. Refuses to overwrite unless overwrite: true.\n\nRequired frontmatter fields: resource, protocol (http), method (GET/POST/PUT/PATCH/DELETE), path (/res/:param), version (1).\nRequired sections in order: ## Request (```http block), ## Expected response (```http block).\nOptional sections: ## Error responses (reference only, not executed), ## Assertions (structured rules), ## Tests (```agent-task block), ## Notes.\nVariables use {{name}} syntax in http blocks. Path params use :param in the path field and URL.\nSee api-docs/_shared/env.md for available environments and variable names.",
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
                "name": "rqb_vars",
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
                "name": "rqb_search",
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
                "name": "rqb_context",
                "description": "Return bounded executable API context for an endpoint, flow, or changed specs. Defaults to surgical mode to minimize tokens.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Endpoint/flow id or file path, e.g. users.create."
                        },
                        "changed_from": {
                            "type": "string",
                            "description": "Git ref used to summarize changed specs only."
                        },
                        "root": {
                            "type": "string",
                            "description": "api-docs root directory (default: api-docs)."
                        },
                        "token_budget": {
                            "type": "integer",
                            "description": "Approximate output token budget (default: 800)."
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["surgical", "compact", "schema"],
                            "description": "Output mode. surgical is contract-only; compact is human-readable; schema is JSON contract summary (default: surgical)."
                        },
                        "intent": {
                            "type": "string",
                            "description": "Agent task intent, e.g. implement, debug, test, review, or document."
                        },
                        "verbose": {
                            "type": "boolean",
                            "description": "Include full request and expected response blocks (default: false)."
                        },
                        "env": {
                            "type": "string",
                            "description": "Environment used in suggested next commands (default: dev)."
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "rqb_history",
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
                "name": "rqb_session",
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
                "name": "rqb_exec_batch",
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

pub(super) async fn handle_exec(args: &Value) -> Result<Value, (i32, String)> {
    let spec_path = args
        .get("spec_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: spec_path is required".to_string()))?;

    let session = read_session();
    let env = resolve_env(args, &session);
    let verbose = args
        .get("verbose")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let infer_expected = args
        .get("infer_expected")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let strict_assertions = args
        .get("strict_assertions")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let context = build_context(args, &session);

    let source = match std::fs::read_to_string(spec_path) {
        Ok(s) => s,
        Err(e) => return Err((-32000, format!("{spec_path}: {e}"))),
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
            strict_assertions,
        },
    )
    .await;

    let root = collection_root_for(spec_path);
    let rel = spec_rel(spec_path, &root);

    let output = match execution_result {
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
            let duration_ms = execution.duration_ms;

            history::write_entry(
                &root,
                &rel,
                HistoryEntry {
                    timestamp: now_iso8601(),
                    passed,
                    status,
                    duration_ms,
                    error_type: error_type.map(str::to_string),
                },
            );

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

            if let Some(et) = error_type {
                result["hint"] = json!(hint_for_error_type(et));
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

            if infer_expected {
                if let Some(resp) = &execution.response {
                    let mut inferred =
                        format!("HTTP/1.1 {} {}\n", resp.status, http_reason(resp.status));
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

pub(super) async fn handle_flow(args: &Value) -> Result<Value, (i32, String)> {
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
    let verbose = args
        .get("verbose")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

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
                strict_assertions: args
                    .get("strict_assertions")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
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
        let steps: Vec<Value> = result
            .steps
            .iter()
            .map(|step| {
                let status = step
                    .execution
                    .as_ref()
                    .and_then(|e| e.response.as_ref())
                    .map(|r| r.status);
                let passed = step
                    .execution
                    .as_ref()
                    .map(|e| e.diff.passed)
                    .unwrap_or(false)
                    && step.error.is_none();
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

pub(super) fn handle_author(args: &Value) -> Result<Value, (i32, String)> {
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

    let ep = parse_endpoint(content, path)
        .map_err(|e| (-32000, format!("spec content is invalid: {e}")))?;

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

pub(super) async fn handle_vars(args: &Value) -> Result<Value, (i32, String)> {
    let spec_path = args
        .get("spec_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: spec_path is required".to_string()))?;

    let session = read_session();
    let env = resolve_env(args, &session);

    let source =
        std::fs::read_to_string(spec_path).map_err(|e| (-32000, format!("{spec_path}: {e}")))?;
    let endpoint =
        parse_endpoint(&source, Path::new(spec_path)).map_err(|e| (-32000, e.to_string()))?;

    let template_re =
        regex::Regex::new(r"\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}").expect("valid var regex");
    let path_re = regex::Regex::new(r":([A-Za-z_][A-Za-z0-9_]*)").expect("valid path param regex");

    let mut vars: std::collections::BTreeMap<String, &'static str> =
        std::collections::BTreeMap::new();

    for caps in template_re.captures_iter(&endpoint.request) {
        vars.insert(caps[1].to_string(), "template");
    }
    if let Some(first_line) = endpoint.request.lines().next() {
        for caps in path_re.captures_iter(first_line) {
            vars.entry(caps[1].to_string()).or_insert("path_param");
        }
    }

    let mut context = crate::resolver::Context::default();

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

    for (k, v) in std::env::vars() {
        if let Some(stripped) = k.strip_prefix("RQB_") {
            context.insert(SourceKind::OsEnv, env_name_to_var(stripped), v);
        } else if let Some(stripped) = k.strip_prefix("MAD_") {
            context.insert(SourceKind::OsEnv, env_name_to_var(stripped), v);
        }
    }

    let mut all_resolved = true;
    let variables: Vec<Value> = vars
        .iter()
        .map(|(name, kind)| {
            let resolved = context.get(name).is_some();
            if !resolved {
                all_resolved = false;
            }
            let source_label = if resolved { Some("env.md") } else { None };
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

fn env_name_to_var(name: &str) -> String {
    let mut parts = name.split('_').filter(|part| !part.is_empty());
    let Some(first) = parts.next() else {
        return String::new();
    };
    let mut out = first.to_ascii_lowercase();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(&chars.as_str().to_ascii_lowercase());
        }
    }
    out
}

pub(super) async fn handle_search(args: &Value) -> Result<Value, (i32, String)> {
    let q = args
        .get("q")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    let method_filter = args
        .get("method")
        .and_then(|v| v.as_str())
        .map(|s| s.to_uppercase());
    let path_filter = args.get("path").and_then(|v| v.as_str());
    let tag_filter = args.get("tag").and_then(|v| v.as_str());

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

pub(super) fn handle_context(args: &Value) -> Result<Value, (i32, String)> {
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or("api-docs");
    let target = args
        .get("target")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let changed_from = args
        .get("changed_from")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let token_budget = args
        .get("token_budget")
        .and_then(|v| v.as_u64())
        .unwrap_or(800) as usize;
    let verbose = args
        .get("verbose")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mode = args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("surgical");
    let mode = crate::agent_context::ContextMode::parse(mode)
        .map_err(|err| (-32602, format!("Invalid params: {err}")))?;
    let intent = args
        .get("intent")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let env = args
        .get("env")
        .and_then(|v| v.as_str())
        .unwrap_or("dev")
        .to_string();

    let text = crate::agent_context::render(crate::agent_context::AgentContextOptions {
        root: std::path::PathBuf::from(root),
        target,
        changed_from,
        token_budget,
        verbose,
        env,
        mode,
        intent,
    })
    .map_err(|err| (-32000, err.to_string()))?;
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

pub(super) async fn handle_history(args: &Value) -> Result<Value, (i32, String)> {
    let spec_path = args
        .get("spec_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: spec_path is required".to_string()))?;

    let last = args.get("last").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let root = collection_root_for(spec_path);
    let rel = spec_rel(spec_path, &root);
    let mut entries = history::read_history(&root, &rel);
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

pub(super) fn handle_session(args: &Value) -> Result<Value, (i32, String)> {
    use super::session::{read_session, write_session};

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
        other => Err((
            -32602,
            format!("Invalid params: unknown action \"{other}\""),
        )),
    }
}

pub(super) async fn handle_exec_batch(args: &Value) -> Result<Value, (i32, String)> {
    let specs = args
        .get("specs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            (
                -32602,
                "Invalid params: specs array is required".to_string(),
            )
        })?
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
                strict_assertions: args
                    .get("strict_assertions")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
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
