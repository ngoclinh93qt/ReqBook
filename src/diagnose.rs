//! Compact execution diagnosis for coding agents.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    engine::{self, EngineError, ExecOpts, Execution, ResponseDiff},
    parser::{parse_endpoint, Endpoint},
    resolver::{mask, Context, SourceKind},
};

/// Agent-facing diagnosis for one endpoint execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnosis {
    /// Whether the endpoint passed.
    pub passed: bool,
    /// HTTP status when available.
    pub status: Option<u16>,
    /// Structured error type when failed.
    pub error_type: Option<String>,
    /// One-line summary.
    pub summary: String,
    /// Likely cause category.
    pub likely_cause: String,
    /// Next concrete action for an agent.
    pub next_action: String,
    /// Files or concepts to inspect next.
    pub inspect: Vec<String>,
    /// Safe verification commands.
    pub verify: Vec<String>,
    /// Compact diff details.
    pub diff: Value,
}

/// Run an endpoint and return an agent-facing diagnosis.
pub async fn diagnose_endpoint(
    spec_path: &Path,
    env: &str,
    context: Context,
    timeout_ms: Option<u64>,
    strict_assertions: bool,
) -> Result<Diagnosis> {
    let verify_vars = context.entries_for(SourceKind::Cli);
    let source = match std::fs::read_to_string(spec_path) {
        Ok(source) => source,
        Err(err) => {
            return Ok(Diagnosis {
                passed: false,
                status: None,
                error_type: Some("SPEC_PARSE_ERROR".to_string()),
                summary: format!("Cannot read spec file: {}", spec_path.display()),
                likely_cause: "Spec path is missing or unreadable.".to_string(),
                next_action: "Check the spec path before inspecting backend source.".to_string(),
                inspect: vec![spec_path.display().to_string()],
                verify: vec![format!(
                    "rqb validate {}",
                    api_docs_root(spec_path).display()
                )],
                diff: json!({ "message": err.to_string() }),
            });
        }
    };

    let endpoint = match parse_endpoint(&source, spec_path) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            return Ok(Diagnosis {
                passed: false,
                status: None,
                error_type: Some("SPEC_PARSE_ERROR".to_string()),
                summary: "Spec does not parse.".to_string(),
                likely_cause: "Markdown sections or frontmatter are invalid.".to_string(),
                next_action: "Fix the Reqbook spec structure, then run validate again.".to_string(),
                inspect: vec![spec_path.display().to_string()],
                verify: vec![format!(
                    "rqb validate {}",
                    api_docs_root(spec_path).display()
                )],
                diff: json!({ "message": err.to_string() }),
            });
        }
    };

    match engine::execute(
        &endpoint,
        env,
        ExecOpts {
            context,
            timeout_ms,
            dry_run: false,
            strict_assertions,
        },
    )
    .await
    {
        Ok(execution) => Ok(diagnose_execution_with_vars(
            spec_path,
            env,
            &endpoint,
            &execution,
            &verify_vars,
        )),
        Err(err) => Ok(diagnose_engine_error(
            spec_path,
            env,
            &endpoint,
            &err,
            &verify_vars,
        )),
    }
}

/// Build a diagnosis from an already captured execution.
pub fn diagnose_execution(
    spec_path: &Path,
    env: &str,
    endpoint: &Endpoint,
    execution: &Execution,
) -> Diagnosis {
    diagnose_execution_with_vars(spec_path, env, endpoint, execution, &[])
}

