//! Compact API context rendering for coding agents.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as AnyhowContext, Result};
use serde_json::Value;

use crate::parser::{parse_endpoint, parse_pipeline, Endpoint, PipelineStep};

/// Output style for agent context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMode {
    /// Minimal contract-only context for low-token agent execution.
    Surgical,
    /// Human-readable compact context with descriptions and related flows.
    Compact,
    /// JSON schema summary for tool-driven agents.
    Schema,
}

impl ContextMode {
    /// Parse a CLI/MCP mode name.
    pub fn parse(input: &str) -> Result<Self> {
        match input {
            "surgical" => Ok(Self::Surgical),
            "compact" => Ok(Self::Compact),
            "schema" => Ok(Self::Schema),
            other => anyhow::bail!(
                "unknown context mode `{other}`. Use one of: surgical, compact, schema"
            ),
        }
    }

    /// Stable mode name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Surgical => "surgical",
            Self::Compact => "compact",
            Self::Schema => "schema",
        }
    }
}

/// Options for rendering agent context.
#[derive(Debug, Clone)]
pub struct AgentContextOptions {
    /// api-docs root directory.
    pub root: PathBuf,
    /// Endpoint/flow id or file path. When absent with `changed_from`, changed files are summarized.
    pub target: Option<String>,
    /// Git ref used to summarize changed specs.
    pub changed_from: Option<String>,
    /// Approximate token budget.
    pub token_budget: usize,
    /// Include full request and expected response blocks.
    pub verbose: bool,
    /// Environment name used in suggested commands.
    pub env: String,
    /// Output style.
    pub mode: ContextMode,
    /// Optional agent task intent, e.g. implement, debug, test, review.
    pub intent: Option<String>,
}

/// Render compact context for an endpoint, flow, or changed files.
pub fn render(options: AgentContextOptions) -> Result<String> {
    let mut out = if let Some(base) = &options.changed_from {
        render_changed(&options.root, base, &options)?
    } else if let Some(target) = &options.target {
        render_target(&options.root, target, &options)?
    } else {
        anyhow::bail!("target is required unless --changed-from is provided");
    };
    truncate_to_budget(&mut out, options.token_budget);
    Ok(out)
}

fn render_target(root: &Path, target: &str, options: &AgentContextOptions) -> Result<String> {
    let path = resolve_target(root, target)
        .with_context(|| format!("finding context target `{target}` under {}", root.display()))?;
    if is_flow_file(&path) {
        render_flow(root, &path, options)
    } else {
        render_endpoint(root, &path, options)
    }
}

fn render_changed(root: &Path, base: &str, options: &AgentContextOptions) -> Result<String> {
    let files = changed_files(base, root)?;
    let mut out = format!("Changed API context since {base}\n\n");
    if files.is_empty() {
        out.push_str("No changed endpoint or flow specs found.");
        return Ok(out);
    }
    for file in files {
        if !is_context_file(&file) {
            continue;
        }
        if is_flow_file(&file) {
            out.push_str(&render_flow(root, &file, options)?);
        } else {
            out.push_str(&render_endpoint(root, &file, options)?);
        }
        out.push_str("\n\n");
    }
    Ok(out.trim_end().to_string())
}

fn render_endpoint(root: &Path, file: &Path, options: &AgentContextOptions) -> Result<String> {
    let source =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let endpoint = parse_endpoint(&source, file)?;
    match options.mode {
        ContextMode::Surgical => render_endpoint_surgical(root, file, &endpoint, options),
        ContextMode::Compact => render_endpoint_compact(root, file, &endpoint, options),
        ContextMode::Schema => render_endpoint_schema(root, file, &endpoint, options),
    }
}

