//! HTTP execution and response comparison.

use std::{collections::BTreeMap, str::FromStr, time::Instant};

use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::time::{sleep, Duration};

use crate::{
    parser::{Assertion, AssertionOp, Endpoint, HttpMethod, Protocol},
    resolver::{mask, resolve, Context, ResolveError},
};

/// HTTP client type used by Trellis.
pub type Client = reqwest::Client;

/// Execution options.
#[derive(Debug, Clone, Default)]
pub struct ExecOpts {
    /// Variable resolution context.
    pub context: Context,
    /// Optional timeout override in milliseconds.
    pub timeout_ms: Option<u64>,
    /// If true, resolve and return the request without sending it.
    pub dry_run: bool,
}

/// Result of evaluating a single assertion rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssertionResult {
    /// Human-readable rule description.
    pub rule: String,
    /// Whether the assertion passed.
    pub passed: bool,
    /// Details about the result.
    pub message: String,
}

/// Captured execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Execution {
    /// Captured request.
    pub request: CapturedRequest,
    /// Captured response, absent for dry runs.
    pub response: Option<CapturedResponse>,
    /// Duration in milliseconds.
    pub duration_ms: u128,
    /// Comparison result against expected response.
    pub diff: ResponseDiff,
    /// Results of structured assertions.
    #[serde(default)]
    pub assertion_results: Vec<AssertionResult>,
}

/// Captured HTTP request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapturedRequest {
    /// HTTP method.
    pub method: String,
    /// URL.
    pub url: String,
    /// Masked request headers.
    pub headers: BTreeMap<String, String>,
    /// Masked request body.
    pub body: String,
}

/// Captured HTTP response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapturedResponse {
    /// Status code.
    pub status: u16,
    /// Masked response headers.
    pub headers: BTreeMap<String, String>,
    /// Masked response body.
    pub body: String,
    /// Response body size in bytes.
    pub size: usize,
}

/// Response comparison result.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ResponseDiff {
    /// Whether actual response matched expected response.
    pub passed: bool,
    /// Status mismatch, if any.
    pub status: Option<String>,
    /// Header mismatches.
    pub headers: Vec<String>,
    /// Body mismatch, if any.
    pub body: Option<String>,
}

/// Engine errors.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Unsupported protocol.
    #[error("{path}: unsupported protocol {protocol}\nFix: use `protocol: http`; ws and sse are reserved for future versions.")]
    UnsupportedProtocol {
        /// Source path.
        path: String,
        /// Protocol name.
        protocol: String,
    },
    /// Variable resolution failed.
    #[error("{path}: {source}")]
    Resolve {
        /// Source path.
        path: String,
        /// Source error.
        #[source]
        source: ResolveError,
    },
    /// Invalid request block.
    #[error("{path}: invalid http request block: {message}\nFix: use `METHOD URL`, optional headers, blank line, then optional body.")]
    InvalidRequest {
        /// Source path.
        path: String,
        /// Details.
        message: String,
    },
    /// Invalid expected response block.
    #[error("{path}: invalid expected response block: {message}\nFix: use `HTTP/1.1 <status> <reason>`, optional headers, blank line, then optional body.")]
    InvalidExpected {
        /// Source path.
        path: String,
        /// Details.
        message: String,
    },
    /// Network request failed.
    #[error("{path}: network error: {source}\nFix: check base URL, connectivity, DNS, firewall, and VPN.")]
    Network {
        /// Source path.
        path: String,
        /// Source error.
        #[source]
        source: reqwest::Error,
    },
    /// HTTP metadata could not be built.
    #[error("{path}: invalid HTTP metadata: {message}\nFix: check method, URL, and headers.")]
    Http {
        /// Source path.
        path: String,
        /// Details.
        message: String,
    },
}

/// Execute an endpoint with a default reqwest client.
pub async fn execute(
    endpoint: &Endpoint,
    _env: &str,
    opts: ExecOpts,
) -> Result<Execution, EngineError> {
    let client = Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|source| EngineError::Network {
            path: source_path(endpoint),
            source,
        })?;
    execute_with_client(&client, endpoint, opts).await
}