fn diagnose_execution_with_vars(
    spec_path: &Path,
    env: &str,
    endpoint: &Endpoint,
    execution: &Execution,
    verify_vars: &[(String, String)],
) -> Diagnosis {
    let status = execution.response.as_ref().map(|response| response.status);
    if execution.diff.passed {
        return Diagnosis {
            passed: true,
            status,
            error_type: None,
            summary: format!(
                "{} {} passed",
                endpoint.schema.method.as_str(),
                endpoint.schema.path
            ),
            likely_cause: "No failure detected.".to_string(),
            next_action: "No code or spec change is required for this endpoint.".to_string(),
            inspect: Vec::new(),
            verify: verify_commands(spec_path, env, verify_vars),
            diff: diff_value(&execution.diff),
        };
    }

    let error_type = if matches!(status, Some(401) | Some(403)) {
        "AUTH_FAILED"
    } else {
        "CONTRACT_MISMATCH"
    };

    let documented_error = status.and_then(|status| documented_error_status(endpoint, status));
    let (likely_cause, next_action) = if error_type == "AUTH_FAILED" {
        (
            "Request reached an auth failure path.".to_string(),
            "Check auth variables and Authorization header before changing backend code or expected response.".to_string(),
        )
    } else if let Some(error_status) = documented_error {
        (
            format!("API returned documented error response HTTP {error_status}."),
            "Inspect request variables/body/auth first; the backend may be correct and the test input may be invalid.".to_string(),
        )
    } else if execution.diff.status.is_some() {
        (
            "Actual status differs from the success contract.".to_string(),
            "Inspect the route handler and the spec expected status; update code or spec only after deciding which behavior is intended.".to_string(),
        )
    } else if !execution.diff.assertions.is_empty() {
        (
            "Structured assertion failed.".to_string(),
            "Inspect the assertion path and the response field it targets; fix implementation or assertion intent.".to_string(),
        )
    } else if execution.diff.body.is_some() {
        (
            "Response body shape differs from the expected contract.".to_string(),
            "Inspect the handler response DTO/serializer and the spec expected body fields."
                .to_string(),
        )
    } else if !execution.diff.headers.is_empty() {
        (
            "Response headers differ from the expected contract.".to_string(),
            "Inspect response headers or relax expected headers if they are not part of the stable contract.".to_string(),
        )
    } else {
        (
            "Response did not match the expected contract.".to_string(),
            "Run rqb exec with JSON output if more detail is needed, then fix code or spec intentionally.".to_string(),
        )
    };

    Diagnosis {
        passed: false,
        status,
        error_type: Some(error_type.to_string()),
        summary: format!(
            "{} {} failed: {error_type}",
            endpoint.schema.method.as_str(),
            endpoint.schema.path
        ),
        likely_cause,
        next_action,
        inspect: inspect_targets(spec_path, endpoint),
        verify: verify_commands(spec_path, env, verify_vars),
        diff: diff_value(&execution.diff),
    }
}

fn diagnose_engine_error(
    spec_path: &Path,
    env: &str,
    endpoint: &Endpoint,
    err: &EngineError,
    verify_vars: &[(String, String)],
) -> Diagnosis {
    let (error_type, likely_cause, next_action) = match err {
        EngineError::UnsupportedProtocol { .. } => (
            "UNSUPPORTED_PROTOCOL",
            "Spec uses a protocol Reqbook does not execute yet.",
            "Use protocol: http for executable specs, or keep this spec as documentation only.",
        ),
        EngineError::Resolve { .. } => (
            "VAR_MISSING",
            "One or more request variables could not be resolved.",
            "Check _shared/env.md, .env.local, RQB_* variables, or pass --var key=value.",
        ),
        EngineError::InvalidRequest { .. } => (
            "VALIDATION_ERROR",
            "The ## Request HTTP block is malformed.",
            "Fix the request block before inspecting backend source.",
        ),
        EngineError::InvalidExpected { .. } => (
            "VALIDATION_ERROR",
            "The ## Expected response HTTP block is malformed.",
            "Fix the expected response block before inspecting backend source.",
        ),
        EngineError::Network { .. } => (
            "NETWORK_ERROR",
            "The request could not reach the API service.",
            "Check baseUrl, running server, DNS, VPN, firewall, and port before changing code.",
        ),
        EngineError::Http { .. } => (
            "VALIDATION_ERROR",
            "Reqbook could not build the HTTP request metadata.",
            "Check method, URL, and headers in the spec.",
        ),
        EngineError::UnsupportedEnvironment { .. } => (
            "VALIDATION_ERROR",
            "The selected environment is not allowed by endpoint frontmatter.",
            "Use an allowed env or update env: [...] after review.",
        ),
    };

    Diagnosis {
        passed: false,
        status: None,
        error_type: Some(error_type.to_string()),
        summary: format!(
            "{} {} failed before response: {error_type}",
            endpoint.schema.method.as_str(),
            endpoint.schema.path
        ),
        likely_cause: likely_cause.to_string(),
        next_action: next_action.to_string(),
        inspect: vec![spec_path.display().to_string()],
        verify: verify_commands(spec_path, env, verify_vars),
        diff: json!({ "message": err.to_string() }),
    }
}

