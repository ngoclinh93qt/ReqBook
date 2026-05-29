//! Import a raw `curl` command as a Trellis endpoint spec.
//!
//! Handles multi-line `curl` commands pasted from browser DevTools →
//! "Copy as cURL (bash)" and converts them into clean Trellis endpoint files.

use std::collections::BTreeMap;

use anyhow::{bail, Result};

use super::{resource_slug, ImportedEndpoint};

/// Parsed fields from a curl command   used by the UI "Paste cURL" feature.
pub struct ParsedCurl {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
}

/// Parse a curl command into discrete fields for ad-hoc requests.
/// Keeps all headers (no filtering) so the user sees exactly what Chrome copied.
/// Used by the REST `POST /api/parse-curl` endpoint (New Request panel).
pub fn parse_to_fields(input: &str) -> Result<ParsedCurl> {
    let req = parse_curl(input.trim())?;
    Ok(ParsedCurl {
        method: req.method,
        url: req.url,
        headers: req.headers.into_iter().collect(),
        body: req.body,
    })
}

/// Browser-specific headers that add noise to API specs.
const BROWSER_HEADERS: &[&str] = &[
    "user-agent",
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "sec-ch-ua-platform",
    "sec-fetch-dest",
    "sec-fetch-mode",
    "sec-fetch-site",
    "pragma",
    "cache-control",
    "accept-language",
    "accept-encoding",
    "priority",
    "te",
    "connection",
    "host",
];

/// Parse a raw `curl` command and return a `(collection_name, endpoints)` tuple,
/// matching the interface used by all other importers.
pub fn import(input: &str) -> Result<(String, Vec<ImportedEndpoint>)> {
    let req = parse_curl(input)?;
    let ep = to_endpoint(req)?;
    let name = format!("cURL import ({})", ep.path);
    Ok((name, vec![ep]))
}

// ─── Internal request representation ─────────────────────────────────────────

struct CurlRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

// ─── Tokenizer ────────────────────────────────────────────────────────────────

/// Tokenize a shell-style string, respecting single/double quotes and
/// backslash-newline line continuations.
fn tokenize(input: &str) -> Result<Vec<String>> {
    // Join backslash-newline continuations first.
    let joined = input.replace("\\\r\n", " ").replace("\\\n", " ");

    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = joined.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                // Single-quoted: content is literal until the next apostrophe.
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => current.push(ch),
                        None => bail!("unterminated single quote in curl command"),
                    }
                }
            }
            '"' => {
                // Double-quoted: backslash can escape the next character.
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => {
                            if let Some(next) = chars.next() {
                                current.push(next);
                            }
                        }
                        Some(ch) => current.push(ch),
                        None => bail!("unterminated double quote in curl command"),
                    }
                }
            }
            '\\' => {
                // Unquoted backslash: escape the next character (skip newlines).
                if let Some(next) = chars.next() {
                    if next != '\n' && next != '\r' {
                        current.push(next);
                    }
                }
            }
            c if c.is_ascii_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

// ─── Parser ───────────────────────────────────────────────────────────────────