fn render_endpoint_compact(
    root: &Path,
    file: &Path,
    endpoint: &Endpoint,
    options: &AgentContextOptions,
) -> Result<String> {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let variables = variables_for(endpoint);
    let expected = expected_summary(&endpoint.expected_response);
    let related = related_flows(root, rel);
    let auth = endpoint
        .schema
        .auth
        .as_ref()
        .map(|auth| format!("{auth:?}").to_ascii_lowercase())
        .unwrap_or_else(|| "none".to_string());

    let mut out = format!(
        "Endpoint: {} {}\nFile: {}\nTitle: {}\nAuth: {}\nVariables: {}\nExpected: {}\n",
        endpoint.schema.method.as_str(),
        endpoint.schema.path,
        rel.display(),
        endpoint.title,
        auth,
        empty_dash(variables),
        expected
    );
    if !endpoint.description.trim().is_empty() {
        out.push_str("Description: ");
        out.push_str(endpoint.description.trim());
        out.push('\n');
    }
    if !related.is_empty() {
        out.push_str("Related flow: ");
        out.push_str(&related.join(", "));
        out.push('\n');
    }
    out.push_str(&format!(
        "Safe next command: rqb exec {} --env {}\n",
        file.display(),
        options.env
    ));
    if options.verbose {
        out.push_str("\nRequest:\n");
        out.push_str(&endpoint.request);
        out.push_str("\n\nExpected response:\n");
        out.push_str(&endpoint.expected_response);
        out.push('\n');
        if let Some(tests) = &endpoint.tests {
            out.push_str("\nAgent task:\n");
            out.push_str(tests.trim());
            out.push('\n');
        }
        if let Some(notes) = &endpoint.notes {
            out.push_str("\nNotes:\n");
            out.push_str(notes.trim());
            out.push('\n');
        }
    }
    Ok(out)
}

fn render_endpoint_surgical(
    root: &Path,
    file: &Path,
    endpoint: &Endpoint,
    options: &AgentContextOptions,
) -> Result<String> {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let variables = variables_for(endpoint);
    let request_shape = body_shape(&endpoint.request, "body", 10);
    let (status, response_shape) = response_shape(&endpoint.expected_response, 10);
    let assertions = assertion_summary(endpoint, 6);
    let intent = options.intent.as_deref().unwrap_or("implement");

    let mut out = format!(
        "API contract ({intent}): {} {}\nFile: {}\n",
        endpoint.schema.method.as_str(),
        endpoint.schema.path,
        rel.display()
    );
    out.push_str(&format!("Title: {}\n", endpoint.title));
    out.push_str(&format!("Variables: {}\n", empty_dash(variables)));
    out.push_str(&format!(
        "Request body: {}\n",
        if request_shape.is_empty() {
            "none".to_string()
        } else {
            request_shape.join(", ")
        }
    ));
    out.push_str(&format!(
        "Success response: HTTP {}{}{}\n",
        status.unwrap_or_else(|| "?".to_string()),
        if response_shape.is_empty() { "" } else { " " },
        response_shape.join(", ")
    ));
    if !assertions.is_empty() {
        out.push_str(&format!("Assertions: {}\n", assertions.join("; ")));
    }
    out.push_str("Agent steps:\n");
    out.push_str("- Use this contract before reading backend source.\n");
    out.push_str(
        "- Only inspect source files needed to satisfy this method/path and body shape.\n",
    );
    out.push_str("- Verify with the commands below after code changes.\n");
    out.push_str("Verify:\n");
    out.push_str(&format!("- rqb validate {}\n", root.display()));
    out.push_str(&format!(
        "- rqb exec {} --env {}\n",
        file.display(),
        options.env
    ));
    Ok(out)
}

fn render_endpoint_schema(
    root: &Path,
    file: &Path,
    endpoint: &Endpoint,
    options: &AgentContextOptions,
) -> Result<String> {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let (status, response_fields) = response_shape(&endpoint.expected_response, 20);
    let value = serde_json::json!({
        "type": "endpoint_contract",
        "mode": options.mode.as_str(),
        "intent": options.intent.as_deref().unwrap_or("implement"),
        "file": rel.to_string_lossy(),
        "title": endpoint.title,
        "method": endpoint.schema.method.as_str(),
        "path": endpoint.schema.path,
        "auth": endpoint.schema.auth.as_ref().map(|auth| format!("{auth:?}").to_ascii_lowercase()),
        "variables": variables_for(endpoint),
        "request": {
            "body_fields": body_shape(&endpoint.request, "body", 20),
        },
        "success_response": {
            "status": status,
            "body_fields": response_fields,
        },
        "assertions": assertion_summary(endpoint, 20),
        "verify": [
            format!("rqb validate {}", root.display()),
            format!("rqb exec {} --env {}", file.display(), options.env),
        ],
    });
    serde_json::to_string_pretty(&value).map_err(Into::into)
}

