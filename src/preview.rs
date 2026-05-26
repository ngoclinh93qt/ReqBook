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
    importer::{self, curl as curl_importer},
    parser::{parse_endpoint, parse_env_config, EnvConfig},
    resolver::{Context, SourceKind},
};

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

#[derive(Debug, Deserialize, Default)]
struct ExecBody {
    #[serde(default)]
    vars: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct SaveVarsBody {
    env: Option<String>,
    #[serde(default)]
    vars: BTreeMap<String, String>,
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
    })
}

async fn api_spec_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
) -> Response {
    let file_path = state.root.join("api-docs").join(&rel_path);
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

async fn exec_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
    body: Bytes,
) -> impl IntoResponse {
    let extra_vars: BTreeMap<String, String> = if body.is_empty() {
        BTreeMap::new()
    } else {
        serde_json::from_slice::<ExecBody>(&body)
            .map(|b| b.vars)
            .unwrap_or_default()
    };
    let file_path = state.root.join("api-docs").join(&rel_path);
    match run_exec(&file_path, &state.root, &state.env, extra_vars).await {
        Ok(execution) => Json(execution).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string(), "diff": {"passed": false}})),
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
    let file_path = state.root.join("api-docs").join(&rel_path);
    if !file_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "spec not found"})),
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
                    .strip_prefix(state.root.join("api-docs"))
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

// ─── Business logic ───────────────────────────────────────────────────────────

async fn run_exec(
    file_path: &Path,
    root: &Path,
    env: &str,
    extra_vars: BTreeMap<String, String>,
) -> Result<crate::engine::Execution> {
    let source = fs::read_to_string(file_path)?;
    let endpoint = parse_endpoint(&source, file_path)?;
    let mut context = load_env_context(root, env);
    for (k, v) in extra_vars {
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

fn collect_specs(root: &Path) -> (String, Vec<ResourceGroup>) {
    use std::collections::BTreeMap as Map;
    let api_docs = root.join("api-docs");
    let project_name = read_project_name(&api_docs).unwrap_or_else(|| "API Specs".to_string());
    let mut groups: Map<String, ResourceGroup> = Map::new();
    if api_docs.exists() {
        collect_recursive(&api_docs, &api_docs, &mut groups);
    }
    (project_name, groups.into_values().collect())
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

pub async fn run(root: PathBuf, host: &str, port: u16, env: &str) -> Result<()> {
    let state = Arc::new(AppState {
        root,
        env: env.to_string(),
    });
    let app = Router::new()
        // JSON API
        .route("/api/index", get(api_index_handler))
        .route(
            "/api/spec/*path",
            get(api_spec_handler).put(save_spec_handler),
        )
        .route("/api/exec/*path", post(exec_handler))
        .route(
            "/api/variables",
            get(get_variables_handler).post(save_variables_handler),
        )
        .route("/api/import/curl", post(import_curl_handler))
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
}
