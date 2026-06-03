//! Web preview server for the Reqbook API spec browser.
//! Serves a React SPA (embedded via rust-embed) + JSON API endpoints.

mod business;
mod handlers;
mod types;
mod workspace;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use axum::{
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use mime_guess::from_path;
use owo_colors::OwoColorize;
use rust_embed::RustEmbed;

use crate::mock::collect_entries;

use axum::{extract::State, response::Json};

use handlers::{
    adhoc_request_handler, api_index_handler, api_spec_handler, exec_handler, flow_handler,
    flows_handler, get_variables_handler, import_curl_handler, import_project_handler,
    parse_curl_fields_handler, run_flow_handler, save_flow_handler, save_spec_handler,
    save_variables_handler, scan_project_handler, validate_handler,
};
use workspace::{
    git_branches_handler, git_checkout_handler, workspace_all_handler, workspace_create_handler,
    workspace_current_handler, workspace_open_handler, workspace_recent_handler,
};

// ─── Static assets ────────────────────────────────────────────────────────────

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
        None => match WebAssets::get("index.html") {
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
        },
    }
}

// ─── Shared state ─────────────────────────────────────────────────────────────

/// Callback injected by Tauri to show a native folder picker dialog.
/// Returns a oneshot receiver that resolves to the selected path (or None if cancelled).
pub type PickFolderFn =
    Arc<dyn Fn() -> tokio::sync::oneshot::Receiver<Option<String>> + Send + Sync>;

pub(super) struct AppState {
    pub(super) root: Arc<std::sync::RwLock<PathBuf>>,
    pub(super) env: String,
    pub(super) mock_entries: Option<Vec<crate::mock::MockEntry>>,
    pub(super) pick_folder: Option<PickFolderFn>,
}

impl AppState {
    pub(super) fn current_root(&self) -> PathBuf {
        self.root.read().unwrap().clone()
    }

    pub(super) fn mock_mode(&self) -> bool {
        self.mock_entries.is_some()
    }
}

// ─── Folder picker handler ────────────────────────────────────────────────────

async fn pick_folder_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let Some(ref pick_fn) = state.pick_folder else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "folder picker not available"})),
        )
            .into_response();
    };
    let rx = pick_fn();
    match rx.await {
        Ok(path) => Json(serde_json::json!({"path": path})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ─── Router ───────────────────────────────────────────────────────────────────

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
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
        .route("/api/workspace/current", get(workspace_current_handler))
        .route("/api/workspace/recent", get(workspace_recent_handler))
        .route("/api/workspace/all", get(workspace_all_handler))
        .route("/api/workspace/open", post(workspace_open_handler))
        .route("/api/workspace/create", post(workspace_create_handler))
        .route("/api/git/branches", get(git_branches_handler))
        .route("/api/git/checkout", post(git_checkout_handler))
        .route("/api/pick-folder", get(pick_folder_handler))
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
        None,
        |_| {},
    )
    .await
}

/// Start the axum server with a pre-created shared workspace root.
///
/// Fires `on_ready(actual_port)` once the TCP listener is bound.
/// Pass `pick_folder` to enable the native OS folder picker at `GET /api/pick-folder`.
pub async fn run_with_ready<F>(
    root: Arc<std::sync::RwLock<PathBuf>>,
    host: &str,
    port: u16,
    env: &str,
    mock: bool,
    pick_folder: Option<PickFolderFn>,
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
        pick_folder,
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
    use std::fs;

    use super::*;
    use crate::parser::EnvConfig;
    use business::{apply_runtime_overrides, collect_specs, render_env_config};
    use types::RuntimeOverrides;

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
        vars.insert("label".to_string(), "key: value".to_string());
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