/// Execute an endpoint with an injected client.
pub async fn execute_with_client(
    client: &Client,
    endpoint: &Endpoint,
    opts: ExecOpts,
) -> Result<Execution, EngineError> {
    if endpoint.schema.protocol != Protocol::Http {
        return Err(EngineError::UnsupportedProtocol {
            path: source_path(endpoint),
            protocol: format!("{:?}", endpoint.schema.protocol),
        });
    }

    let path = source_path(endpoint);
    let resolved_request =
        resolve(&endpoint.request, &opts.context).map_err(|source| EngineError::Resolve {
            path: path.clone(),
            source,
        })?;
    let resolved_request =
        resolve_path_params(&resolved_request, &opts.context).map_err(|source| {
            EngineError::Resolve {
                path: path.clone(),
                source,
            }
        })?;
    let expected = parse_expected(&endpoint.expected_response).map_err(|message| {
        EngineError::InvalidExpected {
            path: path.clone(),
            message,
        }
    })?;
    let request =
        parse_request(&resolved_request).map_err(|message| EngineError::InvalidRequest {
            path: path.clone(),
            message,
        })?;
    let captured_request = request.to_captured();

    if opts.dry_run {
        return Ok(Execution {
            request: captured_request,
            response: None,
            duration_ms: 0,
            diff: ResponseDiff::default(),
            assertion_results: Vec::new(),
        });
    }

    let timeout = opts
        .timeout_ms
        .or(endpoint.schema.timeout)
        .map(Duration::from_millis);
    let attempts = endpoint
        .schema
        .retry
        .as_ref()
        .map_or(0, |retry| retry.attempts);
    let started = Instant::now();
    let mut last_error = None;

    for attempt in 0..=attempts {
        let mut builder = client.request(request.method.clone(), request.url.clone());
        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }
        builder = builder.headers(request.headers.clone());
        if !request.body.is_empty() {
            builder = builder.body(request.body.clone());
        }

        match builder.send().await {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|source| EngineError::Network {
                        path: path.clone(),
                        source,
                    })?;
                let body = String::from_utf8_lossy(&bytes).to_string();
                if status.is_server_error() && attempt < attempts {
                    sleep(Duration::from_millis(25 * u64::from(attempt + 1))).await;
                    continue;
                }
                let captured_response = CapturedResponse {
                    status: status.as_u16(),
                    headers: headers_to_map(&headers),
                    body: mask(&body),
                    size: bytes.len(),
                };
                let diff = diff_response(&expected, status, &headers, &body);
                let assertion_results =
                    evaluate_assertions(&endpoint.assertions, status, &headers, &body);
                return Ok(Execution {
                    request: captured_request,
                    response: Some(captured_response),
                    duration_ms: started.elapsed().as_millis(),
                    diff,
                    assertion_results,
                });
            }
            Err(source) => {
                last_error = Some(source);
                if attempt < attempts {
                    sleep(Duration::from_millis(25 * u64::from(attempt + 1))).await;
                }
            }
        }
    }

    Err(EngineError::Network {
        path,
        source: last_error.expect("network attempt has an error when all attempts fail"),
    })
}

#[derive(Debug, Clone)]
struct ParsedRequest {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: String,
}