fn parse_curl(input: &str) -> Result<CurlRequest> {
    let tokens = tokenize(input.trim())?;
    if tokens.is_empty() {
        bail!("empty input");
    }

    let mut url: Option<String> = None;
    let mut method: Option<String> = None;
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body: Option<String> = None;

    let mut i = 0usize;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "curl" => {}

            "-X" | "--request" => {
                i += 1;
                if let Some(m) = tokens.get(i) {
                    method = Some(m.to_uppercase());
                }
            }

            "-H" | "--header" => {
                i += 1;
                if let Some(h) = tokens.get(i) {
                    if let Some((key, val)) = h.split_once(':') {
                        headers.push((key.trim().to_string(), val.trim().to_string()));
                    }
                }
            }

            "-b" | "--cookie" => {
                // Treat cookie string as a Cookie header so filter_headers can strip it.
                i += 1;
                if let Some(cookie) = tokens.get(i) {
                    headers.push(("Cookie".to_string(), cookie.clone()));
                }
            }

            "-u" | "--user" => {
                // Encode as Basic Authorization header.
                i += 1;
                if let Some(creds) = tokens.get(i) {
                    let encoded = base64_encode(creds.as_bytes());
                    headers.push(("Authorization".to_string(), format!("Basic {encoded}")));
                }
            }

            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-urlencode" => {
                i += 1;
                if let Some(data) = tokens.get(i) {
                    body = Some(data.clone());
                }
            }

            "--url" => {
                i += 1;
                if let Some(u) = tokens.get(i) {
                    url = Some(u.clone());
                }
            }

            // Flags that take no argument   silently skip.
            "--compressed"
            | "--silent"
            | "-s"
            | "--location"
            | "-L"
            | "--fail"
            | "-f"
            | "--show-error"
            | "--no-progress-meter"
            | "--http1.1"
            | "--http2"
            | "--insecure"
            | "-k"
            | "--include"
            | "-i"
            | "-v"
            | "--verbose" => {}

            t if !t.starts_with('-') => {
                // First non-flag token after "curl" is the URL.
                if url.is_none() {
                    url = Some(t.to_string());
                }
            }

            // Unknown flags with optional single-token argument: skip the flag only.
            // (We don't blindly consume the next token to avoid eating a URL.)
            _ => {}
        }
        i += 1;
    }

    let url = url.ok_or_else(|| {
        anyhow::anyhow!(
            "no URL found in curl command\n\
             Fix: make sure the input starts with `curl <url>` or `curl --url <url>`."
        )
    })?;

    // Infer method: POST if a body is present and no explicit method was given.
    let method = method.unwrap_or_else(|| {
        if body.is_some() {
            "POST".to_string()
        } else {
            "GET".to_string()
        }
    });

    Ok(CurlRequest {
        method,
        url,
        headers,
        body,
    })
}

// ─── Converter ────────────────────────────────────────────────────────────────

fn to_endpoint(req: CurlRequest) -> Result<ImportedEndpoint> {
    let (host, path, query) = split_url(&req.url);
    let resource = resource_from_path(&path);
    let title = title_from_path(&req.method, &path);
    let (api_headers, cookie_stripped, browser_skipped) = filter_headers(&req.headers);

    // Build the `## Request` block.
    let mut request_lines = vec![format!("{} {{{{baseUrl}}}}{path}", req.method)];
    for (key, val) in &api_headers {
        request_lines.push(format!("{key}: {val}"));
    }
    if let Some(body) = &req.body {
        request_lines.push(String::new());
        request_lines.push(body.clone());
    }
    let request = request_lines.join("\n");

    // Build the `## Notes` section with import context.
    let mut notes: Vec<String> = Vec::new();
    notes.push(format!("Imported from: `{}`", req.url));
    if !host.is_empty() {
        notes.push(format!(
            "Set `baseUrl` to `{host}` in `api-docs/_shared/env.md` (or `.env.local`)."
        ));
    }
    if !query.is_empty() {
        notes.push(format!("Original query string: `{query}`"));
    }
    if browser_skipped > 0 {
        notes.push(format!(
            "{browser_skipped} browser-specific header(s) removed \
             (user-agent, sec-*, pragma, cache-control, …). \
             Add them back if your API actually requires them."
        ));
    }
    if cookie_stripped {
        notes.push(
            "Session cookies were stripped. \
             Store secrets in `.env.local` and inject them with `--var Cookie=...`."
                .to_string(),
        );
    }

    Ok(ImportedEndpoint {
        resource: resource_slug(&resource),
        method: req.method,
        path,
        title,
        description: String::new(),
        request,
        expected_response: "HTTP/1.1 200 OK\nContent-Type: application/json\n\n{}".to_string(),
        tests: None,
        notes: Some(notes.join("\n\n")),
        tags: vec![],
    })
}

