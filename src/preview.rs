//! Web preview server for the MarkApiDown API spec browser.
//! Serves a React SPA (embedded via rust-embed) + JSON API endpoints.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use anyhow::Result;
use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use mime_guess::from_path;
use owo_colors::OwoColorize;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};

use crate::{
    adhoc::{self, AdHocParams},
    engine::{self, ExecOpts},
    importer::{self, curl as curl_importer, project as project_importer},
    mock::{collect_entries, path_matches, MockEntry},
    parser::{parse_endpoint, parse_env_config, parse_pipeline, EnvConfig},
    pipeline::{self, PipelineOpts},
    resolver::{Context, SourceKind},
    workspace::{self, WorkspaceEntry},
};

const API_DOCS_DIR: &str = "api-docs";
const APIS_DIR: &str = "apis";
const FLOWS_DIR: &str = "flows";
const LEGACY_FLOWS_DIR: &str = "pipelines";

// ─── Static assets (React SPA) ───────────────────────────────────────────────

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct WebAssets;

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match WebAssets::get(path) {
        Some(content) => {
            let mime = from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: all unknown paths serve index.html (React Router handles them)
            match WebAssets::get("index.html") {
                Some(index) => (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    index.data.to_vec(),
                )
                    .into_response(),
                None => (
                    StatusCode::NOT_FOUND,
                    "UI not built. Run: cd web && npm run build",
                )
                    .into_response(),
            }
        }
    }
}

// ─── Shared state ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    root: Arc<std::sync::RwLock<PathBuf>>,
    env: String,
    mock_entries: Option<Vec<MockEntry>>,
}

impl AppState {
    fn current_root(&self) -> PathBuf {
        self.root.read().unwrap().clone()
    }

    fn mock_mode(&self) -> bool {
        self.mock_entries.is_some()
    }
}