fn render_flow(root: &Path, file: &Path, options: &AgentContextOptions) -> Result<String> {
    let source =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let flow = parse_pipeline(&source, file)?;
    let rel = file.strip_prefix(root).unwrap_or(file);
    if options.mode == ContextMode::Schema {
        let steps = flow
            .steps
            .iter()
            .map(|step| {
                serde_json::json!({
                    "name": step.name,
                    "endpoint": step.endpoint,
                    "captures": step.capture.iter().map(|capture| {
                        serde_json::json!({"source": capture.source, "name": capture.name})
                    }).collect::<Vec<_>>(),
                    "injects": step.inject,
                })
            })
            .collect::<Vec<_>>();
        let value = serde_json::json!({
            "type": "flow_contract",
            "mode": options.mode.as_str(),
            "intent": options.intent.as_deref().unwrap_or("implement"),
            "file": rel.to_string_lossy(),
            "name": flow.schema.name,
            "steps": steps,
            "verify": [
                format!("rqb validate {}", root.display()),
                format!("rqb flow {} --env {}", file.display(), options.env),
            ],
        });
        return serde_json::to_string_pretty(&value).map_err(Into::into);
    }

    let mut out = format!(
        "{}Flow: {}\nFile: {}\nSteps: {}\n",
        if options.mode == ContextMode::Surgical {
            "API flow contract\n"
        } else {
            ""
        },
        flow.schema.name,
        rel.display(),
        flow.steps.len()
    );
    for step in &flow.steps {
        out.push_str(&format!("- {} -> {}\n", step.name, step.endpoint));
        if !step.capture.is_empty() {
            let captures = step
                .capture
                .iter()
                .map(|capture| format!("{} as {}", capture.source, capture.name))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  Captures: {captures}\n"));
        }
        if !step.inject.is_empty() {
            out.push_str(&format!("  Injects: {}\n", step.inject.join(", ")));
        }
    }
    if flow.steps.iter().any(|step| !step.inject.is_empty()) {
        out.push_str(
            "Inject semantics: Inject reads values captured by previous steps; env and CLI variables are resolved in request templates but do not satisfy Inject.\n",
        );
    }
    if options.mode == ContextMode::Surgical {
        out.push_str("Agent steps:\n");
        out.push_str("- Use captures/injects as the source of truth for data dependencies.\n");
        out.push_str(
            "- Verify the first failing step in isolation before changing multiple files.\n",
        );
        out.push_str("Verify:\n");
        out.push_str(&format!("- rqb validate {}\n", root.display()));
        out.push_str(&format!(
            "- rqb flow {} --env {}\n",
            file.display(),
            options.env
        ));
    } else {
        out.push_str(&format!(
            "Safe next command: rqb flow {} --env {}\n",
            file.display(),
            options.env
        ));
    }
    if options.verbose {
        out.push_str("\nEndpoint details:\n");
        for step in &flow.steps {
            append_step_endpoint_context(root, step, options, &mut out)?;
        }
    }
    Ok(out)
}

fn append_step_endpoint_context(
    root: &Path,
    step: &PipelineStep,
    options: &AgentContextOptions,
    out: &mut String,
) -> Result<()> {
    let endpoint_path = root.join(&step.endpoint);
    out.push_str("\n## ");
    out.push_str(&step.name);
    out.push('\n');
    if endpoint_path.exists() {
        out.push_str(&render_endpoint(root, &endpoint_path, options)?);
    } else {
        out.push_str("Missing endpoint file: ");
        out.push_str(&step.endpoint);
        out.push('\n');
    }
    Ok(())
}

fn resolve_target(root: &Path, target: &str) -> Result<PathBuf> {
    let explicit = Path::new(target);
    if explicit.exists() {
        return Ok(explicit.to_path_buf());
    }
    let mut candidates = Vec::new();
    collect_markdown(root, &mut candidates)?;
    let normalized_target = normalize_id(target);
    for file in candidates {
        let rel = file.strip_prefix(root).unwrap_or(&file).to_string_lossy();
        let stem = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(normalize_id)
            .unwrap_or_default();
        if normalize_id(&rel).contains(&normalized_target) || stem == normalized_target {
            return Ok(file);
        }
        let source = std::fs::read_to_string(&file).unwrap_or_default();
        if is_flow_file(&file) {
            if let Ok(flow) = parse_pipeline(&source, &file) {
                if normalize_id(&flow.schema.name) == normalized_target {
                    return Ok(file);
                }
            }
        } else if let Ok(endpoint) = parse_endpoint(&source, &file) {
            let endpoint_id = format!(
                "{}.{}",
                endpoint.schema.resource,
                file.file_stem().and_then(|s| s.to_str()).unwrap_or("")
            );
            if normalize_id(&endpoint.title) == normalized_target
                || normalize_id(&endpoint_id).contains(&normalized_target)
            {
                return Ok(file);
            }
        }
    }
    anyhow::bail!("no endpoint or flow matched `{target}`")
}