fn diff_value(diff: &ResponseDiff) -> Value {
    json!({
        "status": diff.status,
        "headers": diff.headers,
        "body": diff.body,
        "assertions": diff.assertions,
    })
}

fn documented_error_status(endpoint: &Endpoint, status: u16) -> Option<u16> {
    endpoint.error_responses.iter().find_map(|block| {
        let parsed = http_status(block)?;
        (parsed == status).then_some(parsed)
    })
}

fn http_status(block: &str) -> Option<u16> {
    block
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
}

fn inspect_targets(spec_path: &Path, endpoint: &Endpoint) -> Vec<String> {
    vec![
        format!(
            "backend route for {} {}",
            endpoint.schema.method.as_str(),
            endpoint.schema.path
        ),
        format!("{} ## Expected response", spec_path.display()),
        format!("{} ## Error responses", spec_path.display()),
    ]
}

fn verify_commands(spec_path: &Path, env: &str, vars: &[(String, String)]) -> Vec<String> {
    let mut exec = format!("rqb exec {} --env {}", spec_path.display(), env);
    for (key, value) in vars {
        let assignment = mask(&format!("{key}={value}"));
        exec.push_str(" --var ");
        exec.push_str(&shell_arg(&assignment));
    }

    vec![
        format!("rqb validate {}", api_docs_root(spec_path).display()),
        exec,
    ]
}

fn shell_arg(value: &str) -> String {
    if !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '=' | '@')
        })
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn api_docs_root(spec_path: &Path) -> std::path::PathBuf {
    spec_path
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "api-docs"))
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("api-docs"))
}

#[cfg(test)]
mod tests {
    use crate::engine::{CapturedRequest, CapturedResponse};

    use super::*;

    fn endpoint() -> Endpoint {
        let source = r#"---
resource: refunds
protocol: http
method: POST
path: /refunds/quote
version: 1
---
# Create refund quote

Create a quote.

## Request

```http
POST {{baseUrl}}/refunds/quote

{"orderId":"ord_1"}
```

## Expected response

```http
HTTP/1.1 201 Created

{"quoteId":"rfq_1"}
```

## Error responses

```http
HTTP/1.1 422 Unprocessable Entity

{"error":"validation_error"}
```
"#;
        parse_endpoint(source, "api-docs/apis/refunds/post-refund-quote.md").unwrap()
    }

    #[test]
    fn diagnosis_points_to_documented_error_path() {
        let endpoint = endpoint();
        let execution = Execution {
            request: CapturedRequest {
                method: "POST".to_string(),
                url: "http://localhost/refunds/quote".to_string(),
                headers: Default::default(),
                body: "{}".to_string(),
            },
            response: Some(CapturedResponse {
                status: 422,
                headers: Default::default(),
                body: "{\"error\":\"validation_error\"}".to_string(),
                size: 28,
            }),
            duration_ms: 10,
            diff: ResponseDiff {
                passed: false,
                status: Some("expected 201, got 422".to_string()),
                ..ResponseDiff::default()
            },
            assertion_results: Vec::new(),
        };

        let diagnosis = diagnose_execution(
            Path::new("api-docs/apis/refunds/post-refund-quote.md"),
            "dev",
            &endpoint,
            &execution,
        );
        assert_eq!(diagnosis.error_type.as_deref(), Some("CONTRACT_MISMATCH"));
        assert!(diagnosis.likely_cause.contains("documented error response"));
        assert!(diagnosis
            .next_action
            .contains("request variables/body/auth"));
    }

    #[test]
    fn verify_command_preserves_cli_vars_and_masks_auth_values() {
        let commands = verify_commands(
            Path::new("api-docs/apis/users/get-user.md"),
            "dev",
            &[
                ("userId".to_string(), "usr_123".to_string()),
                ("authToken".to_string(), "secret-token".to_string()),
            ],
        );
        assert_eq!(
            commands[1],
            "rqb exec api-docs/apis/users/get-user.md --env dev --var userId=usr_123 --var 'authToken=****'"
        );
    }
}