// ─── Data types for API responses ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
struct SpecEntry {
    method: String,
    path: String,
    title: String,
    rel_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ResourceGroup {
    resource: String,
    specs: Vec<SpecEntry>,
}

#[derive(Debug, Clone, Serialize)]
struct IndexResponse {
    project_name: String,
    groups: Vec<ResourceGroup>,
    spec_count: usize,
    version: &'static str,
    mock_mode: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SpecResponse {
    title: String,
    method: String,
    path: String,
    description: String,
    request: String,
    expected_response: String,
    tests: Option<String>,
    rel_path: String,
    env: String,
    raw_source: String,
    version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct FlowEntry {
    name: String,
    title: String,
    rel_path: String,
    steps: usize,
}

#[derive(Debug, Clone, Serialize)]
struct FlowsResponse {
    flows: Vec<FlowEntry>,
}

#[derive(Debug, Clone, Serialize)]
struct FlowResponse {
    name: String,
    title: String,
    description: Option<String>,
    rel_path: String,
    raw_source: String,
    steps: Vec<FlowStepResponse>,
}

#[derive(Debug, Clone, Serialize)]
struct FlowStepResponse {
    name: String,
    endpoint: String,
    inject: Vec<String>,
    capture: Vec<FlowCaptureResponse>,
    assert: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct FlowCaptureResponse {
    source: String,
    name: String,
}

#[derive(Debug, Deserialize, Default)]
struct ExecBody {
    #[serde(default)]
    vars: BTreeMap<String, String>,
    #[serde(default, alias = "params")]
    path_params: BTreeMap<String, String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    body: Option<String>,
}

#[derive(Debug, Default)]
struct RuntimeOverrides {
    vars: BTreeMap<String, String>,
    path_params: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SaveVarsBody {
    env: Option<String>,
    #[serde(default)]
    vars: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
struct ScanRoute {
    method: String,
    path: String,
    title: String,
    resource: String,
    exists: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ScanProjectResponse {
    project_name: String,
    routes_found: usize,
    missing_count: usize,
    existing_count: usize,
    duration_ms: u128,
    routes: Vec<ScanRoute>,
    written: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ValidateResponse {
    valid: bool,
    kind: String,
    path: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GitBranchEntry {
    name: String,
    current: bool,
    remote: bool,
    upstream: Option<String>,
    commit: Option<String>,
    summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GitBranchesResponse {
    is_repo: bool,
    root: Option<String>,
    current: Option<String>,
    dirty: bool,
    branches: Vec<GitBranchEntry>,
}

/// Body for `POST /api/request`   ad-hoc request without a spec file.
#[derive(Debug, Deserialize)]
struct AdHocReqBody {
    method: String,
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    body: Option<String>,
    #[serde(default)]
    vars: BTreeMap<String, String>,
    #[serde(default = "default_env")]
    env: String,
    /// If Some, save as spec at this relative path inside api-docs/.
    save_as: Option<String>,
}

fn default_env() -> String {
    "dev".to_string()
}

#[derive(Debug, Serialize)]
struct AdHocReqResponse {
    #[serde(flatten)]
    execution: crate::engine::Execution,
    saved_path: Option<String>,
}

// ─── API Handlers ─────────────────────────────────────────────────────────────

async fn api_index_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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

async fn api_spec_handler(
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

async fn flows_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(FlowsResponse {
        flows: collect_flows(&state.current_root()),
    })
}

async fn validate_handler(
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

async fn flow_handler(
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

async fn save_flow_handler(
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

async fn run_flow_handler(
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
    match pipeline::run(
        &flow,
        &state.env,
        PipelineOpts {
            root: state.current_root().join(API_DOCS_DIR),
            exec: ExecOpts {
                context: load_env_context(&state.current_root(), &state.env),
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

async fn exec_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
    body: Bytes,
) -> impl IntoResponse {
    let file_path = spec_path(&state.current_root(), &rel_path);

    // Mock mode: return mock response directly from spec's expected response block.
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
                "request": {
                    "method": method,
                    "url": format!("(mock) {}", pattern),
                    "headers": {},
                    "body": ""
                },
                "response": {
                    "status": entry.status.as_u16(),
                    "headers": {"content-type": entry.content_type.clone()},
                    "body": body_str,
                    "size": size
                },
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

async fn save_spec_handler(
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

async fn get_variables_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let env_path = state.current_root().join("api-docs/_shared/env.md");
    let (vars, all_envs) = if let Ok(source) = fs::read_to_string(&env_path) {
        if let Ok(config) = parse_env_config(&source, &env_path) {
            let vars = config.envs.get(&state.env).cloned().unwrap_or_default();
            let all_envs: Vec<String> = config.envs.keys().cloned().collect();
            (vars, all_envs)
        } else {
            (BTreeMap::new(), vec![])
        }
    } else {
        (BTreeMap::new(), vec![])
    };
    Json(serde_json::json!({
        "env": state.env,
        "vars": vars,
        "envs": all_envs,
    }))
}

async fn save_variables_handler(
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

async fn import_curl_handler(State(state): State<Arc<AppState>>, body: Bytes) -> impl IntoResponse {
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

async fn parse_curl_fields_handler(body: Bytes) -> impl IntoResponse {
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

async fn scan_project_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match scan_project(&state.current_root(), false) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn import_project_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match scan_project(&state.current_root(), true) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn adhoc_request_handler(
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

    let execution = match engine::execute(
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
        let response_block = execution
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
            .unwrap_or_default();
        match adhoc::save_to_path(&dest, &params, &response_block) {
            Ok(()) => Some(rel.clone()),
            Err(_) => None,
        }
    } else {
        // Auto-save to scratch.
        let response_block = execution
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
            .unwrap_or_default();
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

// ─── Business logic ───────────────────────────────────────────────────────────

fn doc_path(root: &Path, rel_path: &str) -> PathBuf {
    let api_docs = root.join(API_DOCS_DIR);
    let direct = api_docs.join(rel_path);
    if direct.exists() {
        return direct;
    }
    if let Some(rest) = rel_path.strip_prefix(&format!("{LEGACY_FLOWS_DIR}/")) {
        let modern = api_docs.join(FLOWS_DIR).join(rest);
        if modern.exists() {
            return modern;
        }
    }
    direct
}

fn spec_path(root: &Path, rel_path: &str) -> PathBuf {
    let api_docs = root.join(API_DOCS_DIR);
    let direct = api_docs.join(rel_path);
    if direct.exists() || rel_path.starts_with(&format!("{APIS_DIR}/")) {
        direct
    } else {
        let nested = api_docs.join(APIS_DIR).join(rel_path);
        if nested.exists() {
            nested
        } else {
            direct
        }
    }
}

fn is_flow_rel_path(rel_path: &str) -> bool {
    rel_path.starts_with(&format!("{FLOWS_DIR}/"))
        || rel_path.starts_with(&format!("{LEGACY_FLOWS_DIR}/"))
}

fn endpoint_roots(root: &Path) -> Vec<PathBuf> {
    let api_docs = root.join(API_DOCS_DIR);
    let apis = api_docs.join(APIS_DIR);
    if apis.exists() {
        vec![apis]
    } else {
        vec![api_docs]
    }
}

fn flow_roots(root: &Path) -> Vec<PathBuf> {
    let api_docs = root.join(API_DOCS_DIR);
    [FLOWS_DIR, LEGACY_FLOWS_DIR]
        .into_iter()
        .map(|dir| api_docs.join(dir))
        .filter(|path| path.exists())
        .collect()
}

async fn run_exec(
    file_path: &Path,
    root: &Path,
    env: &str,
    overrides: RuntimeOverrides,
) -> Result<crate::engine::Execution> {
    let source = fs::read_to_string(file_path)?;
    let mut endpoint = parse_endpoint(&source, file_path)?;
    endpoint.request = apply_runtime_overrides(&endpoint.request, &overrides);
    let mut context = load_env_context(root, env);
    for (k, v) in overrides.vars {
        context.insert(SourceKind::Cli, k, v);
    }
    Ok(engine::execute(
        &endpoint,
        env,
        ExecOpts {
            context,
            ..ExecOpts::default()
        },
    )
    .await?)
}

fn apply_runtime_overrides(source: &str, overrides: &RuntimeOverrides) -> String {
    if overrides.path_params.is_empty() && overrides.headers.is_empty() && overrides.body.is_none()
    {
        return source.to_string();
    }

    let mut parts = source.splitn(2, "\n\n");
    let head = parts.next().unwrap_or_default();
    let original_body = parts.next().unwrap_or_default();
    let mut lines = head.lines();
    let Some(request_line) = lines.next() else {
        return source.to_string();
    };

    let mut request_parts = request_line.split_whitespace();
    let Some(method) = request_parts.next() else {
        return source.to_string();
    };
    let Some(mut url) = request_parts.next().map(ToOwned::to_owned) else {
        return source.to_string();
    };

    for (name, value) in &overrides.path_params {
        if !value.is_empty() {
            url = url.replace(&format!(":{name}"), value);
        }
    }

    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_string(), value.trim().to_string());
        }
    }
    for (name, value) in &overrides.headers {
        if !name.trim().is_empty() {
            headers.insert(name.trim().to_string(), value.trim().to_string());
        }
    }

    let mut next = format!("{method} {url}");
    for (name, value) in headers {
        next.push('\n');
        next.push_str(&name);
        next.push_str(": ");
        next.push_str(&value);
    }

    let body = overrides.body.as_deref().unwrap_or(original_body);
    if !body.is_empty() {
        next.push_str("\n\n");
        next.push_str(body);
    }
    next
}

fn load_env_context(root: &Path, env: &str) -> Context {
    let env_path = root.join("api-docs/_shared/env.md");
    let mut context = Context::default();
    if let Ok(source) = fs::read_to_string(&env_path) {
        if let Ok(config) = parse_env_config(&source, &env_path) {
            if let Some(vars) = config.envs.get(env) {
                for (key, value) in vars {
                    context.insert(SourceKind::Env, key, value);
                }
            }
        }
    }
    context
}

fn scan_project(root: &Path, write_missing: bool) -> Result<ScanProjectResponse> {
    let started = std::time::Instant::now();
    let (project_name, endpoints) = project_importer::import(root)?;
    let existing = existing_endpoint_keys(root);
    let mut routes = Vec::with_capacity(endpoints.len());
    let mut missing = Vec::new();

    for endpoint in endpoints {
        let key = endpoint_key(&endpoint.method, &endpoint.path);
        let exists = existing.contains(&key);
        routes.push(ScanRoute {
            method: endpoint.method.clone(),
            path: endpoint.path.clone(),
            title: endpoint.title.clone(),
            resource: endpoint.resource.clone(),
            exists,
        });
        if !exists {
            missing.push(endpoint);
        }
    }

    let written = if write_missing && !missing.is_empty() {
        importer::write_endpoints(root, &missing)?
            .into_iter()
            .map(|path| {
                path.strip_prefix(root.join(API_DOCS_DIR))
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .trim_start_matches(std::path::MAIN_SEPARATOR)
                    .to_string()
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(ScanProjectResponse {
        project_name,
        routes_found: routes.len(),
        missing_count: missing.len(),
        existing_count: routes.iter().filter(|route| route.exists).count(),
        duration_ms: started.elapsed().as_millis(),
        routes,
        written,
    })
}

fn existing_endpoint_keys(root: &Path) -> std::collections::HashSet<(String, String)> {
    let mut keys = std::collections::HashSet::new();
    for dir in endpoint_roots(root) {
        collect_existing_keys(&dir, &mut keys);
    }
    keys
}

fn collect_existing_keys(dir: &Path, keys: &mut std::collections::HashSet<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_existing_keys(&path, keys);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if matches!(
                name,
                "README.md" | "mad.md" | "env.md" | "auth.md" | "variables.md"
            ) {
                continue;
            }
            if let Ok(source) = fs::read_to_string(&path) {
                if let Ok(endpoint) = parse_endpoint(&source, &path) {
                    keys.insert(endpoint_key(
                        endpoint.schema.method.as_str(),
                        &endpoint.schema.path,
                    ));
                }
            }
        }
    }
}

fn endpoint_key(method: &str, path: &str) -> (String, String) {
    (
        method.to_uppercase(),
        path.trim_end_matches('/').to_string(),
    )
}

fn collect_specs(root: &Path) -> (String, Vec<ResourceGroup>) {
    use std::collections::BTreeMap as Map;
    let api_docs = root.join(API_DOCS_DIR);
    let project_name = read_project_name(&api_docs).unwrap_or_else(|| "API Specs".to_string());
    let mut groups: Map<String, ResourceGroup> = Map::new();
    for endpoint_root in endpoint_roots(root) {
        if endpoint_root.exists() {
            collect_recursive(&api_docs, &endpoint_root, &mut groups);
        }
    }
    (project_name, groups.into_values().collect())
}

fn collect_flows(root: &Path) -> Vec<FlowEntry> {
    let mut flows = Vec::new();
    for flow_root in flow_roots(root) {
        collect_flows_recursive(&root.join(API_DOCS_DIR), &flow_root, &mut flows);
    }
    flows.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    flows
}

fn collect_flows_recursive(api_docs: &Path, dir: &Path, flows: &mut Vec<FlowEntry>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_flows_recursive(api_docs, &path, flows);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            if let Ok(source) = fs::read_to_string(&path) {
                if let Ok(flow) = parse_pipeline(&source, &path) {
                    let rel = path
                        .strip_prefix(api_docs)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();
                    flows.push(FlowEntry {
                        name: flow.schema.name,
                        title: flow.title,
                        rel_path: rel,
                        steps: flow.steps.len(),
                    });
                }
            }
        }
    }
}

fn flow_to_response(
    flow: crate::parser::Pipeline,
    raw_source: String,
    rel_path: String,
) -> FlowResponse {
    FlowResponse {
        name: flow.schema.name,
        title: flow.title,
        description: flow.schema.description,
        rel_path,
        raw_source,
        steps: flow
            .steps
            .into_iter()
            .map(|step| FlowStepResponse {
                name: step.name,
                endpoint: step.endpoint,
                inject: step.inject,
                capture: step
                    .capture
                    .into_iter()
                    .map(|capture| FlowCaptureResponse {
                        source: capture.source,
                        name: capture.name,
                    })
                    .collect(),
                assert: step.assert,
            })
            .collect(),
    }
}

fn collect_recursive(
    api_docs: &Path,
    dir: &Path,
    groups: &mut std::collections::BTreeMap<String, ResourceGroup>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_recursive(api_docs, &path, groups);
        } else if path.extension().is_some_and(|e| e == "md") {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if matches!(
                name,
                "README.md" | "mad.md" | "env.md" | "auth.md" | "variables.md"
            ) {
                continue;
            }
            if let Ok(source) = fs::read_to_string(&path) {
                if let Ok(ep) = parse_endpoint(&source, &path) {
                    let rel_path = path
                        .strip_prefix(api_docs)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();
                    let entry = SpecEntry {
                        method: ep.schema.method.as_str().to_string(),
                        path: ep.schema.path.clone(),
                        title: ep.title.clone(),
                        rel_path,
                    };
                    groups
                        .entry(ep.schema.resource.clone())
                        .or_insert_with(|| ResourceGroup {
                            resource: ep.schema.resource.clone(),
                            specs: Vec::new(),
                        })
                        .specs
                        .push(entry);
                }
            }
        }
    }
}

fn read_project_name(api_docs: &Path) -> Option<String> {
    let source = fs::read_to_string(api_docs.join("mad.md")).ok()?;
    let rest = source.strip_prefix("---\n")?;
    for line in rest.lines() {
        if line == "---" {
            break;
        }
        if let Some(name) = line.strip_prefix("name: ") {
            return Some(name.trim().to_string());
        }
    }
    None
}

fn render_env_config(config: &EnvConfig) -> String {
    let mut out = String::from("# Environments\n");
    for (env, vars) in &config.envs {
        out.push_str(&format!("\n## {env}\n\n```yaml\n"));
        for (k, v) in vars {
            // YAML: quote if empty, starts with a special char, or contains ": " (colon+space)
            let needs_quotes = v.is_empty() || v.starts_with(['{', '[', '#']) || v.contains(": ");
            if needs_quotes {
                let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
                out.push_str(&format!("{k}: \"{escaped}\"\n"));
            } else {
                out.push_str(&format!("{k}: {v}\n"));
            }
        }
        out.push_str("```\n");
    }
    out
}

// ─── Workspace management routes ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenWorkspaceBody {
    path: String,
}

#[derive(Debug, Deserialize)]
struct CreateWorkspaceBody {
    path: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CheckoutBranchBody {
    branch: String,
}

async fn workspace_current_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let root = state.current_root();
    let name = workspace::workspace_name(&root).unwrap_or_else(|| {
        root.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned())
    });
    Json(WorkspaceEntry {
        path: root.to_string_lossy().into_owned(),
        name,
        last_opened: None,
    })
}

async fn workspace_recent_handler() -> impl IntoResponse {
    Json(workspace::load_history())
}

async fn workspace_all_handler() -> impl IntoResponse {
    Json(workspace::list_all_workspaces())
}

async fn workspace_open_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<OpenWorkspaceBody>,
) -> impl IntoResponse {
    let new_root = PathBuf::from(&body.path);
    if !new_root.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "path does not exist"})),
        )
            .into_response();
    }
    let name = workspace::workspace_name(&new_root).unwrap_or_else(|| {
        new_root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| body.path.clone())
    });
    *state.root.write().unwrap() = new_root.clone();
    workspace::save_to_history(&new_root, &name);
    Json(serde_json::json!({"status": "ok", "name": name})).into_response()
}

async fn workspace_create_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateWorkspaceBody>,
) -> impl IntoResponse {
    let dir = PathBuf::from(&body.path);
    let name = body.name.unwrap_or_else(|| {
        dir.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "my-api".to_string())
    });
    if let Err(e) = workspace::init_workspace_dir(&dir, &name) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }
    *state.root.write().unwrap() = dir.clone();
    workspace::save_to_history(&dir, &name);
    Json(serde_json::json!({"status": "ok", "name": name})).into_response()
}

// ─── Git routes ──────────────────────────────────────────────────────────────

async fn git_branches_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match git_branches_for_workspace(&state.current_root()) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn git_checkout_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CheckoutBranchBody>,
) -> impl IntoResponse {
    let target = body.branch.trim();
    if target.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "branch is required"})),
        )
            .into_response();
    }

    let workspace_root = state.current_root();
    let Some(repo_root) = git_repo_root(&workspace_root) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "workspace is not inside a git repository"})),
        )
            .into_response();
    };

    let branch_list = match git_branches_for_root(&repo_root) {
        Ok(response) => response,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        }
    };

    let Some(branch) = branch_list
        .branches
        .iter()
        .find(|branch| branch.name == target)
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("unknown branch: {target}")})),
        )
            .into_response();
    };

    let result = if branch.current {
        Ok(String::new())
    } else if branch.remote {
        run_git(&repo_root, &["switch", "--track", &branch.name])
    } else {
        run_git(&repo_root, &["switch", "--", &branch.name])
    };

    match result {
        Ok(_) => match git_branches_for_root(&repo_root) {
            Ok(response) => Json(response).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response(),
        },
        Err(e) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": e}))).into_response(),
    }
}

