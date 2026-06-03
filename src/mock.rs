//! Mock HTTP server that replays recorded responses from Reqbook spec files.
//!
//! `rqb mock [--port 4001] [--dir api-docs/]`
//!
//! Walks all endpoint `.md` files under `<dir>/apis/`, extracts the
//! `## Expected response` block from each spec, and serves those responses
//! over HTTP. Useful for frontend development and CI contract testing without
//! a running backend.
//!
//! Route matching uses a simple linear scan with `:param` wildcard support,
//! identical to the syntax used in Reqbook spec paths.

use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context as _, Result};
use axum::{
    body::Bytes,
    extract::Request,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use owo_colors::OwoColorize;

use crate::parser::parse_endpoint;

// ─── Recorded mock entry ──────────────────────────────────────────────────────

/// A single recorded response, built once at startup from the spec's
/// `## Expected response` block.
#[derive(Debug, Clone)]
pub struct MockEntry {
    /// HTTP method in uppercase, e.g. "GET".
    pub method: String,
    /// Path pattern with `:param` segments, e.g. "/users/:userId".
    pub pattern: String,
    pub status: StatusCode,
    pub content_type: String,
    pub body: Bytes,
}

// ─── Path matching ────────────────────────────────────────────────────────────

/// Returns `true` if `pattern` matches `actual` where `:word` segments are
/// wildcards that match any non-empty path segment.
///
/// Examples:
/// - `/users/:id` matches `/users/42`   → `true`
/// - `/users/:id` matches `/users/`     → `false` (empty segment)
/// - `/users`     matches `/users/42`   → `false` (different segment count)
pub fn path_matches(pattern: &str, actual: &str) -> bool {
    let pp: Vec<&str> = pattern.split('/').collect();
    let ap: Vec<&str> = actual.split('/').collect();
    if pp.len() != ap.len() {
        return false;
    }
    pp.iter()
        .zip(ap.iter())
        .all(|(p, a)| p.starts_with(':') && !a.is_empty() || *p == *a)
}

// ─── Response parsing ─────────────────────────────────────────────────────────

/// Parse the raw text of an `## Expected response` block into status,
/// content-type, and body.
///
/// Expected format:
/// ```text
/// HTTP/1.1 200 OK
/// Content-Type: application/json
///
/// {"id": 1}
/// ```
///
/// Returns `None` if the block is empty or the status line is malformed.
fn parse_expected_response(raw: &str) -> Option<(StatusCode, String, Bytes)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // Split header section from body on the first blank line.
    let (head, body) = raw.split_once("\n\n").unwrap_or((raw, ""));

    let mut lines = head.lines();

    // "HTTP/1.1 200 OK"  →  status 200
    let status_code: u16 = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let status = StatusCode::from_u16(status_code).ok()?;

    // Extract Content-Type if present; default to application/json.
    let mut content_type = "application/json".to_string();
    for line in lines {
        if line.to_lowercase().starts_with("content-type:") {
            content_type = line["content-type:".len()..].trim().to_string();
        }
    }

    Some((status, content_type, Bytes::from(body.to_string())))
}

// ─── Route collection ──────────────────────────────────────────────────────────

pub fn collect_entries(dir: &std::path::Path) -> Result<Vec<MockEntry>> {
    // Mirror the layout logic from preview.rs: prefer `<dir>/apis/` when it
    // exists; otherwise fall back to `<dir>` itself (legacy flat layout where
    // resource folders live directly under api-docs/).
    let apis_dir = dir.join("apis");
    let root = if apis_dir.exists() {
        apis_dir
    } else {
        dir.to_path_buf()
    };
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    collect_dir(&root, &mut entries)?;
    Ok(entries)
}

fn collect_dir(dir: &std::path::Path, out: &mut Vec<MockEntry>) -> Result<()> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    paths.sort();

    for p in paths {
        if p.is_dir() {
            // Skip non-endpoint directories regardless of layout depth.
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if matches!(name.as_ref(), "_shared" | "flows" | "pipelines") {
                continue;
            }
            collect_dir(&p, out)?;
        } else if p.extension().is_some_and(|e| e == "md") {
            let Ok(source) = std::fs::read_to_string(&p) else {
                continue;
            };
            let Ok(endpoint) = parse_endpoint(&source, &p) else {
                continue;
            };
            let Some((status, content_type, body)) =
                parse_expected_response(&endpoint.expected_response)
            else {
                continue;
            };
            out.push(MockEntry {
                method: endpoint.schema.method.as_str().to_string(),
                pattern: endpoint.schema.path.clone(),
                status,
                content_type,
                body,
            });
        }
    }
    Ok(())
}

// ─── Shared state ─────────────────────────────────────────────────────────────

struct MockState {
    /// All loaded entries. Duplicate (method, pattern) pairs are warned at
    /// startup; only the first entry encountered is kept.
    entries: Vec<MockEntry>,
    latency_ms: Option<u64>,
}

// ─── Request handler ──────────────────────────────────────────────────────────

