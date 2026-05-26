//! Pipeline orchestration.

use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json_path::JsonPath;
use thiserror::Error;

use crate::{
    engine::{execute, EngineError, ExecOpts, Execution},
    parser::{parse_endpoint, ParseError, Pipeline},
    resolver::SourceKind,
};

/// Pipeline execution options.
#[derive(Debug, Clone, Default)]
pub struct PipelineOpts {
    /// Root `api-docs` directory.
    pub root: PathBuf,
    /// Base execution options.
    pub exec: ExecOpts,
}

/// Pipeline execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    /// Step results in execution order.
    pub steps: Vec<StepResult>,
    /// Captured values propagated across steps.
    pub captures: BTreeMap<String, String>,
    /// Whether all required steps passed.
    pub passed: bool,
}

/// One step result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step name.
    pub name: String,
    /// Endpoint file.
    pub endpoint: String,
    /// Execution result when the step ran.
    pub execution: Option<Execution>,
    /// Error message when the step failed.
    pub error: Option<String>,
}

/// Pipeline errors.
#[derive(Debug, Error)]
pub enum PipelineError {
    /// Endpoint could not be read.
    #[error("{path}: failed to read endpoint: {source}\nFix: check the pipeline endpoint path.")]
    ReadEndpoint {
        /// File path.
        path: String,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Endpoint parse failed.
    #[error(transparent)]
    Parse(#[from] ParseError),
    /// Endpoint execution failed.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// Capture failed.
    #[error("{step}: capture `{capture}` failed\nFix: check the JSONPath and response body.")]
    Capture {
        /// Step name.
        step: String,
        /// Capture expression.
        capture: String,
    },
}

/// Run a pipeline.
pub async fn run(
    pipeline: &Pipeline,
    env: &str,
    opts: PipelineOpts,
) -> Result<PipelineResult, PipelineError> {
    let mut captures: BTreeMap<String, String> = BTreeMap::new();
    let mut steps = Vec::new();
    let mut passed = true;

    for step in &pipeline.steps {
        let path = opts.root.join(&step.endpoint);
        let path_display = path.display().to_string();
        let source = fs::read_to_string(&path).map_err(|source| PipelineError::ReadEndpoint {
            path: path_display.clone(),
            source,
        })?;
        let endpoint = parse_endpoint(&source, &path)?;
        let mut exec_opts = opts.exec.clone();
        for (key, value) in &captures {
            exec_opts
                .context
                .insert(SourceKind::Pipeline, key.clone(), value.clone());
        }

        match execute(&endpoint, env, exec_opts).await {
            Ok(execution) => {
                for capture in &step.capture {
                    let value = capture_value(&execution, &capture.source).ok_or_else(|| {
                        PipelineError::Capture {
                            step: step.name.clone(),
                            capture: capture.source.clone(),
                        }
                    })?;
                    captures.insert(capture.name.clone(), value);
                }
                let step_passed = execution.diff.passed;
                if !step_passed {
                    passed = false;
                }
                steps.push(StepResult {
                    name: step.name.clone(),
                    endpoint: step.endpoint.clone(),
                    execution: Some(execution),
                    error: None,
                });
                if !step_passed && !pipeline.schema.continue_on_error {
                    break;
                }
            }
            Err(error) => {
                passed = false;
                let error_string = error.to_string();
                steps.push(StepResult {
                    name: step.name.clone(),
                    endpoint: step.endpoint.clone(),
                    execution: None,
                    error: Some(error_string),
                });
                if !pipeline.schema.continue_on_error {
                    break;
                }
            }
        }
    }

    Ok(PipelineResult {
        steps,
        captures,
        passed,
    })
}

fn capture_value(execution: &Execution, source: &str) -> Option<String> {
    let response = execution.response.as_ref()?;
    if source == "response.status" {
        return Some(response.status.to_string());
    }
    let path = source.strip_prefix("response.body.")?;
    let json: Value = serde_json::from_str(&response.body).ok()?;
    let json_path = JsonPath::parse(&format!("$.{path}")).ok()?;
    let nodes = json_path.query(&json);
    let first = nodes.first()?;
    match first {
        Value::String(value) => Some(value.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::parser::parse_pipeline;

    use super::*;

    #[tokio::test]
    async fn runs_pipeline_and_captures_value() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "u1"})))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("users")).unwrap();
        fs::write(
            dir.path().join("users/get-user.md"),
            format!(
                r#"---
resource: users
protocol: http
method: GET
path: /user
version: 1
---
# Get user

Fetches user.

## Request

```http
GET {}/user
```

## Expected response

```http
HTTP/1.1 200 OK

{{"id": "u1"}}
```
"#,
                server.uri()
            ),
        )
        .unwrap();
        let pipeline = parse_pipeline(
            r#"---
type: pipeline
name: demo
---
# Demo

## Steps

1. **Get user** -> `users/get-user.md`
   - Capture: `response.body.id` as `userId`
"#,
            "pipeline.md",
        )
        .unwrap();
        let result = run(
            &pipeline,
            "dev",
            PipelineOpts {
                root: dir.path().to_path_buf(),
                exec: ExecOpts::default(),
            },
        )
        .await
        .unwrap();
        assert_eq!(result.captures.get("userId").unwrap(), "u1");
    }

    #[tokio::test]
    async fn reports_missing_endpoint_path() {
        let dir = tempdir().unwrap();
        let pipeline = parse_pipeline(
            r#"---
type: pipeline
name: demo
---
# Demo

## Steps

1. **Missing** -> `users/missing.md`
"#,
            "pipeline.md",
        )
        .unwrap();
        let err = run(
            &pipeline,
            "dev",
            PipelineOpts {
                root: dir.path().to_path_buf(),
                exec: ExecOpts::default(),
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, PipelineError::ReadEndpoint { .. }));
    }

    #[tokio::test]
    async fn captures_response_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("health")).unwrap();
        fs::write(
            dir.path().join("health/get-health.md"),
            format!(
                r#"---
resource: health
protocol: http
method: GET
path: /health
version: 1
---
# Get health

Checks health.

## Request

```http
GET {}/health
```

## Expected response

```http
HTTP/1.1 204 No Content
```
"#,
                server.uri()
            ),
        )
        .unwrap();
        let pipeline = parse_pipeline(
            r#"---
type: pipeline
name: demo
---
# Demo

## Steps

1. **Health** -> `health/get-health.md`
   - Capture: `response.status` as `statusCode`
"#,
            "pipeline.md",
        )
        .unwrap();
        let result = run(
            &pipeline,
            "dev",
            PipelineOpts {
                root: dir.path().to_path_buf(),
                exec: ExecOpts::default(),
            },
        )
        .await
        .unwrap();
        assert_eq!(result.captures["statusCode"], "204");
    }
}