impl ParsedRequest {
    fn to_captured(&self) -> CapturedRequest {
        CapturedRequest {
            method: self.method.to_string(),
            url: self.url.to_string(),
            headers: headers_to_map(&self.headers),
            body: mask(&self.body),
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedExpected {
    status: StatusCode,
    headers: BTreeMap<String, String>,
    body: String,
}

fn parse_request(source: &str) -> Result<ParsedRequest, String> {
    let mut parts = source.splitn(2, "\n\n");
    let head = parts.next().unwrap_or_default();
    let body = parts.next().unwrap_or_default().to_string();
    let mut lines = head.lines();
    let request_line = lines.next().ok_or("missing request line")?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().ok_or("missing method")?;
    let url = request_parts.next().ok_or("missing URL")?;
    if request_parts.next().is_some() {
        return Err("request line has too many fields".to_string());
    }
    let method = Method::from_bytes(method.as_bytes()).map_err(|err| err.to_string())?;
    let url = Url::parse(url).map_err(|err| err.to_string())?;
    let mut headers = HeaderMap::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Err(format!("invalid header `{line}`"));
        };
        headers.insert(
            HeaderName::from_str(name.trim()).map_err(|err| err.to_string())?,
            HeaderValue::from_str(value.trim()).map_err(|err| err.to_string())?,
        );
    }
    Ok(ParsedRequest {
        method,
        url,
        headers,
        body,
    })
}

fn resolve_path_params(source: &str, ctx: &Context) -> Result<String, ResolveError> {
    let mut parts = source.splitn(2, '\n');
    let request_line = parts.next().unwrap_or_default();
    let rest = parts.next();
    let mut request_parts = request_line.split_whitespace();
    let Some(method) = request_parts.next() else {
        return Ok(source.to_string());
    };
    let Some(url) = request_parts.next() else {
        return Ok(source.to_string());
    };
    if request_parts.next().is_some() {
        return Ok(source.to_string());
    }

    let param_re = regex::Regex::new(r":([A-Za-z_][A-Za-z0-9_]*)").expect("valid path param regex");
    let mut resolved_url = String::with_capacity(url.len());
    let mut last = 0;
    for caps in param_re.captures_iter(url) {
        let mat = caps.get(0).expect("whole match exists");
        resolved_url.push_str(&url[last..mat.start()]);
        let name = caps.get(1).expect("name match exists").as_str();
        let value = ctx.get(name).ok_or_else(|| ResolveError::MissingVariable {
            name: name.to_string(),
            env_name: to_env_name(name),
        })?;
        resolved_url.push_str(value);
        last = mat.end();
    }
    resolved_url.push_str(&url[last..]);

    let mut out = format!("{method} {resolved_url}");
    if let Some(rest) = rest {
        out.push('\n');
        out.push_str(rest);
    }
    Ok(out)
}

fn to_env_name(name: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && idx > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

fn parse_expected(source: &str) -> Result<ParsedExpected, String> {
    let mut parts = source.splitn(2, "\n\n");
    let head = parts.next().unwrap_or_default();
    let body = parts.next().unwrap_or_default().to_string();
    let mut lines = head.lines();
    let status_line = lines.next().ok_or("missing status line")?;
    let mut status_parts = status_line.split_whitespace();
    let _version = status_parts.next().ok_or("missing HTTP version")?;
    let status = status_parts.next().ok_or("missing status code")?;
    let status = status.parse::<u16>().map_err(|err| err.to_string())?;
    let status = StatusCode::from_u16(status).map_err(|err| err.to_string())?;
    let mut headers = BTreeMap::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Err(format!("invalid header `{line}`"));
        };
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }
    Ok(ParsedExpected {
        status,
        headers,
        body,
    })
}

fn diff_response(
    expected: &ParsedExpected,
    status: StatusCode,
    headers: &HeaderMap,
    body: &str,
) -> ResponseDiff {
    let mut diff = ResponseDiff {
        passed: true,
        status: None,
        headers: Vec::new(),
        body: None,
    };
    if expected.status != status {
        diff.passed = false;
        diff.status = Some(format!(
            "expected {}, got {}",
            expected.status.as_u16(),
            status.as_u16()
        ));
    }

    let actual_headers = headers_to_map(headers);
    for (name, expected_value) in &expected.headers {
        match actual_headers.get(name) {
            Some(actual) if actual.contains(expected_value) => {}
            Some(actual) => {
                diff.passed = false;
                diff.headers.push(format!(
                    "{name}: expected `{expected_value}`, got `{actual}`"
                ));
            }
            None => {
                diff.passed = false;
                diff.headers.push(format!("{name}: missing"));
            }
        }
    }

    if !expected.body.trim().is_empty() {
        let body_ok = match (
            serde_json::from_str::<Value>(&expected.body),
            serde_json::from_str::<Value>(body),
        ) {
            (Ok(expected_json), Ok(actual_json)) => {
                json_shape_matches(&expected_json, &actual_json)
            }
            _ => expected.body.trim() == body.trim(),
        };
        if !body_ok {
            diff.passed = false;
            diff.body = Some("response body did not match expected shape".to_string());
        }
    }
    diff
}

fn json_shape_matches(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .is_some_and(|actual_value| json_shape_matches(value, actual_value))
        }),
        (Value::Array(expected), Value::Array(actual)) => {
            expected.is_empty()
                || actual
                    .first()
                    .is_some_and(|first| json_shape_matches(&expected[0], first))
        }
        _ => true,
    }
}

