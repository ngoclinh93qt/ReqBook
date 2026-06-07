//! Pipeline orchestration.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

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
    /// Capture names must be unique for dependency analysis.
    #[error("capture `{capture}` is produced by multiple pipeline steps\nFix: use unique Capture names before injecting values downstream.")]
    DuplicateCapture {
        /// Duplicate capture name.
        capture: String,
    },
    /// No progress can be made because Inject/Capture dependencies form a cycle.
    #[error("pipeline has cyclic Inject/Capture dependencies\nFix: make each Inject depend on a Capture that can run first.")]
    CyclicDependencies,
}

/// Run a pipeline.
pub async fn run(
    pipeline: &Pipeline,
    env: &str,
    opts: PipelineOpts,
) -> Result<PipelineResult, PipelineError> {
    validate_dependencies(pipeline)?;
    if pipeline.schema.parallel {
        run_parallel(pipeline, env, opts).await
    } else {
        run_sequential(pipeline, env, opts).await
    }
}

/// Validate Inject/Capture graph rules that can be checked without sending requests.
pub fn validate_dependencies(pipeline: &Pipeline) -> Result<(), PipelineError> {
    let capture_owners = capture_owners(&pipeline.steps)?;
    let dependencies = step_dependencies(&pipeline.steps, &capture_owners);
    let mut pending: BTreeSet<usize> = (0..pipeline.steps.len()).collect();
    let mut completed = BTreeSet::new();

    while !pending.is_empty() {
        let wave: Vec<usize> = pending
            .iter()
            .copied()
            .filter(|index| dependencies[*index].is_subset(&completed))
            .collect();
        if wave.is_empty() {
            return Err(PipelineError::CyclicDependencies);
        }
        for index in wave {
            pending.remove(&index);
            completed.insert(index);
        }
    }

    Ok(())
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
    let capture_owners = capture_owners(&pipeline.steps)?;
    let dependencies = step_dependencies(&pipeline.steps, &capture_owners);
    let mut pending: BTreeSet<usize> = (0..pipeline.steps.len()).collect();
    let mut completed = BTreeSet::new();
    let mut results: Vec<Option<StepResult>> = vec![None; pipeline.steps.len()];
    let mut captures = BTreeMap::new();
    let mut passed = true;

    while !pending.is_empty() {
        let wave: Vec<usize> = pending
            .iter()
            .copied()
            .filter(|index| dependencies[*index].is_subset(&completed))
            .collect();
        if wave.is_empty() {
            return Err(PipelineError::CyclicDependencies);
        }

        let handles: Vec<_> = wave
            .iter()
            .map(|index| {
                let root = opts.root.clone();
                let exec_opts = opts.exec.clone();
                let env = env.to_string();
                let step = pipeline.steps[*index].clone();
                let captures = captures.clone();
                tokio::task::spawn(async move {
                    execute_step(&step, &env, &root, exec_opts, captures).await
                })
            })
            .collect();

        let mut wave_failed = false;
        for (index, handle) in wave.into_iter().zip(handles) {
            let (step_captures, step_result) =
                handle.await.expect("pipeline step task panicked")?;
            let step_passed = step_result
                .execution
                .as_ref()
                .map_or(step_result.error.is_none(), |e| e.diff.passed);
            if !step_passed {
                passed = false;
                wave_failed = true;
            }
            captures.extend(step_captures);
            results[index] = Some(step_result);
            pending.remove(&index);
            completed.insert(index);
        }

        if wave_failed && !pipeline.schema.continue_on_error {
            break;
        }
    }

    Ok(PipelineResult {
        steps: results.into_iter().flatten().collect(),
        captures,
        passed,
    })
}

fn capture_owners(steps: &[PipelineStep]) -> Result<BTreeMap<String, usize>, PipelineError> {
    let mut owners = BTreeMap::new();
    for (index, step) in steps.iter().enumerate() {
        for capture in &step.capture {
            if owners.insert(capture.name.clone(), index).is_some() {
                return Err(PipelineError::DuplicateCapture {
                    capture: capture.name.clone(),
                });
            }
        }
    }
    Ok(owners)
}

fn step_dependencies(
    steps: &[PipelineStep],
    capture_owners: &BTreeMap<String, usize>,
) -> Vec<BTreeSet<usize>> {
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            step.inject
                .iter()
                .filter_map(|name| capture_owners.get(name).copied())
                .filter(|owner| *owner != index)
                .collect()
        })
        .collect()
}