/// Split a URL into `(origin, path, query)`.
/// e.g. `https://api.example.com/users?page=1` → `("https://api.example.com", "/users", "page=1")`
fn split_url(url: &str) -> (String, String, String) {
    let url = url.trim().trim_matches('\'').trim_matches('"');

    // Strip fragment.
    let no_fragment = url.split('#').next().unwrap_or(url);

    // Split query string.
    let (base, query) = if let Some(idx) = no_fragment.find('?') {
        (&no_fragment[..idx], no_fragment[idx + 1..].to_string())
    } else {
        (no_fragment, String::new())
    };

    // Extract scheme://host and path.
    if let Some(scheme_end) = base.find("://") {
        let scheme = &base[..scheme_end];
        let after_scheme = &base[scheme_end + 3..];
        if let Some(path_start) = after_scheme.find('/') {
            let host = &after_scheme[..path_start];
            let path = &after_scheme[path_start..];
            return (format!("{scheme}://{host}"), path.to_string(), query);
        }
        // No path component (e.g. `https://api.example.com`).
        return (format!("{scheme}://{after_scheme}"), "/".to_string(), query);
    }

    // No scheme   treat entire string as a path.
    (String::new(), format!("/{base}"), query)
}

/// Return the first meaningful path segment as the resource name.
fn resource_from_path(path: &str) -> String {
    path.trim_start_matches('/')
        .split('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("resources")
        .to_string()
}

/// Generate a human-readable title from the HTTP method and URL path.
pub(crate) fn title_from_path(method: &str, path: &str) -> String {
    let segments: Vec<&str> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    // Separate static segments from path-parameter segments.
    let static_segs: Vec<&str> = segments
        .iter()
        .filter(|s| !s.starts_with(':'))
        .cloned()
        .collect();
    let has_id_param = segments.iter().any(|s| s.starts_with(':'));

    let base = static_segs
        .last()
        .copied()
        .unwrap_or("resource")
        .replace(['-', '_'], " ");

    match method {
        "GET" if has_id_param => format!("{} by id", sentence_case(&base)),
        "GET" => sentence_case(&base),
        "POST" => format!("Create {}", base.to_lowercase()),
        "PUT" | "PATCH" => format!("Update {}", base.to_lowercase()),
        "DELETE" => format!("Delete {}", base.to_lowercase()),
        m => format!("{} {}", sentence_case(m), base.to_lowercase()),
    }
}

/// Separate API-relevant headers from browser noise and cookies.
/// Returns `(api_headers, cookie_was_stripped, browser_header_count_removed)`.
fn filter_headers(headers: &[(String, String)]) -> (Vec<(String, String)>, bool, usize) {
    let mut api_headers = Vec::new();
    let mut cookie_stripped = false;
    let mut browser_count = 0usize;

    for (key, val) in headers {
        let lower = key.to_lowercase();
        if lower == "cookie" {
            cookie_stripped = true;
            browser_count += 1;
        } else if BROWSER_HEADERS.contains(&lower.as_str()) {
            browser_count += 1;
        } else {
            api_headers.push((key.clone(), val.clone()));
        }
    }

    (api_headers, cookie_stripped, browser_count)
}

/// Minimal Base64 encoder (no external dep required).
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(CHARS[(b0 >> 2) & 63] as char);
        out.push(CHARS[((b0 << 4) | (b1 >> 4)) & 63] as char);
        out.push(if chunk.len() > 1 {
            CHARS[((b1 << 2) | (b2 >> 6)) & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[b2 & 63] as char
        } else {
            '='
        });
    }
    out
}