fn headers_to_map(headers: &HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_ascii_lowercase(),
                mask(value.to_str().unwrap_or("<non-utf8>")),
            )
        })
        .collect()
}

fn source_path(endpoint: &Endpoint) -> String {
    endpoint
        .source
        .clone()
        .unwrap_or_else(|| "<memory>".to_string())
}

/// Evaluate structured assertion rules against an actual HTTP response.
fn evaluate_assertions(
    assertions: &[Assertion],
    status: http::StatusCode,
    headers: &HeaderMap,
    body: &str,
) -> Vec<AssertionResult> {
    if assertions.is_empty() {
        return Vec::new();
    }
    let body_json: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let header_map = headers_to_map(headers);

    assertions
        .iter()
        .map(|a| evaluate_one(a, status, &header_map, body, &body_json))
        .collect()
}

fn evaluate_one(
    assertion: &Assertion,
    status: http::StatusCode,
    headers: &std::collections::BTreeMap<String, String>,
    body: &str,
    body_json: &Value,
) -> AssertionResult {
    let rule = format!(
        "{}: {} {}",
        assertion.path,
        serde_json::to_string(&assertion.op).unwrap_or_default().trim_matches('"'),
        assertion.value.as_deref().unwrap_or(""),
    )
    .trim()
    .to_string();

    // Resolve the actual value for the path.
    let actual: Option<String> = resolve_assertion_path(&assertion.path, status, headers, body, body_json);

    let (passed, message) = match &assertion.op {
        AssertionOp::Exists => {
            let ok = actual.as_deref().map(|v| v != "null").unwrap_or(false);
            (ok, if ok { "exists".to_string() } else { "expected to exist but was absent or null".to_string() })
        }
        AssertionOp::Equals => {
            let expected = assertion.value.as_deref().unwrap_or("");
            match &actual {
                Some(v) if v == expected => (true, format!("= {expected}")),
                Some(v) => (false, format!("expected `{expected}`, got `{v}`")),
                None => (false, format!("expected `{expected}`, but path was absent")),
            }
        }
        AssertionOp::Contains => {
            let expected = assertion.value.as_deref().unwrap_or("");
            match &actual {
                Some(v) if v.contains(expected) => (true, format!("contains `{expected}`")),
                Some(v) => (false, format!("expected to contain `{expected}`, got `{v}`")),
                None => (false, format!("expected to contain `{expected}`, but path was absent")),
            }
        }
        AssertionOp::Matches => {
            let pattern = assertion.value.as_deref().unwrap_or("");
            match regex::Regex::new(pattern) {
                Ok(re) => match &actual {
                    Some(v) if re.is_match(v) => (true, format!("matches `{pattern}`")),
                    Some(v) => (false, format!("expected to match `{pattern}`, got `{v}`")),
                    None => (false, format!("expected to match `{pattern}`, but path was absent")),
                },
                Err(e) => (false, format!("invalid regex `{pattern}`: {e}")),
            }
        }
        AssertionOp::In => {
            let list: Vec<&str> = assertion
                .value
                .as_deref()
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .collect();
            match &actual {
                Some(v) if list.contains(&v.as_str()) => (true, format!("in [{}]", list.join(", "))),
                Some(v) => (false, format!("expected one of [{}], got `{v}`", list.join(", "))),
                None => (false, format!("expected one of [{}], but path was absent", list.join(", "))),
            }
        }
    };

    AssertionResult { rule, passed, message }
}

fn resolve_assertion_path(
    path: &str,
    status: http::StatusCode,
    headers: &std::collections::BTreeMap<String, String>,
    body: &str,
    body_json: &Value,
) -> Option<String> {
    if path == "status" {
        return Some(status.as_u16().to_string());
    }
    if let Some(header_name) = path.strip_prefix("headers.") {
        return headers.get(header_name).cloned();
    }
    if path == "body" {
        return Some(body.to_string());
    }
    if let Some(json_path) = path.strip_prefix("body.") {
        return navigate_json(body_json, json_path);
    }
    None
}