async fn execute_step(
    step: &PipelineStep,
    env: &str,
    root: &Path,
    exec_opts: ExecOpts,
    initial_captures: BTreeMap<String, String>,
) -> Result<(BTreeMap<String, String>, StepResult), PipelineError> {
    let missing_injections: Vec<String> = step
        .inject
        .iter()
        .filter(|name| !initial_captures.contains_key(*name))
        .cloned()
        .collect();
    if !missing_injections.is_empty() {
        return Ok((
            BTreeMap::new(),
            StepResult {
                name: step.name.clone(),
                endpoint: step.endpoint.clone(),
                execution: None,
                error: Some(format!(
                    "missing injected capture(s): {}\nFix: add a prior Capture directive or remove the Inject directive.",
                    missing_injections.join(", ")
                )),
            },
        ));
    }

    let path = root.join(&step.endpoint);
    let path_display = path.display().to_string();
    let source = fs::read_to_string(&path).map_err(|source| PipelineError::ReadEndpoint {
        path: path_display.clone(),
        source,
    })?;
    let endpoint = parse_endpoint(&source, &path)?;
    let mut opts = exec_opts;
    let dry_run = opts.dry_run;
    for (key, value) in &initial_captures {
        opts.context
            .insert(SourceKind::Pipeline, key.clone(), value.clone());
    }

    match execute(&endpoint, env, opts).await {
        Ok(execution) => {
            let mut new_captures = BTreeMap::new();
            for capture in &step.capture {
                let value = if dry_run && execution.response.is_none() {
                    synthetic_capture_value(&capture.name)
                } else {
                    capture_value(&execution, &capture.source).ok_or_else(|| {
                        PipelineError::Capture {
                            step: step.name.clone(),
                            capture: capture.source.clone(),
                        }
                    })?
                };
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

fn synthetic_capture_value(name: &str) -> String {
    format!("__capture_{name}__")
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
    async fn parallel_pipeline_waits_for_injected_capture() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "u1"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/users/u1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "u1",
                "name": "Ada"
            })))
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
        fs::write(
            dir.path().join("users/get-user-detail.md"),
            format!(
                r#"---
resource: users
protocol: http
method: GET
path: /users/:userId
version: 1
---
# Get user detail

Fetches user detail.

## Request

```http
GET {}/users/:userId
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
parallel: true
---
# Demo

## Steps

1. **Get user detail** -> `users/get-user-detail.md`
   - Inject: `userId`
2. **Get user** -> `users/get-user.md`
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

        assert!(result.passed);
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.captures["userId"], "u1");
    }

    #[tokio::test]
    async fn dry_run_pipeline_uses_synthetic_captures_for_downstream_steps() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("users")).unwrap();
        fs::write(
            dir.path().join("users/create-user.md"),
            r#"---
resource: users
protocol: http
method: POST
path: /users
version: 1
---
# Create user

Creates a user.

## Request

```http
POST https://api.example.test/users
Content-Type: application/json

{"name":"Ada"}
```

## Expected response

```http
HTTP/1.1 201 Created

{"id":"u1"}
```
"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("users/get-user.md"),
            r#"---
resource: users
protocol: http
method: GET
path: /users/:userId
version: 1
---
# Get user

Fetches a user.

## Request

```http
GET https://api.example.test/users/:userId
```

## Expected response

```http
HTTP/1.1 200 OK

{"id":"u1"}
```
"#,
        )
        .unwrap();
        let pipeline = parse_pipeline(
            r#"---
type: pipeline
name: dry-run-demo
---
# Dry run demo

## Steps

1. **Create user** -> `users/create-user.md`
   - Capture: `response.body.id` as `userId`
2. **Get user** -> `users/get-user.md`
   - Inject: userId
"#,
            "pipeline.md",
        )
        .unwrap();

        let result = run(
            &pipeline,
            "dev",
            PipelineOpts {
                root: dir.path().to_path_buf(),
                exec: ExecOpts {
                    dry_run: true,
                    ..ExecOpts::default()
                },
            },
        )
        .await
        .unwrap();

        assert!(result.passed);
        assert_eq!(result.captures["userId"], "__capture_userId__");
        assert!(result.steps.iter().all(|step| step
            .execution
            .as_ref()
            .is_some_and(|execution| execution.response.is_none())));
        let second = result.steps[1].execution.as_ref().unwrap();
        assert_eq!(
            second.request.url,
            "https://api.example.test/users/__capture_userId__"
        );
    }

    #[tokio::test]
    async fn parallel_pipeline_rejects_duplicate_capture_names() {
        let pipeline = parse_pipeline(
            r#"---
type: pipeline
name: demo
parallel: true
---
# Demo

## Steps

1. **One** -> `users/one.md`
   - Capture: `response.body.id` as `userId`
2. **Two** -> `users/two.md`
   - Capture: `response.body.id` as `userId`
"#,
            "pipeline.md",
        )
        .unwrap();

        let err = run(
            &pipeline,
            "dev",
            PipelineOpts {
                root: tempdir().unwrap().path().to_path_buf(),
                exec: ExecOpts::default(),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(err, PipelineError::DuplicateCapture { .. }));
    }

    #[test]
    fn validate_dependencies_rejects_cycles_without_network() {
        let pipeline = parse_pipeline(
            r#"---
type: pipeline
name: cyclic
---
# Cyclic

## Steps

1. **One** -> `users/one.md`
   - Inject: twoId
   - Capture: `response.body.id` as oneId
2. **Two** -> `users/two.md`
   - Inject: oneId
   - Capture: `response.body.id` as twoId
"#,
            "pipeline.md",
        )
        .unwrap();

        let err = validate_dependencies(&pipeline).unwrap_err();
        assert!(matches!(err, PipelineError::CyclicDependencies));
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