fn git_branches_for_workspace(root: &Path) -> std::result::Result<GitBranchesResponse, String> {
    let Some(repo_root) = git_repo_root(root) else {
        return Ok(GitBranchesResponse {
            is_repo: false,
            root: None,
            current: None,
            dirty: false,
            branches: Vec::new(),
        });
    };
    git_branches_for_root(&repo_root)
}

fn git_branches_for_root(repo_root: &Path) -> std::result::Result<GitBranchesResponse, String> {
    let current = git_current_branch(repo_root)?;
    let dirty = git_is_dirty(repo_root)?;
    let output = run_git(
        repo_root,
        &[
            "for-each-ref",
            "--format=%(refname)%09%(refname:short)%09%(upstream:short)%09%(HEAD)%09%(objectname:short)%09%(subject)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;

    let mut locals = std::collections::HashSet::new();
    let mut parsed = Vec::new();
    for line in output.lines() {
        let mut parts = line.splitn(6, '\t');
        let full_ref = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default().trim();
        let upstream = parts.next().unwrap_or_default().trim();
        let marker = parts.next().unwrap_or_default().trim();
        let commit = parts.next().unwrap_or_default().trim();
        let summary = parts.next().unwrap_or_default().trim();
        if name.is_empty() || full_ref.ends_with("/HEAD") {
            continue;
        }
        let remote = full_ref.starts_with("refs/remotes/");
        if !remote {
            locals.insert(name.to_string());
        }
        parsed.push(GitBranchEntry {
            name: name.to_string(),
            current: marker == "*",
            remote,
            upstream: non_empty(upstream),
            commit: non_empty(commit),
            summary: non_empty(summary),
        });
    }

    let mut branches: Vec<_> = parsed
        .into_iter()
        .filter(|branch| {
            if !branch.remote {
                return true;
            }
            remote_local_name(&branch.name)
                .map(|local| !locals.contains(local))
                .unwrap_or(true)
        })
        .collect();
    branches.sort_by(|a, b| {
        b.current
            .cmp(&a.current)
            .then_with(|| a.remote.cmp(&b.remote))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(GitBranchesResponse {
        is_repo: true,
        root: Some(repo_root.to_string_lossy().into_owned()),
        current,
        dirty,
        branches,
    })
}

fn git_repo_root(root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let path = stdout.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn git_current_branch(repo_root: &Path) -> std::result::Result<Option<String>, String> {
    let branch = run_git(repo_root, &["branch", "--show-current"])?;
    let branch = branch.trim();
    if !branch.is_empty() {
        return Ok(Some(branch.to_string()));
    }
    let commit = run_git(repo_root, &["rev-parse", "--short", "HEAD"])?;
    let commit = commit.trim();
    Ok((!commit.is_empty()).then(|| format!("detached@{commit}")))
}

fn git_is_dirty(repo_root: &Path) -> std::result::Result<bool, String> {
    Ok(!run_git(repo_root, &["status", "--porcelain"])?
        .trim()
        .is_empty())
}

fn run_git(repo_root: &Path, args: &[&str]) -> std::result::Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        return String::from_utf8(output.stdout).map_err(|e| e.to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let msg = if !stderr.is_empty() { stderr } else { stdout };
    Err(if msg.is_empty() {
        format!("git exited with {}", output.status)
    } else {
        msg
    })
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn remote_local_name(remote: &str) -> Option<&str> {
    remote.split_once('/').map(|(_, branch)| branch)
}

// ─── Server entry point ───────────────────────────────────────────────────────

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // JSON API
        .route("/api/index", get(api_index_handler))
        .route(
            "/api/spec/*path",
            get(api_spec_handler).put(save_spec_handler),
        )
        .route("/api/flows", get(flows_handler))
        .route("/api/validate/*path", get(validate_handler))
        .route(
            "/api/flow/*path",
            get(flow_handler)
                .put(save_flow_handler)
                .post(run_flow_handler),
        )
        .route("/api/exec/*path", post(exec_handler))
        .route(
            "/api/variables",
            get(get_variables_handler).post(save_variables_handler),
        )
        .route("/api/request", post(adhoc_request_handler))
        .route("/api/parse-curl", post(parse_curl_fields_handler))
        .route("/api/import/curl", post(import_curl_handler))
        .route(
            "/api/scan/project",
            get(scan_project_handler).post(import_project_handler),
        )
        .route(
            "/api/sync/project",
            get(scan_project_handler).post(import_project_handler),
        )
        // Workspace management
        .route("/api/workspace/current", get(workspace_current_handler))
        .route("/api/workspace/recent", get(workspace_recent_handler))
        .route("/api/workspace/all", get(workspace_all_handler))
        .route("/api/workspace/open", post(workspace_open_handler))
        .route("/api/workspace/create", post(workspace_create_handler))
        .route("/api/git/branches", get(git_branches_handler))
        .route("/api/git/checkout", post(git_checkout_handler))
        // SPA static files (catch-all)
        .fallback(static_handler)
        .with_state(state)
}

async fn bind_listener(host: &str, port: u16) -> Result<tokio::net::TcpListener> {
    let mut candidate = port;
    loop {
        let addr = format!("{host}:{candidate}");
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => {
                if candidate != port {
                    println!("{} Port {} in use, using {}", "!".yellow(), port, candidate);
                }
                return Ok(l);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && candidate < port + 10 => {
                candidate += 1;
            }
            Err(e) => {
                return Err(anyhow::anyhow!("cannot bind to {host}:{port}: {e}"));
            }
        }
    }
}

/// CLI entry point: starts the server and blocks until Ctrl-C.
pub async fn run(root: PathBuf, host: &str, port: u16, env: &str, mock: bool) -> Result<()> {
    run_with_ready(
        Arc::new(std::sync::RwLock::new(root)),
        host,
        port,
        env,
        mock,
        |_| {},
    )
    .await
}

/// Start the axum server with a pre-created shared workspace root.
///
/// Fires `on_ready(actual_port)` once the TCP listener is bound — the Tauri
/// app uses this to navigate the webview to the correct URL before the server
/// has processed a single request.
pub async fn run_with_ready<F>(
    root: Arc<std::sync::RwLock<PathBuf>>,
    host: &str,
    port: u16,
    env: &str,
    mock: bool,
    on_ready: F,
) -> Result<()>
where
    F: FnOnce(u16) + Send + 'static,
{
    let mock_entries = if mock {
        let r = root.read().unwrap().clone();
        let api_docs = r.join("api-docs");
        let dir = if api_docs.exists() { api_docs } else { r };
        let entries = collect_entries(&dir)?;
        println!(
            "{} Mock mode   {} route(s) loaded",
            "→".cyan(),
            entries.len()
        );
        Some(entries)
    } else {
        None
    };
    let state = Arc::new(AppState {
        root,
        env: env.to_string(),
        mock_entries,
    });
    let app = build_router(state);
    let listener = bind_listener(host, port).await?;
    let local_addr = listener.local_addr()?;
    println!("{} Preview:  http://{local_addr}", "✓".green());
    println!("    Press Ctrl-C to stop.");
    on_ready(local_addr.port());
    axum::serve(listener, app).await?;
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_specs_on_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let (name, groups) = collect_specs(dir.path());
        assert_eq!(name, "API Specs");
        assert!(groups.is_empty());
    }

    #[test]
    fn collect_specs_finds_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let api_docs = dir.path().join("api-docs");
        let users_dir = api_docs.join("users");
        fs::create_dir_all(&users_dir).unwrap();
        fs::write(
            users_dir.join("get-user.md"),
            r#"---
resource: users
protocol: http
method: GET
path: /users/:id
version: 1
---
# Get user

## Request

```http
GET {{baseUrl}}/users/:id
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```
"#,
        )
        .unwrap();
        let (_, groups) = collect_specs(dir.path());
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].resource, "users");
        assert_eq!(groups[0].specs[0].method, "GET");
    }

    #[test]
    fn render_env_config_basic() {
        use std::collections::BTreeMap;
        let mut vars = BTreeMap::new();
        vars.insert("baseUrl".to_string(), "https://example.com".to_string());
        let mut config = EnvConfig::default();
        config.envs.insert("dev".to_string(), vars);
        let rendered = render_env_config(&config);
        assert!(rendered.contains("## dev"));
        assert!(rendered.contains("baseUrl: https://example.com"));
    }

    #[test]
    fn render_env_config_quotes_special_values() {
        use std::collections::BTreeMap;
        let mut vars = BTreeMap::new();
        // "value: with colon-space" must be quoted (YAML key: value ambiguity)
        vars.insert("label".to_string(), "key: value".to_string());
        // plain URLs without colon-space do NOT need quotes
        vars.insert("baseUrl".to_string(), "https://example.com".to_string());
        let mut config = EnvConfig::default();
        config.envs.insert("dev".to_string(), vars);
        let rendered = render_env_config(&config);
        assert!(rendered.contains("label: \"key: value\""));
        assert!(rendered.contains("baseUrl: https://example.com"));
    }

    #[test]
    fn runtime_overrides_request_without_touching_source() {
        let source = "POST https://example.com/users/:id\nAccept: application/json\n\n{}";
        let mut overrides = RuntimeOverrides::default();
        overrides
            .path_params
            .insert("id".to_string(), "42".to_string());
        overrides
            .headers
            .insert("Authorization".to_string(), "Bearer {{token}}".to_string());
        overrides.body = Some("{\"name\":\"Ada\"}".to_string());

        let rendered = apply_runtime_overrides(source, &overrides);

        assert_eq!(
            rendered,
            "POST https://example.com/users/42\nAccept: application/json\nAuthorization: Bearer {{token}}\n\n{\"name\":\"Ada\"}"
        );
        assert_eq!(
            source,
            "POST https://example.com/users/:id\nAccept: application/json\n\n{}"
        );
    }
}