fn navigate_json(value: &Value, path: &str) -> Option<String> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    match current {
        Value::Null => Some("null".to_string()),
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

impl From<HttpMethod> for Method {
    fn from(value: HttpMethod) -> Self {
        Method::from_bytes(value.as_str().as_bytes()).expect("known method is valid")
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::{
        parser::parse_endpoint,
        resolver::{Context, SourceKind},
    };

    use super::*;

    fn endpoint(url: &str) -> Endpoint {
        let doc = format!(
            r#"---
resource: anything
protocol: http
method: GET
path: /get
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Get anything

Fetches anything.

## Request

```http
GET {url}/get
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{{"ok": true}}
```
"#
        );
        parse_endpoint(&doc, "endpoint.md").unwrap()
    }

    #[tokio::test]
    async fn executes_real_request_against_mock_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let execution = execute(&endpoint(&server.uri()), "dev", ExecOpts::default())
            .await
            .unwrap();
        assert_eq!(execution.response.unwrap().status, 200);
        assert!(execution.diff.passed);
    }

    #[tokio::test]
    async fn dry_run_does_not_send() {
        let server = MockServer::start().await;
        let execution = execute(
            &endpoint(&server.uri()),
            "dev",
            ExecOpts {
                dry_run: true,
                ..ExecOpts::default()
            },
        )
        .await
        .unwrap();
        assert!(execution.response.is_none());
    }

    #[tokio::test]
    async fn resolves_request_vars() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;
        let mut endpoint = endpoint("{{baseUrl}}");
        let mut context = Context::default();
        context.insert(SourceKind::Cli, "baseUrl", server.uri());
        let execution = execute(
            &endpoint,
            "dev",
            ExecOpts {
                context,
                ..ExecOpts::default()
            },
        )
        .await
        .unwrap();
        endpoint.source = None;
        assert_eq!(execution.response.unwrap().status, 200);
    }

    #[tokio::test]
    async fn resolves_colon_path_params_from_context() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;
        let mut endpoint = endpoint("{{baseUrl}}");
        endpoint.request = "GET {{baseUrl}}/users/:userId\nAccept: application/json".to_string();
        let mut context = Context::default();
        context.insert(SourceKind::Cli, "baseUrl", server.uri());
        context.insert(SourceKind::Cli, "userId", "1");
        let execution = execute(
            &endpoint,
            "dev",
            ExecOpts {
                context,
                ..ExecOpts::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(execution.request.url, format!("{}/users/1", server.uri()));
        assert_eq!(execution.response.unwrap().status, 200);
    }

    #[tokio::test]
    async fn rejects_unsupported_protocol() {
        let mut endpoint = endpoint("https://example.com");
        endpoint.schema.protocol = crate::parser::Protocol::Ws;
        let err = execute(&endpoint, "dev", ExecOpts::default())
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::UnsupportedProtocol { .. }));
    }

    #[tokio::test]
    async fn rejects_invalid_request_block() {
        let mut endpoint = endpoint("https://example.com");
        endpoint.request = "GET".to_string();
        let err = execute(&endpoint, "dev", ExecOpts::default())
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::InvalidRequest { .. }));
    }

    #[tokio::test]
    async fn rejects_invalid_expected_block() {
        let mut endpoint = endpoint("https://example.com");
        endpoint.expected_response = "not-http".to_string();
        let err = execute(&endpoint, "dev", ExecOpts::default())
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::InvalidExpected { .. }));
    }

    #[test]
    fn detects_status_header_and_body_diff() {
        let expected = parse_expected(
            "HTTP/1.1 201 Created\nContent-Type: application/json\nX-Required: yes\n\n{\"id\": 1}",
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        let diff = diff_response(
            &expected,
            StatusCode::OK,
            &headers,
            "{\"name\":\"missing id\"}",
        );
        assert!(!diff.passed);
        assert!(diff.status.is_some());
        assert_eq!(diff.headers.len(), 1);
        assert!(diff.body.is_some());
    }

    #[test]
    fn parses_request_with_body() {
        let request = parse_request(
            "POST https://example.com/users\nContent-Type: application/json\n\n{\"name\":\"A\"}",
        )
        .unwrap();
        assert_eq!(request.method, Method::POST);
        assert_eq!(request.body, "{\"name\":\"A\"}");
    }
}
