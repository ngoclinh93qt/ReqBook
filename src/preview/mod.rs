//! Web preview server for the Reqbook API spec browser.
//! Serves a React SPA (embedded via rust-embed) + JSON API endpoints.

mod business;
mod handlers;
mod types;
mod workspace;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use axum::{
    extract::Request,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
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

#[derive(Default)]
pub struct PreviewOptions {
    pub pick_folder: Option<PickFolderFn>,
    pub write_token: Option<String>,
}

pub(super) struct AppState {
    pub(super) root: Arc<std::sync::RwLock<PathBuf>>,
    pub(super) env: String,
    pub(super) mock_entries: Option<Vec<crate::mock::MockEntry>>,
    pub(super) pick_folder: Option<PickFolderFn>,
    pub(super) write_token: Option<String>,
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
        .layer(from_fn_with_state(state.clone(), unsafe_request_guard))
        .with_state(state)
}

async fn unsafe_request_guard(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let is_unsafe = is_unsafe_method(req.method());
    if is_unsafe && !is_allowed_browser_write(req.headers()) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "write requests are only allowed from the local preview origin"
            })),
        )
            .into_response();
    }

    if is_unsafe && !has_valid_write_token(req.headers(), state.write_token.as_deref()) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "write requests require the active Reqbook desktop session"
            })),
        )
            .into_response();
    }

    let mut response = next.run(req).await;
    if !is_unsafe {
        if let Some(token) = state.write_token.as_deref() {
            set_write_token_cookie(response.headers_mut(), token);
        }
    }
    response
}

fn is_unsafe_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn is_allowed_browser_write(headers: &HeaderMap) -> bool {
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("cross-site"))
    {
        return false;
    }

    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return true;
    };
    let Ok(url) = reqwest::Url::parse(origin) else {
        return false;
    };
    matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
}

fn has_valid_write_token(headers: &HeaderMap, expected: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    if expected.is_empty() {
        return false;
    }
    if headers
        .get("x-rqb-write-token")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == expected)
    {
        return true;
    }
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .any(|part| {
            part.trim()
                .strip_prefix("rqb_write_token=")
                .is_some_and(|value| value == expected)
        })
}

fn set_write_token_cookie(headers: &mut HeaderMap, token: &str) {
    let cookie =
        format!("rqb_write_token={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400");
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        headers.insert(header::SET_COOKIE, value);
    }
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
    run_with_ready_options(
        root,
        host,
        port,
        env,
        mock,
        PreviewOptions {
            pick_folder,
            write_token: None,
        },
        on_ready,
    )
    .await
}

pub async fn run_with_ready_options<F>(
    root: Arc<std::sync::RwLock<PathBuf>>,
    host: &str,
    port: u16,
    env: &str,
    mock: bool,
    options: PreviewOptions,
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
        pick_folder: options.pick_folder,
        write_token: options.write_token,
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
    use business::{
        apply_runtime_overrides, collect_specs, doc_path, render_env_config, safe_rel_path,
    };
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

    #[test]
    fn safe_rel_path_rejects_traversal() {
        assert!(safe_rel_path("flows/../../outside.md").is_err());
        assert!(safe_rel_path("../api-docs/apis/users.md").is_err());
        assert!(safe_rel_path("/api-docs/apis/users.md").is_err());
        assert_eq!(
            safe_rel_path("flows/demo.md").unwrap(),
            PathBuf::from("flows/demo.md")
        );
    }

    #[test]
    fn doc_path_rejects_traversal_before_filesystem_access() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("api-docs/flows")).unwrap();
        let err = doc_path(dir.path(), "flows/../../outside.md").unwrap_err();
        assert!(err.contains(".."));
    }

    #[test]
    fn unsafe_request_guard_rejects_cross_site_browser_writes() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, "https://example.com".parse().unwrap());
        assert!(!is_allowed_browser_write(&headers));

        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, "http://127.0.0.1:8091".parse().unwrap());
        assert!(is_allowed_browser_write(&headers));

        let mut headers = HeaderMap::new();
        headers.insert("sec-fetch-site", "cross-site".parse().unwrap());
        assert!(!is_allowed_browser_write(&headers));
    }

    #[test]
    fn write_token_guard_is_optional_for_cli_preview() {
        let headers = HeaderMap::new();
        assert!(has_valid_write_token(&headers, None));
    }

    #[test]
    fn write_token_guard_accepts_session_cookie_or_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "theme=dark; rqb_write_token=session-123; other=value"
                .parse()
                .unwrap(),
        );
        assert!(has_valid_write_token(&headers, Some("session-123")));
        assert!(!has_valid_write_token(&headers, Some("wrong")));

        let mut headers = HeaderMap::new();
        headers.insert("x-rqb-write-token", "session-123".parse().unwrap());
        assert!(has_valid_write_token(&headers, Some("session-123")));
    }

    #[test]
    fn write_token_cookie_is_http_only_and_strict() {
        let mut headers = HeaderMap::new();
        set_write_token_cookie(&mut headers, "session-123");
        let cookie = headers
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .unwrap();
        assert!(cookie.contains("rqb_write_token=session-123"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
    }
}