fn variables_for(endpoint: &Endpoint) -> Vec<String> {
    let mut vars = BTreeSet::new();
    let template_re =
        regex::Regex::new(r"\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}").expect("valid var regex");
    for caps in template_re.captures_iter(&endpoint.request) {
        vars.insert(caps[1].to_string());
    }
    let path_re = regex::Regex::new(r":([A-Za-z_][A-Za-z0-9_]*)").expect("valid path regex");
    for caps in path_re.captures_iter(&endpoint.schema.path) {
        vars.insert(caps[1].to_string());
    }
    vars.into_iter().collect()
}

fn expected_summary(expected_response: &str) -> String {
    let (status, body) = http_status_and_body(expected_response);
    let status = status.unwrap_or_else(|| "?".to_string());
    let mut fields = Vec::new();
    if let Ok(json) = serde_json::from_str::<Value>(&body) {
        collect_json_fields("body", &json, &mut fields);
    }
    if fields.is_empty() {
        status.to_string()
    } else {
        format!("{status} {}", fields.join(", "))
    }
}

fn response_shape(expected_response: &str, limit: usize) -> (Option<String>, Vec<String>) {
    let (status, body) = http_status_and_body(expected_response);
    let fields = json_shape(&body, "body", limit);
    (status, fields)
}

fn body_shape(http_block: &str, prefix: &str, limit: usize) -> Vec<String> {
    let (_, body) = http_status_and_body(http_block);
    json_shape(&body, prefix, limit)
}

fn http_status_and_body(http_block: &str) -> (Option<String>, String) {
    let normalized = http_block.replace("\r\n", "\n");
    let (head, body) = normalized
        .split_once("\n\n")
        .map_or((normalized.as_str(), ""), |(head, body)| (head, body));
    let status = head
        .lines()
        .next()
        .and_then(|line| {
            if line.starts_with("HTTP/") {
                line.split_whitespace().nth(1)
            } else {
                None
            }
        })
        .map(str::to_string);
    (status, body.to_string())
}

fn json_shape(body: &str, prefix: &str, limit: usize) -> Vec<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
        return vec![format!("{prefix}:raw")];
    };
    let mut fields = Vec::new();
    collect_json_shape(prefix, &json, &mut fields, limit);
    fields
}

fn collect_json_shape(prefix: &str, value: &Value, fields: &mut Vec<String>, limit: usize) {
    if fields.len() >= limit {
        return;
    }
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                fields.push(format!("{prefix}:object"));
                return;
            }
            for (key, value) in map {
                let next = format!("{prefix}.{key}");
                match value {
                    Value::Object(_) | Value::Array(_) => {
                        collect_json_shape(&next, value, fields, limit)
                    }
                    _ => fields.push(format!("{next}:{}", json_type(value))),
                }
                if fields.len() >= limit {
                    return;
                }
            }
        }
        Value::Array(items) => {
            if let Some(first) = items.first() {
                collect_json_shape(&format!("{prefix}[]"), first, fields, limit);
            } else {
                fields.push(format!("{prefix}:array"));
            }
        }
        _ => fields.push(format!("{prefix}:{}", json_type(value))),
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn assertion_summary(endpoint: &Endpoint, limit: usize) -> Vec<String> {
    endpoint
        .assertions
        .iter()
        .take(limit)
        .map(|assertion| {
            let op = serde_json::to_string(&assertion.op)
                .unwrap_or_else(|_| "\"equals\"".to_string())
                .trim_matches('"')
                .to_string();
            match &assertion.value {
                Some(value) => format!("{} {} {}", assertion.path, op, value),
                None => format!("{} {}", assertion.path, op),
            }
        })
        .collect()
}

fn collect_json_fields(prefix: &str, value: &Value, fields: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let next = format!("{prefix}.{key}");
                match value {
                    Value::Object(_) => collect_json_fields(&next, value, fields),
                    Value::Array(items) if !items.is_empty() => {
                        collect_json_fields(&format!("{next}[]"), &items[0], fields)
                    }
                    _ => fields.push(next),
                }
                if fields.len() >= 8 {
                    return;
                }
            }
        }
        Value::Array(items) if !items.is_empty() => {
            collect_json_fields(&format!("{prefix}[]"), &items[0], fields);
        }
        _ => {}
    }
}

