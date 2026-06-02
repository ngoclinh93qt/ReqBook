//! HTTP API handler functions.

use std::{fs, sync::Arc};

use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};

use crate::{
    adhoc::{self, AdHocParams},
    engine::ExecOpts,
    importer::{self, curl as curl_importer},
    mock::{path_matches, MockEntry},
    parser::{parse_endpoint, parse_env_config, parse_pipeline, EnvConfig},
    pipeline::{self, PipelineOpts},
    resolver::SourceKind,
};

use super::{
    business::{
        collect_flows, collect_specs, doc_path, flow_to_response, is_flow_rel_path,
        load_env_context, render_env_config, run_exec, scan_project, spec_path,
    },
    types::{
        AdHocReqBody, AdHocReqResponse, ExecBody, FlowsResponse, IndexResponse, RuntimeOverrides,
        SaveVarsBody, SpecResponse, ValidateResponse, API_DOCS_DIR,
    },
    AppState,
};

// ─── Spec handlers ────────────────────────────────────────────────────────────

pub(super) async fn api_index_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let (project_name, groups) = collect_specs(&state.current_root());
    let spec_count: usize = groups.iter().map(|g| g.specs.len()).sum();
    Json(IndexResponse {
        project_name,
        spec_count,
        groups,
        version: env!("CARGO_PKG_VERSION"),
        mock_mode: state.mock_mode(),
    })
}

pub(super) async fn api_spec_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
) -> Response {
    let file_path = spec_path(&state.current_root(), &rel_path);
    let source = match fs::read_to_string(&file_path) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("Spec not found: {rel_path}")})),
            )
                .into_response()
        }
    };
    match parse_endpoint(&source, &file_path) {
        Ok(ep) => Json(SpecResponse {
            title: ep.title,
            method: ep.schema.method.as_str().to_string(),
            path: ep.schema.path,
            description: ep.description,
            request: ep.request,
            expected_response: ep.expected_response,
            tests: ep.tests,
            raw_source: source,
            rel_path,
            env: state.env.clone(),
            version: env!("CARGO_PKG_VERSION"),
        })
        .into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub(super) async fn flows_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(FlowsResponse {
        flows: collect_flows(&state.current_root()),
    })
}

pub(super) async fn validate_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
) -> impl IntoResponse {
    let file_path = doc_path(&state.current_root(), &rel_path);
    let source = match fs::read_to_string(&file_path) {
        Ok(source) => source,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ValidateResponse {
                    valid: false,
                    kind: "unknown".to_string(),
                    path: rel_path,
                    error: Some(e.to_string()),
                }),
            )
                .into_response()
        }
    };
    let (kind, result) = if is_flow_rel_path(&rel_path) {
        ("flow", parse_pipeline(&source, &file_path).map(|_| ()))
    } else {
        ("api", parse_endpoint(&source, &file_path).map(|_| ()))
    };
    match result {
        Ok(()) => Json(ValidateResponse {
            valid: true,
            kind: kind.to_string(),
            path: rel_path,
            error: None,
        })
        .into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ValidateResponse {
                valid: false,
                kind: kind.to_string(),
                path: rel_path,
                error: Some(e.to_string()),
            }),
        )
            .into_response(),
    }
}

// ─── Flow handlers ────────────────────────────────────────────────────────────