async fn mock_handler(
    axum::extract::State(state): axum::extract::State<Arc<MockState>>,
    req: Request,
) -> Response {
    let method = req.method().as_str().to_uppercase();
    let path = req.uri().path().to_string();

    // Find the first entry whose (method, pattern) matches (method, path).
    let entry = state
        .entries
        .iter()
        .find(|e| e.method == method && path_matches(&e.pattern, &path));

    match entry {
        Some(e) => {
            if let Some(ms) = state.latency_ms {
                tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
            }
            let mut headers = HeaderMap::new();
            if let Ok(val) = HeaderValue::from_str(&e.content_type) {
                headers.insert(header::CONTENT_TYPE, val);
            }
            (e.status, headers, e.body.clone()).into_response()
        }
        None => {
            let body = serde_json::json!({
                "error": format!("no mock for {method} {path}")
            })
            .to_string();
            (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "application/json")],
                body,
            )
                .into_response()
        }
    }
}

// ─── Public entry point ────────────────────────────────────────────────────────

/// Start the mock server.
///
/// - `dir`           project root containing `apis/` sub-directory (default: `api-docs/`).
/// - `port`          TCP port to bind (default: `4001`).
/// - `latency_ms`    optional artificial response delay in milliseconds.
pub async fn run_mock_server(dir: PathBuf, port: u16, latency_ms: Option<u64>) -> Result<()> {
    let raw_entries = collect_entries(&dir)?;

    // Deduplicate: warn on conflicts and keep first occurrence.
    let mut seen: HashMap<(String, String), ()> = HashMap::new();
    let mut entries: Vec<MockEntry> = Vec::new();
    for entry in raw_entries {
        let key = (entry.method.clone(), entry.pattern.clone());
        if seen.contains_key(&key) {
            eprintln!(
                "  {} duplicate mock {} {}   skipping",
                "!".yellow(),
                entry.method,
                entry.pattern
            );
            continue;
        }
        seen.insert(key, ());
        entries.push(entry);
    }

    println!(
        "{} Mock server   {} route(s) from {}",
        "→".cyan(),
        entries.len(),
        dir.display()
    );
    for e in &entries {
        println!(
            "  {} {}  {}",
            e.method.cyan(),
            e.pattern,
            e.status.as_u16().to_string().dimmed()
        );
    }

    let state = Arc::new(MockState {
        entries,
        latency_ms,
    });
    let app = Router::new().fallback(mock_handler).with_state(state);

    let addr: SocketAddr = format!("127.0.0.1:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to port {port}"))?;

    println!("{} Listening on http://127.0.0.1:{port}", "✓".green());
    axum::serve(listener, app).await?;
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── path_matches ──

    #[test]
    fn exact_path_matches() {
        assert!(path_matches("/users", "/users"));
        assert!(path_matches("/", "/"));
    }

    #[test]
    fn param_segment_matches_any_value() {
        assert!(path_matches("/users/:id", "/users/42"));
        assert!(path_matches("/users/:id", "/users/abc-123"));
        assert!(path_matches(
            "/orgs/:org/repos/:repo",
            "/orgs/acme/repos/api"
        ));
    }

    #[test]
    fn param_segment_does_not_match_empty() {
        assert!(!path_matches("/users/:id", "/users/"));
    }

    #[test]
    fn different_segment_counts_do_not_match() {
        assert!(!path_matches("/users/:id", "/users/42/posts"));
        assert!(!path_matches("/users", "/users/42"));
    }

    #[test]
    fn literal_mismatch_does_not_match() {
        assert!(!path_matches("/users/:id", "/posts/42"));
    }

    // ── parse_expected_response ──

    #[test]
    fn parses_full_response_block() {
        let raw = "HTTP/1.1 200 OK\nContent-Type: application/json\n\n{\"id\":1}";
        let (status, ct, body) = parse_expected_response(raw).unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ct, "application/json");
        assert_eq!(body, Bytes::from("{\"id\":1}"));
    }

    #[test]
    fn missing_content_type_defaults_to_json() {
        let raw = "HTTP/1.1 201 Created\n\n{\"ok\":true}";
        let (status, ct, _) = parse_expected_response(raw).unwrap();
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(ct, "application/json");
    }

    #[test]
    fn parses_404_response() {
        let raw = "HTTP/1.1 404 Not Found\n\n{\"error\":\"not found\"}";
        let (status, _, _) = parse_expected_response(raw).unwrap();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn parses_500_response() {
        let raw = "HTTP/1.1 500 Internal Server Error\n\n{}";
        let (status, _, _) = parse_expected_response(raw).unwrap();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn empty_block_returns_none() {
        assert!(parse_expected_response("").is_none());
        assert!(parse_expected_response("   ").is_none());
    }

    #[test]
    fn malformed_status_line_returns_none() {
        // No status code integer
        assert!(parse_expected_response("HTTP/1.1 OK\n\n{}").is_none());
        // Completely empty
        assert!(parse_expected_response("\n\n{}").is_none());
    }

    // ── collect_entries ──

    #[test]
    fn collect_from_nonexistent_apis_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let entries = collect_entries(dir.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn collect_from_example_project() {
        let dir = std::path::Path::new("examples/jsonplaceholder/api-docs");
        if !dir.exists() {
            return; // skip if examples not present
        }
        let entries = collect_entries(dir).unwrap();
        assert!(
            !entries.is_empty(),
            "expected at least one mock entry from example project"
        );
        // All entries should have valid methods
        for e in &entries {
            assert!(
                ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]
                    .contains(&e.method.as_str()),
                "unexpected method: {}",
                e.method
            );
        }
    }
}