fn related_flows(root: &Path, endpoint_rel: &Path) -> Vec<String> {
    let mut flows = Vec::new();
    let flow_dirs = [root.join("flows"), root.join("pipelines")];
    for dir in flow_dirs {
        if !dir.exists() {
            continue;
        }
        let mut files = Vec::new();
        if collect_markdown(&dir, &mut files).is_ok() {
            for file in files {
                let Ok(source) = std::fs::read_to_string(&file) else {
                    continue;
                };
                let needle = endpoint_rel.to_string_lossy();
                if source.contains(needle.as_ref()) {
                    flows.push(
                        file.strip_prefix(root)
                            .unwrap_or(&file)
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    }
    flows.sort();
    flows
}

fn changed_files(base: &str, root: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", base, "--"])
        .arg(root)
        .output()
        .with_context(|| format!("running git diff against {base}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut files: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .collect();
    files.sort();
    Ok(files)
}

fn collect_markdown(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "_shared" {
                collect_markdown(&path, out)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "md") && is_context_file(&path) {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}

fn is_context_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    path.extension().is_some_and(|ext| ext == "md")
        && !matches!(name, "README.md" | "reqbook.md" | "mad.md" | "env.md")
}

fn is_flow_file(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component.as_os_str().to_str(), Some("flows" | "pipelines")))
}

fn normalize_id(input: &str) -> String {
    input
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

fn empty_dash(values: Vec<String>) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn truncate_to_budget(out: &mut String, token_budget: usize) {
    let max_chars = token_budget.max(64) * 4;
    if out.len() > max_chars {
        out.truncate(max_chars.saturating_sub(20));
        out.push_str("\n...[truncated]");
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn summarizes_expected_fields() {
        let summary = expected_summary(
            "HTTP/1.1 201 Created\nContent-Type: application/json\n\n{\"id\":\"u1\",\"profile\":{\"email\":\"a\"}}",
        );
        assert!(summary.contains("201"));
        assert!(summary.contains("body.id"));
        assert!(summary.contains("body.profile.email"));
    }

    #[test]
    fn verbose_flow_context_includes_step_endpoint_details() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("apis/users")).unwrap();
        fs::create_dir_all(root.join("flows")).unwrap();
        fs::write(
            root.join("apis/users/patch-user.md"),
            r#"---
resource: users
protocol: http
method: PATCH
path: /users/:id
version: 1
---
# Update user

Update a user profile.

## Request

```http
PATCH {{baseUrl}}/users/:id
Content-Type: application/json

{"name":"Ada"}
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{"id":"u1","name":"Ada"}
```

## Tests

```agent-task
Implement the PATCH handler and verify the response body keeps the user id.
```
"#,
        )
        .unwrap();
        fs::write(
            root.join("flows/update-user.md"),
            r#"---
type: pipeline
name: update-user
---
# Update user

## Steps

1. **Update user** -> `apis/users/patch-user.md`
   - Capture: `response.body.id` as id
"#,
        )
        .unwrap();

        let rendered = render(AgentContextOptions {
            root: root.to_path_buf(),
            target: Some(root.join("flows/update-user.md").display().to_string()),
            changed_from: None,
            token_budget: 1200,
            verbose: true,
            env: "dev".to_string(),
            mode: ContextMode::Compact,
            intent: None,
        })
        .unwrap();

        assert!(rendered.contains("Endpoint details"));
        assert!(rendered.contains("Endpoint: PATCH /users/:id"));
        assert!(rendered.contains("Request:"));
        assert!(rendered.contains("Agent task:"));
    }

    #[test]
    fn surgical_context_summarizes_json_contract() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("apis/refunds")).unwrap();
        let spec = root.join("apis/refunds/post-refund-quote.md");
        fs::write(
            &spec,
            r#"---
resource: refunds
protocol: http
method: POST
path: /refunds/quote
version: 1
---
# Create refund quote

Create a quote before refund capture.

## Request

```http
POST {{baseUrl}}/refunds/quote
Content-Type: application/json

{"orderId":"ord_123","reason":"duplicate"}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{"quoteId":"rfq_123","amount":1250,"expiresAt":"2026-06-07T12:00:00Z"}
```
"#,
        )
        .unwrap();

        let rendered = render(AgentContextOptions {
            root: root.to_path_buf(),
            target: Some(spec.display().to_string()),
            changed_from: None,
            token_budget: 400,
            verbose: false,
            env: "dev".to_string(),
            mode: ContextMode::Surgical,
            intent: Some("implement".to_string()),
        })
        .unwrap();

        assert!(rendered.contains("API contract (implement): POST /refunds/quote"));
        assert!(rendered.contains("body.orderId:string"));
        assert!(rendered.contains("body.amount:integer"));
        assert!(rendered.contains("Verify:"));
    }
}