fn sentence_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let up: String = first.to_uppercase().collect();
            up + chars.as_str()
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_single_quoted() {
        let tokens =
            tokenize("curl 'https://example.com/api' -H 'Accept: application/json'").unwrap();
        assert_eq!(
            tokens,
            vec![
                "curl",
                "https://example.com/api",
                "-H",
                "Accept: application/json"
            ]
        );
    }

    #[test]
    fn tokenize_multiline_continuation() {
        let tokens =
            tokenize("curl 'https://example.com' \\\n  -H 'Accept: application/json'").unwrap();
        assert_eq!(tokens[0], "curl");
        assert!(tokens.contains(&"Accept: application/json".to_string()));
    }

    #[test]
    fn parse_simple_get() {
        let (name, eps) =
            import("curl 'https://api.example.com/users' -H 'accept: application/json'").unwrap();
        assert!(name.contains("/users"));
        let ep = &eps[0];
        assert_eq!(ep.method, "GET");
        assert_eq!(ep.path, "/users");
        assert_eq!(ep.resource, "users");
        assert!(ep.request.contains("{{baseUrl}}/users"));
        assert!(ep.request.contains("accept: application/json"));
    }

    #[test]
    fn parse_post_with_body() {
        let (_, eps) = import(
            r#"curl 'https://api.example.com/posts' -X POST \
  -H 'content-type: application/json' \
  -d '{"title":"hello"}'
"#,
        )
        .unwrap();
        let ep = &eps[0];
        assert_eq!(ep.method, "POST");
        assert!(ep.request.contains(r#"{"title":"hello"}"#));
    }

    #[test]
    fn infers_post_from_body() {
        let (_, eps) = import("curl 'https://api.example.com/posts' -d '{\"x\":1}'").unwrap();
        assert_eq!(eps[0].method, "POST");
    }

    #[test]
    fn cookies_stripped() {
        let (_, eps) =
            import("curl 'https://api.example.com/data' -b 'session=abc123; token=xyz'").unwrap();
        let ep = &eps[0];
        assert!(!ep.request.contains("Cookie"));
        let notes = ep.notes.as_deref().unwrap_or("");
        assert!(
            notes.contains("cookies were stripped")
                || notes.contains("Cookies were stripped")
                || notes.contains("Session cookies")
        );
    }

    #[test]
    fn browser_headers_removed() {
        let (_, eps) = import(
            "curl 'https://api.example.com/data' \
             -H 'sec-ch-ua: Chrome' \
             -H 'accept: application/json'",
        )
        .unwrap();
        let ep = &eps[0];
        assert!(!ep.request.contains("sec-ch-ua"));
        assert!(ep.request.contains("accept: application/json"));
    }

    #[test]
    fn generated_spec_passes_validate() {
        use crate::importer::render_endpoint;
        use crate::parser::parse_endpoint;

        let (_, eps) =
            import("curl 'https://api.example.com/users/123' -H 'accept: application/json'")
                .unwrap();
        let md = render_endpoint(&eps[0]);
        let path = std::path::PathBuf::from("api-docs/users/get-users-by-id.md");
        parse_endpoint(&md, &path).expect("generated spec must be valid");
    }

    #[test]
    fn real_world_github_curl() {
        // Simulated copy-as-curl from GitHub DevTools (stripped down).
        let curl = r#"curl 'https://github.com/renovatebot/renovate/latest-commit' \
  -H 'accept: application/json' \
  -H 'github-is-react: true' \
  -H 'github-verified-fetch: true' \
  -H 'sec-ch-ua: "Chromium";v="148"' \
  -H 'sec-fetch-dest: empty' \
  -H 'x-requested-with: XMLHttpRequest' \
  -b '_octo=GH1.1; user_session=secret123'"#;

        let (_, eps) = import(curl).unwrap();
        let ep = &eps[0];

        assert_eq!(ep.method, "GET");
        assert_eq!(ep.path, "/renovatebot/renovate/latest-commit");
        assert_eq!(ep.resource, "renovatebot");
        // Browser headers must be gone.
        assert!(!ep.request.contains("sec-ch-ua"));
        assert!(!ep.request.contains("sec-fetch-dest"));
        // API-specific headers must be present.
        assert!(ep.request.contains("github-is-react"));
        assert!(ep.request.contains("github-verified-fetch"));
        assert!(ep.request.contains("x-requested-with"));
        // Cookies must be stripped.
        assert!(!ep.request.contains("Cookie"));
    }

    #[test]
    fn split_url_extracts_host_and_path() {
        let (host, path, query) = split_url("https://api.example.com/users?page=1");
        assert_eq!(host, "https://api.example.com");
        assert_eq!(path, "/users");
        assert_eq!(query, "page=1");
    }

    #[test]
    fn title_get_collection() {
        assert_eq!(title_from_path("GET", "/users"), "Users");
    }

    #[test]
    fn title_get_by_id() {
        assert_eq!(title_from_path("GET", "/users/:id"), "Users by id");
    }

    #[test]
    fn title_post() {
        assert_eq!(title_from_path("POST", "/posts"), "Create posts");
    }

    #[test]
    fn base64_hello() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }
}
