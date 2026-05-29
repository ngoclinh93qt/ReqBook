//! Shared logic for ad-hoc (inline, file-free) HTTP requests.
//!
//! Used by both the `mad request` CLI command and the `POST /api/request`
//! REST endpoint so the construction and execution paths are identical.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};

use crate::parser::{AuthMode, Backoff, Endpoint, EndpointSchema, HttpMethod, Protocol, RetryPolicy};

/// Parameters for an ad-hoc request (shared between CLI and REST).
#[derive(Debug, Clone)]
pub struct AdHocParams {
    pub method: String,
    pub url: String,
    /// `Name: Value` pairs.
    pub headers: BTreeMap<String, String>,
    /// Raw body string.
    pub body: Option<String>,
    pub env: String,
}

/// Build an in-memory `Endpoint` from `AdHocParams`   no file I/O.
pub fn build_endpoint(params: &AdHocParams) -> Result<Endpoint> {
    let method = parse_method(&params.method)?;
    let path = url_path(&params.url);

    let mut req = format!("{} {}", params.method.to_uppercase(), params.url);
    for (name, value) in &params.headers {
        req.push('\n');
        req.push_str(name);
        req.push_str(": ");
        req.push_str(value);
    }
    if let Some(body) = &params.body {
        req.push_str("\n\n");
        req.push_str(body);
    }

    Ok(Endpoint {
        source: None,
        schema: EndpointSchema {
            resource: "scratch".to_string(),
            protocol: Protocol::Http,
            method,
            path,
            tags: vec!["adhoc".to_string()],
            version: 1,
            env: vec![params.env.clone()],
            auth: Some(AuthMode::None),
            timeout: None,
            retry: Some(RetryPolicy {
                attempts: 0,
                backoff: Backoff::Fixed,
            }),
        },
        title: format!("{} {}", params.method.to_uppercase(), params.url),
        description: "Ad-hoc request".to_string(),
        request: req,
        expected_response: "HTTP/1.1 200 OK".to_string(),
        tests: None,
        notes: None,
        assertions: Vec::new(),
    })
}

/// Render a minimal scratch spec file (omits empty sections to reduce tokens).
pub fn render_scratch_spec(params: &AdHocParams, response_block: &str) -> String {
    let method_upper = params.method.to_uppercase();
    let path = url_path(&params.url);

    let mut req_block = format!("{} {}", method_upper, params.url);
    for (name, value) in &params.headers {
        req_block.push('\n');
        req_block.push_str(name);
        req_block.push_str(": ");
        req_block.push_str(value);
    }
    if let Some(body) = &params.body {
        req_block.push_str("\n\n");
        req_block.push_str(body);
    }

    let mut out = format!(
        "---\nresource: scratch\nmethod: {method_upper}\npath: {path}\nversion: 1\n---\n\n# {method_upper} {path}\n\n## Request\n\n```http\n{req_block}\n```\n"
    );
    if !response_block.is_empty() {
        out.push_str("\n## Expected response\n\n```http\n");
        out.push_str(response_block);
        out.push_str("\n```\n");
    }
    out
}

/// Auto-generate a scratch filename from timestamp + method + url slug.
pub fn scratch_filename(method: &str, url: &str) -> String {
    let now = chrono_timestamp();
    let slug = slug_from_url(url);
    format!("{now}-{}-{slug}.md", method.to_lowercase())
}

/// Write a spec to the scratch workspace and return the path.
pub fn save_to_scratch(params: &AdHocParams, response_block: &str) -> Result<PathBuf> {
    let dir = crate::workspace::ensure_scratch_workspace()
        .map_err(|e| anyhow::anyhow!("scratch workspace: {e}"))?;
    let filename = scratch_filename(&params.method, &params.url);
    let path = dir.join("apis/scratch").join(filename);
    let content = render_scratch_spec(params, response_block);
    std::fs::write(&path, content)?;
    Ok(path)
}

/// Write a spec to a user-specified path (--save).
pub fn save_to_path(dest: &Path, params: &AdHocParams, response_block: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let content = render_scratch_spec(params, response_block);
    std::fs::write(dest, content)?;
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_method(s: &str) -> Result<HttpMethod> {
    match s.to_uppercase().as_str() {
        "GET" => Ok(HttpMethod::Get),
        "POST" => Ok(HttpMethod::Post),
        "PUT" => Ok(HttpMethod::Put),
        "PATCH" => Ok(HttpMethod::Patch),
        "DELETE" => Ok(HttpMethod::Delete),
        "HEAD" => Ok(HttpMethod::Head),
        "OPTIONS" => Ok(HttpMethod::Options),
        other => bail!("unsupported HTTP method: {other}\nFix: use GET, POST, PUT, PATCH, DELETE, HEAD, or OPTIONS."),
    }
}

fn url_path(url: &str) -> String {
    // Strip scheme + host to get just the path portion.
    let without_scheme = url
        .find("://")
        .map(|i| &url[i + 3..])
        .unwrap_or(url);
    let path_start = without_scheme.find('/').unwrap_or(without_scheme.len());
    let path = &without_scheme[path_start..];
    if path.is_empty() { "/".to_string() } else { path.to_string() }
}

fn slug_from_url(url: &str) -> String {
    let path = url_path(url);
    let slug: String = path
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if slug.is_empty() { "request".to_string() } else { slug }
}

fn chrono_timestamp() -> String {
    // Simple timestamp without chrono dep: use std::time.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Format as YYYYMMDDTHHMMSS (approximate, UTC).
    let s = secs;
    let seconds = s % 60;
    let minutes = (s / 60) % 60;
    let hours = (s / 3600) % 24;
    let days_since_epoch = s / 86400;
    // Approximate date from days since epoch (1970-01-01).
    let (year, month, day) = days_to_ymd(days_since_epoch);
    format!("{year:04}{month:02}{day:02}T{hours:02}{minutes:02}{seconds:02}")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let dy = if leap { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        year += 1;
    }
    let leap = is_leap(year);
    let months = [31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for dm in months {
        if days < dm { break; }
        days -= dm;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
