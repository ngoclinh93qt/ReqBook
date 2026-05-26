//! Web preview server for the Trellis API spec browser.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use askama::Template;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use owo_colors::OwoColorize;

use crate::{
    engine::{self, ExecOpts},
    parser::{parse_endpoint, parse_env_config},
    resolver::{Context, SourceKind},
};

// ─── Template data types ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct SpecEntry {
    method: String,
    path: String,
    title: String,
    rel_path: String,
}

#[derive(Debug, Clone)]
struct ResourceGroup {
    resource: String,
    specs: Vec<SpecEntry>,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    project_name: String,
    groups: Vec<ResourceGroup>,
    spec_count: usize,
    version: &'static str,
}

#[derive(Template)]
#[template(path = "spec.html")]
struct SpecTemplate {
    title: String,
    method: String,
    path: String,
    description: String,
    request: String,
    expected_response: String,
    tests: Option<String>,
    rel_path: String,
    env: String,
    version: &'static str,
}

// ─── Shared state ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    env: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

async fn index_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let (project_name, groups) = collect_specs(&state.root);
    let spec_count: usize = groups.iter().map(|g| g.specs.len()).sum();
    render_template(IndexTemplate {
        project_name,
        spec_count,
        groups,
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn spec_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
) -> impl IntoResponse {
    let file_path = state.root.join("api-docs").join(&rel_path);
    let source = match fs::read_to_string(&file_path) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Html(error_html(&format!("Spec not found: {rel_path}"))),
            )
                .into_response()
        }
    };
    match parse_endpoint(&source, &file_path) {
        Ok(ep) => render_template(SpecTemplate {
            title: ep.title,
            method: ep.schema.method.as_str().to_string(),
            path: ep.schema.path,
            description: ep.description,
            request: ep.request,
            expected_response: ep.expected_response,
            tests: ep.tests,
            rel_path,
            env: state.env.clone(),
            version: env!("CARGO_PKG_VERSION"),
        }),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Html(error_html(&e.to_string())),
        )
            .into_response(),
    }
}

async fn exec_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(rel_path): AxumPath<String>,
) -> impl IntoResponse {
    let file_path = state.root.join("api-docs").join(&rel_path);
    match run_exec(&file_path, &state.root, &state.env).await {
        Ok(execution) => Json(execution).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string(), "diff": {"passed": false}})),
        )
            .into_response(),
    }
}

// ─── Business logic ───────────────────────────────────────────────────────────

async fn run_exec(file_path: &Path, root: &Path, env: &str) -> Result<crate::engine::Execution> {
    let source = fs::read_to_string(file_path)?;
    let endpoint = parse_endpoint(&source, file_path)?;
    let context = load_env_context(root, env);
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
    let api_docs = root.join("api-docs");
    let project_name = read_project_name(&api_docs).unwrap_or_else(|| "API Specs".to_string());

    let mut groups: BTreeMap<String, ResourceGroup> = BTreeMap::new();
    if api_docs.exists() {
        collect_recursive(&api_docs, &api_docs, &mut groups);
    }

    (project_name, groups.into_values().collect())
}

fn collect_recursive(api_docs: &Path, dir: &Path, groups: &mut BTreeMap<String, ResourceGroup>) {
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

// ─── Template helpers ─────────────────────────────────────────────────────────

fn render_template<T: Template>(template: T) -> axum::response::Response {
    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("<h1>Template error</h1><p>{e}</p>")),
    )
    .into_response()
}

fn error_html(msg: &str) -> String {
    let escaped = msg
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<!DOCTYPE html><html><head><title>Error · Trellis Preview</title></head>\
         <body style='font-family:sans-serif;padding:2rem'>\
         <h1>Error</h1><pre style='background:#fee2e2;padding:1rem;border-radius:6px'>{escaped}</pre>\
         <p><a href='/'>← Back to all endpoints</a></p></body></html>"
    )
}

// ─── Server entry point ───────────────────────────────────────────────────────

/// Start the web preview server.
pub async fn run(root: PathBuf, host: &str, port: u16, env: &str) -> Result<()> {
    let state = Arc::new(AppState {
        root,
        env: env.to_string(),
    });
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/spec/*path", get(spec_handler))
        .route("/exec/*path", post(exec_handler))
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

Fetches a user.

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
    fn index_template_renders() {
        let t = IndexTemplate {
            project_name: "Test API".to_string(),
            groups: vec![],
            spec_count: 0,
            version: "1.0.0",
        };
        let html = t.render().unwrap();
        assert!(html.contains("Test API"));
        assert!(html.contains("0 endpoints"));
    }

    #[test]
    fn spec_template_renders() {
        let t = SpecTemplate {
            title: "Get user".to_string(),
            method: "GET".to_string(),
            path: "/users/:id".to_string(),
            description: "Fetches a user.".to_string(),
            request: "GET {{baseUrl}}/users/:id".to_string(),
            expected_response: "HTTP/1.1 200 OK".to_string(),
            tests: None,
            rel_path: "users/get-user.md".to_string(),
            env: "dev".to_string(),
            version: "1.0.0",
        };
        let html = t.render().unwrap();
        assert!(html.contains("Get user"));
        assert!(html.contains("/users/:id"));
        assert!(html.contains("GET {{baseUrl}}/users/:id"));
    }
}