pub(super) async fn flow_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
) -> Response {
    let file_path = doc_path(&state.current_root(), &rel_path);
    let source = match fs::read_to_string(&file_path) {
        Ok(source) => source,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("Flow not found: {rel_path}")})),
            )
                .into_response()
        }
    };
    match parse_pipeline(&source, &file_path) {
        Ok(flow) => Json(flow_to_response(flow, source, rel_path)).into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub(super) async fn save_flow_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
    body: Bytes,
) -> impl IntoResponse {
    let text = match std::str::from_utf8(&body) {
        Ok(text) => text,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "body must be UTF-8"})),
            )
                .into_response()
        }
    };
    let file_path = doc_path(&state.current_root(), &rel_path);
    if !is_flow_rel_path(&rel_path) || !rel_path.ends_with(".md") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "flow path must be under flows/*.md"})),
        )
            .into_response();
    }
    if let Err(e) = parse_pipeline(text, &file_path) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }
    if let Some(parent) = file_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    }
    match fs::write(&file_path, text) {
        Ok(()) => Json(serde_json::json!({"status": "saved"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub(super) async fn run_flow_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
) -> impl IntoResponse {
    let file_path = doc_path(&state.current_root(), &rel_path);
    let source = match fs::read_to_string(&file_path) {
        Ok(source) => source,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    let flow = match parse_pipeline(&source, &file_path) {
        Ok(flow) => flow,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    let root = state.current_root();
    match pipeline::run(
        &flow,
        &state.env,
        PipelineOpts {
            root: root.join(API_DOCS_DIR),
            exec: ExecOpts {
                context: load_env_context(&root, &state.env),
                ..ExecOpts::default()
            },
        },
    )
    .await
    {
        Ok(result) => Json(result).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ─── Exec handler ─────────────────────────────────────────────────────────────

pub(super) async fn exec_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
    body: Bytes,
) -> impl IntoResponse {
    let file_path = spec_path(&state.current_root(), &rel_path);

    if let Some(ref entries) = state.mock_entries {
        return mock_exec_response(&file_path, entries);
    }

    let overrides: RuntimeOverrides = if body.is_empty() {
        RuntimeOverrides::default()
    } else {
        serde_json::from_slice::<ExecBody>(&body)
            .map(|b| RuntimeOverrides {
                vars: b.vars,
                path_params: b.path_params,
                headers: b.headers,
                body: b.body.filter(|body| !body.is_empty()),
            })
            .unwrap_or_default()
    };
    match run_exec(&file_path, &state.current_root(), &state.env, overrides).await {
        Ok(execution) => Json(execution).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string(), "diff": {"passed": false}})),
        )
            .into_response(),
    }
}

fn mock_exec_response(file_path: &std::path::Path, entries: &[MockEntry]) -> Response {
    let source = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string(), "diff": {"passed": false}, "mock": true})),
            )
                .into_response()
        }
    };
    let endpoint = match parse_endpoint(&source, file_path) {
        Ok(ep) => ep,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string(), "diff": {"passed": false}, "mock": true})),
            )
                .into_response()
        }
    };
    let method = endpoint.schema.method.as_str().to_string();
    let pattern = &endpoint.schema.path;

    match entries
        .iter()
        .find(|e| e.method == method && path_matches(&e.pattern, pattern))
    {
        Some(entry) => {
            let body_str = String::from_utf8_lossy(&entry.body).to_string();
            let size = entry.body.len();
            Json(serde_json::json!({
                "request": {"method": method, "url": format!("(mock) {}", pattern), "headers": {}, "body": ""},
                "response": {"status": entry.status.as_u16(), "headers": {"content-type": entry.content_type.clone()}, "body": body_str, "size": size},
                "duration_ms": 0,
                "diff": {"passed": true},
                "mock": true
            }))
            .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("no mock response defined for {} {}", method, pattern),
                "diff": {"passed": false},
                "mock": true
            })),
        )
            .into_response(),
    }
}

