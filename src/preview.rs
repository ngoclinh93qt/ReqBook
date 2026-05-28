//! Web preview server for the Trellis API spec browser.
//! Serves a React SPA (embedded via rust-embed) + JSON API endpoints.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
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
    engine::{self, ExecOpts},
    importer::{self, curl as curl_importer, project as project_importer},
    mock::{collect_entries, path_matches, MockEntry},
    parser::{parse_endpoint, parse_env_config, parse_pipeline, EnvConfig},
    pipeline::{self, PipelineOpts},
    resolver::{Context, SourceKind},
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
    root: PathBuf,
    env: String,
    mock_entries: Option<Vec<MockEntry>>,
}

impl AppState {
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

// ─── API Handlers ─────────────────────────────────────────────────────────────

async fn api_index_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let (project_name, groups) = collect_specs(&state.root);
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
    let file_path = spec_path(&state.root, &rel_path);
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
        flows: collect_flows(&state.root),
    })
}

async fn validate_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
) -> impl IntoResponse {
    let file_path = doc_path(&state.root, &rel_path);
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
    let file_path = doc_path(&state.root, &rel_path);
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
    let file_path = doc_path(&state.root, &rel_path);
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
    let file_path = doc_path(&state.root, &rel_path);
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
            root: state.root.join(API_DOCS_DIR),
            exec: ExecOpts {
                context: load_env_context(&state.root, &state.env),
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
    let file_path = spec_path(&state.root, &rel_path);

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
    match run_exec(&file_path, &state.root, &state.env, overrides).await {
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

    match entries.iter().find(|e| e.method == method && path_matches(&e.pattern, pattern)) {
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
    let file_path = spec_path(&state.root, &rel_path);
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
    let env_path = state.root.join("api-docs/_shared/env.md");
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
    let env_path = state.root.join("api-docs/_shared/env.md");
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
    match importer::write_endpoints(&state.root, &endpoints) {
        Ok(written) => {
            if written.is_empty() {
                let ep = &endpoints[0];
                let spec = importer::render_endpoint(ep);
                Json(serde_json::json!({"status":"exists","message":"A spec for this endpoint already exists.","spec":spec})).into_response()
            } else {
                let path = &written[0];
                let rel_path = path
                    .strip_prefix(state.root.join(API_DOCS_DIR))
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

async fn scan_project_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match scan_project(&state.root, false) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn import_project_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match scan_project(&state.root, true) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
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
                "README.md" | "trellis.md" | "env.md" | "auth.md" | "variables.md"
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
                "README.md" | "trellis.md" | "env.md" | "auth.md" | "variables.md"
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
    let source = fs::read_to_string(api_docs.join("trellis.md")).ok()?;
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

// ─── Server entry point ───────────────────────────────────────────────────────

pub async fn run(root: PathBuf, host: &str, port: u16, env: &str, mock: bool) -> Result<()> {
    let mock_entries = if mock {
        let api_docs = root.join("api-docs");
        let dir = if api_docs.exists() { api_docs } else { root.clone() };
        let entries = collect_entries(&dir)?;
        println!(
            "{} Mock mode — {} route(s) loaded",
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
    let app = Router::new()
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
        .route("/api/import/curl", post(import_curl_handler))
        .route(
            "/api/scan/project",
            get(scan_project_handler).post(import_project_handler),
        )
        .route(
            "/api/sync/project",
            get(scan_project_handler).post(import_project_handler),
        )
        // SPA static files (catch-all)
        .fallback(static_handler)
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("cannot bind to {addr}: {e}"))?;
    let local_addr = listener.local_addr()?;
    println!("{} Preview:  http://{local_addr}", "✓".green());
    println!("    Press Ctrl-C to stop.");
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
