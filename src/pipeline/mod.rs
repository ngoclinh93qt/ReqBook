//! Pipeline orchestration.

use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json_path::JsonPath;
use thiserror::Error;

use crate::{
    engine::{execute, EngineError, ExecOpts, Execution},
    parser::{parse_endpoint, ParseError, Pipeline, PipelineStep},
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
    if pipeline.schema.parallel {
        run_parallel(pipeline, env, opts).await
    } else {
        run_sequential(pipeline, env, opts).await
    }
}

async fn run_sequential(
    pipeline: &Pipeline,
    env: &str,
    opts: PipelineOpts,
) -> Result<PipelineResult, PipelineError> {
    let mut captures: BTreeMap<String, String> = BTreeMap::new();
    let mut steps = Vec::new();
    let mut passed = true;

    for step in &pipeline.steps {
        let (step_captures, step_result) =
            execute_step(step, env, &opts.root, opts.exec.clone(), captures.clone()).await?;
        let step_passed = step_result
            .execution
            .as_ref()
            .map_or(step_result.error.is_none(), |e| e.diff.passed);
        if !step_passed {
            passed = false;
        }
        captures.extend(step_captures);
        steps.push(step_result);
        if !step_passed && !pipeline.schema.continue_on_error {
            break;
        }
    }

    Ok(PipelineResult {
        steps,
        captures,
        passed,
    })
}

async fn run_parallel(
    pipeline: &Pipeline,
    env: &str,
    opts: PipelineOpts,
) -> Result<PipelineResult, PipelineError> {
    let handles: Vec<_> = pipeline
        .steps
        .iter()
        .map(|step| {
            let root = opts.root.clone();
            let exec_opts = opts.exec.clone();
            let env = env.to_string();
            let step = step.clone();
            tokio::task::spawn(async move {
                execute_step(&step, &env, &root, exec_opts, BTreeMap::new()).await
            })
        })
        .collect();

    let mut steps = Vec::with_capacity(handles.len());
    let mut captures = BTreeMap::new();
    let mut passed = true;

    for handle in handles {
        let (step_captures, step_result) = handle.await.expect("pipeline step task panicked")?;
        let step_passed = step_result
            .execution
            .as_ref()
            .map_or(step_result.error.is_none(), |e| e.diff.passed);
        if !step_passed {
            passed = false;
        }
        captures.extend(step_captures);
        steps.push(step_result);
    }

    Ok(PipelineResult {
        steps,
        captures,
        passed,
    })
}

async fn execute_step(
    step: &PipelineStep,
    env: &str,
    root: &PathBuf,
    exec_opts: ExecOpts,
    initial_captures: BTreeMap<String, String>,
) -> Result<(BTreeMap<String, String>, StepResult), PipelineError> {
    let path = root.join(&step.endpoint);
    let path_display = path.display().to_string();
    let source = fs::read_to_string(&path).map_err(|source| PipelineError::ReadEndpoint {
        path: path_display.clone(),
        source,
    })?;
    let endpoint = parse_endpoint(&source, &path)?;
    let mut opts = exec_opts;
    for (key, value) in &initial_captures {
        opts.context
            .insert(SourceKind::Pipeline, key.clone(), value.clone());
    }

    match execute(&endpoint, env, opts).await {
        Ok(execution) => {
            let mut new_captures = BTreeMap::new();
            for capture in &step.capture {
                let value = capture_value(&execution, &capture.source).ok_or_else(|| {
                    PipelineError::Capture {
                        step: step.name.clone(),
                        capture: capture.source.clone(),
                    }
                })?;
                new_captures.insert(capture.name.clone(), value);
            }
            Ok((
                new_captures,
                StepResult {
                    name: step.name.clone(),
                    endpoint: step.endpoint.clone(),
                    execution: Some(execution),
                    error: None,
                },
            ))
        }
        Err(error) => Ok((
            BTreeMap::new(),
            StepResult {
                name: step.name.clone(),
                endpoint: step.endpoint.clone(),
                execution: None,
                error: Some(error.to_string()),
            },
        )),
    }
}

fn capture_value(execution: &Execution, source: &str) -> Option<String> {
    let response = execution.response.as_ref()?;
    let json: Value = serde_json::from_str(&response.body).unwrap_or(Value::Null);
    let wrapper = serde_json::json!({
        "response": {
            "status": response.status,
            "body": json.clone(),
        },
        "status": response.status,
        "body": json,
    });
    let expression = normalize_capture_expression(source)?;
    let json_path = JsonPath::parse(&expression).ok()?;
    let nodes = json_path.query(&wrapper);
    let first = nodes.first()?;
    match first {
        Value::String(value) => Some(value.clone()),
        other => Some(other.to_string()),
    }
}

fn normalize_capture_expression(source: &str) -> Option<String> {
    if source.starts_with('$') {
        return Some(source.to_string());
    }
    if let Some(rest) = source.strip_prefix("response.body") {
        return if rest.is_empty() {
            Some("$.response.body".to_string())
        } else if rest.starts_with('.') || rest.starts_with('[') {
            Some(format!("$.response.body{rest}"))
        } else {
            None
        };
    }
    if let Some(rest) = source.strip_prefix("response.") {
        return Some(format!("$.response.{rest}"));
    }
    None
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

    #[tokio::test]
    async fn captures_array_value_from_response_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!([{"id": 1, "name": "Ada"}])),
            )
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("users")).unwrap();
        fs::write(
            dir.path().join("users/get-users.md"),
            format!(
                r#"---
resource: users
protocol: http
method: GET
path: /users
version: 1
---
# Get users

Fetches users.

## Request

```http
GET {}/users
```

## Expected response

```http
HTTP/1.1 200 OK

[
  {{"id": 1}}
]
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

1. **Get users** -> `users/get-users.md`
   - Capture: `response.body[0].id` as `firstUserId`
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
        assert_eq!(result.captures["firstUserId"], "1");
    }
}