pub(super) async fn save_spec_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
    body: Bytes,
) -> impl IntoResponse {
    let text = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "body must be UTF-8"})),
            )
                .into_response()
        }
    };
    let file_path = spec_path(&state.current_root(), &rel_path);
    if !file_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "spec not found"})),
        )
            .into_response();
    }
    if let Err(e) = parse_endpoint(text, &file_path) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }
    match fs::write(&file_path, text) {
        Ok(()) => Json(serde_json::json!({"status": "saved"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ─── Variables handlers ───────────────────────────────────────────────────────

pub(super) async fn get_variables_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let env_path = state.current_root().join("api-docs/_shared/env.md");
    let (vars, all_envs) = if let Ok(source) = fs::read_to_string(&env_path) {
        if let Ok(config) = parse_env_config(&source, &env_path) {
            let vars = config.envs.get(&state.env).cloned().unwrap_or_default();
            let all_envs: Vec<String> = config.envs.keys().cloned().collect();
            (vars, all_envs)
        } else {
            (std::collections::BTreeMap::new(), vec![])
        }
    } else {
        (std::collections::BTreeMap::new(), vec![])
    };
    Json(serde_json::json!({"env": state.env, "vars": vars, "envs": all_envs}))
}

pub(super) async fn save_variables_handler(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> impl IntoResponse {
    let body: SaveVarsBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    let env_name = body.env.unwrap_or_else(|| state.env.clone());
    let env_path = state.current_root().join("api-docs/_shared/env.md");
    let mut config = if let Ok(source) = fs::read_to_string(&env_path) {
        parse_env_config(&source, &env_path).unwrap_or_default()
    } else {
        EnvConfig::default()
    };
    config.envs.insert(env_name, body.vars);
    let dir = env_path.parent().expect("path has parent");
    if let Err(e) = fs::create_dir_all(dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }
    let rendered = render_env_config(&config);
    match fs::write(&env_path, rendered) {
        Ok(()) => Json(serde_json::json!({"status": "saved"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ─── Import handlers ──────────────────────────────────────────────────────────

pub(super) async fn import_curl_handler(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> impl IntoResponse {
    let text = match std::str::from_utf8(&body) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "request body must be UTF-8"})),
            )
                .into_response()
        }
    };
    if text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "empty curl command"})),
        )
            .into_response();
    }
    let endpoints = match curl_importer::import(&text) {
        Ok((_, eps)) => eps,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    match importer::write_endpoints(&state.current_root(), &endpoints) {
        Ok(written) => {
            if written.is_empty() {
                let ep = &endpoints[0];
                let spec = importer::render_endpoint(ep);
                Json(serde_json::json!({"status":"exists","message":"A spec for this endpoint already exists.","spec":spec})).into_response()
            } else {
                let path = &written[0];
                let rel_path = path
                    .strip_prefix(state.current_root().join(API_DOCS_DIR))
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned();
                let spec = importer::render_endpoint(&endpoints[0]);
                Json(serde_json::json!({"status":"created","path":path.display().to_string(),"rel_path":rel_path,"spec":spec})).into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub(super) async fn parse_curl_fields_handler(body: Bytes) -> impl IntoResponse {
    let text = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "request body must be UTF-8"})),
            )
                .into_response()
        }
    };
    match curl_importer::parse_to_fields(text) {
        Ok(parsed) => Json(serde_json::json!({
            "method": parsed.method,
            "url": parsed.url,
            "headers": parsed.headers,
            "body": parsed.body,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub(super) async fn scan_project_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match scan_project(&state.current_root(), false) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub(super) async fn import_project_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match scan_project(&state.current_root(), true) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ─── Ad-hoc request handler ───────────────────────────────────────────────────

pub(super) async fn adhoc_request_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AdHocReqBody>,
) -> impl IntoResponse {
    let params = AdHocParams {
        method: body.method.clone(),
        url: body.url.clone(),
        headers: body.headers.clone(),
        body: body.body.clone(),
        env: body.env.clone(),
    };

    let endpoint = match adhoc::build_endpoint(&params) {
        Ok(ep) => ep,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let mut context = load_env_context(&state.current_root(), &body.env);
    for (k, v) in &body.vars {
        context.insert(SourceKind::Cli, k, v);
    }

    let execution = match crate::engine::execute(
        &endpoint,
        &body.env,
        ExecOpts {
            context,
            ..ExecOpts::default()
        },
    )
    .await
    {
        Ok(ex) => ex,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let saved_path = if let Some(rel) = &body.save_as {
        let dest = state.current_root().join(API_DOCS_DIR).join(rel);
        let response_block = build_response_block(&execution);
        adhoc::save_to_path(&dest, &params, &response_block)
            .ok()
            .map(|_| rel.clone())
    } else {
        let response_block = build_response_block(&execution);
        adhoc::save_to_scratch(&params, &response_block)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    };

    Json(AdHocReqResponse {
        execution,
        saved_path,
    })
    .into_response()
}

fn build_response_block(execution: &crate::engine::Execution) -> String {
    execution
        .response
        .as_ref()
        .map(|r| {
            let mut block = format!("HTTP/1.1 {}\n", r.status);
            for (k, v) in &r.headers {
                block.push_str(&format!("{k}: {v}\n"));
            }
            if !r.body.is_empty() {
                block.push('\n');
                block.push_str(&r.body);
            }
            block
        })
        .unwrap_or_default()
}
