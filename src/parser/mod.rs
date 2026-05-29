//! Markdown parsing for MarkApiDown specs.

use std::{collections::BTreeMap, path::Path};

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::resolver::ensure_no_secret;

/// Parsed endpoint specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Endpoint {
    /// Source path when loaded from disk.
    pub source: Option<String>,
    /// Endpoint frontmatter.
    pub schema: EndpointSchema,
    /// Sentence-case title from the H1.
    pub title: String,
    /// First paragraph after the title.
    pub description: String,
    /// Executable HTTP request block.
    pub request: String,
    /// Expected HTTP response block.
    pub expected_response: String,
    /// Optional agent task test instructions.
    pub tests: Option<String>,
    /// Optional notes section.
    pub notes: Option<String>,
    /// Structured assertions from the `## Assertions` section.
    #[serde(default)]
    pub assertions: Vec<Assertion>,
}

/// A structured assertion rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assertion {
    /// Path to check: `"status"`, `"body.field"`, `"headers.content-type"`, etc.
    pub path: String,
    /// Comparison operator.
    pub op: AssertionOp,
    /// Value operand (absent for `exists`).
    pub value: Option<String>,
}

/// Assertion operator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssertionOp {
    /// Field exists and is non-null.
    Exists,
    /// Exact string match.
    Equals,
    /// Substring match.
    Contains,
    /// Regex match.
    Matches,
    /// Value is one of a comma-separated list.
    In,
}

/// Endpoint YAML frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct EndpointSchema {
    /// Resource group, usually matching the containing folder.
    pub resource: String,
    /// Protocol. MarkApiDown v1.0 executes HTTP only.
    pub protocol: Protocol,
    /// HTTP method.
    pub method: HttpMethod,
    /// URL path, with path params in `:param` form.
    pub path: String,
    /// Search and grouping tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Endpoint spec version.
    pub version: u32,
    /// Supported environments.
    #[serde(default)]
    pub env: Vec<String>,
    /// Auth mode.
    #[serde(default)]
    pub auth: Option<AuthMode>,
    /// Timeout in milliseconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Retry policy.
    #[serde(default)]
    pub retry: Option<RetryPolicy>,
}

/// Supported endpoint protocols.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// HTTP request/response.
    Http,
    /// Reserved for future versions.
    Ws,
    /// Reserved for future versions.
    Sse,
}

/// HTTP method supported by request blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// GET.
    Get,
    /// POST.
    Post,
    /// PUT.
    Put,
    /// PATCH.
    Patch,
    /// DELETE.
    Delete,
    /// HEAD.
    Head,
    /// OPTIONS.
    Options,
}

impl HttpMethod {
    /// Return the uppercase method string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

/// Endpoint auth modes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    /// No auth.
    None,
    /// Bearer token auth.
    Bearer,
    /// Basic auth.
    Basic,
    /// Custom auth handled by request headers.
    Custom,
}

/// Retry policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Number of retry attempts after the first request.
    pub attempts: u32,
    /// Backoff mode.
    pub backoff: Backoff,
}

/// Retry backoff mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Backoff {
    /// Fixed delay.
    Fixed,
    /// Exponential delay.
    Exponential,
}

/// Parsed pipeline specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pipeline {
    /// Source path when loaded from disk.
    pub source: Option<String>,
    /// Pipeline frontmatter.
    pub schema: PipelineSchema,
    /// Pipeline title.
    pub title: String,
    /// Ordered steps.
    pub steps: Vec<PipelineStep>,
}

/// Pipeline YAML frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct PipelineSchema {
    /// Must be `pipeline`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Pipeline name.
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether execution continues after step failure.
    #[serde(default)]
    pub continue_on_error: bool,
    /// Whether independent steps may run concurrently.
    #[serde(default)]
    pub parallel: bool,
}

/// One pipeline step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineStep {
    /// Step display name.
    pub name: String,
    /// Endpoint path relative to `api-docs/`.
    pub endpoint: String,
    /// Variable names injected into the step.
    pub inject: Vec<String>,
    /// Captures from response paths.
    pub capture: Vec<Capture>,
    /// Assertions in simple textual form.
    pub assert: Vec<String>,
}

/// A captured pipeline variable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capture {
    /// JSONPath-like source expression.
    pub source: String,
    /// Destination variable name.
    pub name: String,
}

/// Environment config keyed by environment name.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EnvConfig {
    /// Environment values.
    pub envs: BTreeMap<String, BTreeMap<String, String>>,
}

/// Parser errors with actionable context.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Frontmatter is missing.
    #[error("{path}: missing frontmatter\nFix: add YAML frontmatter delimited by --- at the start of the file.")]
    MissingFrontmatter {
        /// File path.
        path: String,
    },
    /// Required section is missing.
    #[error("{path}: missing section {name}\nFix: add `{name}` in the canonical endpoint section order.")]
    MissingSection {
        /// File path.
        path: String,
        /// Section name.
        name: String,
    },
    /// More than one executable HTTP block was found.
    #[error("{path}: multiple http blocks in {section}\nFix: keep one executable http block and move alternatives to Notes.")]
    MultipleHttpBlocks {
        /// File path.
        path: String,
        /// Section.
        section: String,
    },
    /// YAML did not deserialize.
    #[error("{path}: invalid YAML: {source}\nFix: correct the YAML frontmatter or structured code block.")]
    InvalidYaml {
        /// File path.
        path: String,
        /// Source error.
        #[source]
        source: serde_yaml::Error,
    },
    /// Required field is missing.
    #[error("{path}: missing required field {field}\nFix: add `{field}` to the YAML frontmatter.")]
    MissingField {
        /// File path.
        path: String,
        /// Field name.
        field: &'static str,
    },
    /// Generic invalid spec.
    #[error("{path}: {message}\nFix: {fix}")]
    Invalid {
        /// File path.
        path: String,
        /// Message.
        message: String,
        /// Suggested fix.
        fix: String,
    },
    /// Secret detected in versioned markdown.
    #[error("{path}: possible secret detected\nFix: move this value to .env.local or MAD_* environment variables.")]
    SecretDetected {
        /// File path.
        path: String,
    },
}

/// Parse an endpoint markdown string.
pub fn parse_endpoint(source: &str, path: impl AsRef<Path>) -> Result<Endpoint, ParseError> {
    let path = path.as_ref().display().to_string();
    let (frontmatter, body) = split_frontmatter(source, &path)?;
    let schema: EndpointSchema =
        serde_yaml::from_str(frontmatter).map_err(|source| ParseError::InvalidYaml {
            path: path.clone(),
            source,
        })?;
    validate_endpoint_schema(&schema, &path)?;

    let doc = MarkdownDoc::parse(body);
    let title = doc.h1.clone().ok_or_else(|| ParseError::MissingSection {
        path: path.clone(),
        name: "# <title>".to_string(),
    })?;
    let description = doc.description.clone().unwrap_or_default();
    let request = exactly_one_code(&doc, "Request", "http", &path)?;
    let expected_response = exactly_one_code(&doc, "Expected response", "http", &path)?;
    let tests = optional_one_code(&doc, "Tests", "agent-task", &path)?;
    let notes = doc.section_text("Notes");
    let assertions = parse_assertions(doc.section_text("Assertions").as_deref().unwrap_or(""));

    Ok(Endpoint {
        source: Some(path),
        schema,
        title,
        description,
        request,
        expected_response,
        tests,
        notes,
        assertions,
    })
}

/// Parse a pipeline markdown string.
pub fn parse_pipeline(source: &str, path: impl AsRef<Path>) -> Result<Pipeline, ParseError> {
    let path = path.as_ref().display().to_string();
    let (frontmatter, body) = split_frontmatter(source, &path)?;
    let schema: PipelineSchema =
        serde_yaml::from_str(frontmatter).map_err(|source| ParseError::InvalidYaml {
            path: path.clone(),
            source,
        })?;
    if schema.kind != "pipeline" {
        return Err(ParseError::Invalid {
            path,
            message: "pipeline type must be `pipeline`".to_string(),
            fix: "set `type: pipeline` in frontmatter".to_string(),
        });
    }

    let doc = MarkdownDoc::parse(body);
    let title = doc.h1.clone().ok_or_else(|| ParseError::MissingSection {
        path: path.clone(),
        name: "# <title>".to_string(),
    })?;
    let steps_text = raw_section(body, "Steps").ok_or_else(|| ParseError::MissingSection {
        path: path.clone(),
        name: "## Steps".to_string(),
    })?;

    Ok(Pipeline {
        source: Some(path),
        schema,
        title,
        steps: parse_steps(&steps_text),
    })
}

/// Parse `_shared/env.md` and reject committed secrets.
pub fn parse_env_config(source: &str, path: impl AsRef<Path>) -> Result<EnvConfig, ParseError> {
    let path = path.as_ref().display().to_string();
    ensure_no_secret(source, &path)
        .map_err(|_| ParseError::SecretDetected { path: path.clone() })?;
    let mut envs = BTreeMap::new();
    let mut current_env: Option<String> = None;
    let mut in_yaml = false;
    let mut yaml = String::new();

    for line in source.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            if let Some(env) = current_env.take() {
                insert_env(&mut envs, env, &yaml, &path)?;
                yaml.clear();
            }
            current_env = Some(name.trim().to_string());
        } else if line.trim() == "```yaml" {
            in_yaml = true;
        } else if in_yaml && line.trim() == "```" {
            in_yaml = false;
        } else if in_yaml {
            yaml.push_str(line);
            yaml.push('\n');
        }
    }
    if let Some(env) = current_env {
        insert_env(&mut envs, env, &yaml, &path)?;
    }

    Ok(EnvConfig { envs })
}

fn insert_env(
    envs: &mut BTreeMap<String, BTreeMap<String, String>>,
    env: String,
    yaml: &str,
    path: &str,
) -> Result<(), ParseError> {
    if yaml.trim().is_empty() {
        envs.insert(env, BTreeMap::new());
        return Ok(());
    }
    let values: BTreeMap<String, serde_yaml::Value> =
        serde_yaml::from_str(yaml).map_err(|source| ParseError::InvalidYaml {
            path: path.to_string(),
            source,
        })?;
    let values = values
        .into_iter()
        .map(|(key, value)| {
            let value = match value {
                serde_yaml::Value::String(value) => value,
                other => serde_yaml::to_string(&other)
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            };
            (key, value)
        })
        .collect();
    envs.insert(env, values);
    Ok(())
}

fn split_frontmatter<'a>(source: &'a str, path: &str) -> Result<(&'a str, &'a str), ParseError> {
    let mut lines = source.lines();
    if lines.next() != Some("---") {
        return Err(ParseError::MissingFrontmatter {
            path: path.to_string(),
        });
    }
    let rest = &source[4..];
    let Some(end) = rest.find("\n---") else {
        return Err(ParseError::MissingFrontmatter {
            path: path.to_string(),
        });
    };
    let frontmatter = &rest[..end];
    let body_start = end + "\n---".len();
    let body = rest
        .get(body_start..)
        .unwrap_or_default()
        .strip_prefix('\n')
        .unwrap_or_default();
    Ok((frontmatter, body))
}

fn validate_endpoint_schema(schema: &EndpointSchema, path: &str) -> Result<(), ParseError> {
    if schema.resource.trim().is_empty() {
        return Err(ParseError::MissingField {
            path: path.to_string(),
            field: "resource",
        });
    }
    if schema.path.trim().is_empty() {
        return Err(ParseError::MissingField {
            path: path.to_string(),
            field: "path",
        });
    }
    if schema.version == 0 {
        return Err(ParseError::Invalid {
            path: path.to_string(),
            message: "version must be greater than 0".to_string(),
            fix: "set `version: 1`".to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Default)]
struct MarkdownDoc {
    h1: Option<String>,
    description: Option<String>,
    sections: BTreeMap<String, Section>,
}

#[derive(Debug, Default)]
struct Section {
    text: String,
    code: BTreeMap<String, Vec<String>>,
}

impl MarkdownDoc {
    fn parse(source: &str) -> Self {
        let parser = Parser::new_ext(source, Options::all());
        let mut doc = Self::default();
        let mut current_heading = String::new();
        let mut in_heading: Option<HeadingLevel> = None;
        let mut heading_text = String::new();
        let mut in_code: Option<String> = None;
        let mut code_text = String::new();
        let mut paragraph = String::new();
        let mut seen_description = false;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    in_heading = Some(level);
                    heading_text.clear();
                }
                Event::End(pulldown_cmark::TagEnd::Heading(level)) => {
                    let text = heading_text.trim().to_string();
                    match level {
                        HeadingLevel::H1 => doc.h1 = Some(text),
                        HeadingLevel::H2 => {
                            current_heading = text.clone();
                            doc.sections.entry(text).or_default();
                        }
                        _ => {}
                    }
                    in_heading = None;
                }
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                    in_code = Some(lang.to_string());
                    code_text.clear();
                }
                Event::End(pulldown_cmark::TagEnd::CodeBlock) => {
                    if let Some(lang) = in_code.take() {
                        doc.sections
                            .entry(current_heading.clone())
                            .or_default()
                            .code
                            .entry(lang)
                            .or_default()
                            .push(code_text.trim().to_string());
                    }
                }
                Event::Start(Tag::Paragraph) => paragraph.clear(),
                Event::End(pulldown_cmark::TagEnd::Paragraph) => {
                    let text = paragraph.trim();
                    if !text.is_empty() {
                        if current_heading.is_empty() && doc.h1.is_some() && !seen_description {
                            doc.description = Some(text.to_string());
                            seen_description = true;
                        } else if !current_heading.is_empty() {
                            let section = doc.sections.entry(current_heading.clone()).or_default();
                            if !section.text.is_empty() {
                                section.text.push('\n');
                            }
                            section.text.push_str(text);
                        }
                    }
                }
                Event::Text(text) | Event::Code(text) => {
                    if in_code.is_some() {
                        code_text.push_str(&text);
                    } else if in_heading.is_some() {
                        heading_text.push_str(&text);
                    } else {
                        paragraph.push_str(&text);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if in_code.is_some() {
                        code_text.push('\n');
                    } else {
                        paragraph.push('\n');
                    }
                }
                _ => {}
            }
        }
        doc
    }

    fn section_text(&self, name: &str) -> Option<String> {
        self.sections.get(name).map(|section| section.text.clone())
    }
}

fn exactly_one_code(
    doc: &MarkdownDoc,
    section: &str,
    lang: &str,
    path: &str,
) -> Result<String, ParseError> {
    let Some(section_data) = doc.sections.get(section) else {
        return Err(ParseError::MissingSection {
            path: path.to_string(),
            name: format!("## {section}"),
        });
    };
    let blocks = section_data.code.get(lang).cloned().unwrap_or_default();
    match blocks.len() {
        1 => Ok(blocks[0].clone()),
        0 => Err(ParseError::MissingSection {
            path: path.to_string(),
            name: format!("```{lang}``` in ## {section}"),
        }),
        _ => Err(ParseError::MultipleHttpBlocks {
            path: path.to_string(),
            section: format!("## {section}"),
        }),
    }
}

fn optional_one_code(
    doc: &MarkdownDoc,
    section: &str,
    lang: &str,
    path: &str,
) -> Result<Option<String>, ParseError> {
    let Some(section_data) = doc.sections.get(section) else {
        return Ok(None);
    };
    let blocks = section_data.code.get(lang).cloned().unwrap_or_default();
    match blocks.len() {
        0 => Ok(None),
        1 => Ok(Some(blocks[0].clone())),
        _ => Err(ParseError::Invalid {
            path: path.to_string(),
            message: format!("multiple {lang} blocks in ## {section}"),
            fix: format!("keep one `{lang}` block in ## {section}"),
        }),
    }
}

/// Parse `## Assertions` section text into structured `Assertion` values.
///
/// Each line starting with `- ` is a rule in one of these forms:
/// - `- status: 201`                       → Equals
/// - `- body.id: exists`                   → Exists
/// - `- body.email: equals "test@x.com"`   → Equals (strips quotes)
/// - `- body.role: in [admin, user]`        → In
/// - `- headers.content-type: contains application/json` → Contains
/// - `- body.slug: matches ^[a-z]+$`        → Matches
fn parse_assertions(source: &str) -> Vec<Assertion> {
    let mut assertions = Vec::new();
    for line in source.lines() {
        let Some(rule) = line.strip_prefix("- ") else {
            continue;
        };
        let Some((path, rest)) = rule.split_once(':') else {
            continue;
        };
        let path = path.trim().to_string();
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }

        // Determine op and value from the rhs.
        let (op, value) = if rest.eq_ignore_ascii_case("exists") {
            (AssertionOp::Exists, None)
        } else if let Some(v) = rest.strip_prefix("equals ").or(rest.strip_prefix("equals\t")) {
            let v = strip_surrounding_quotes(v.trim());
            (AssertionOp::Equals, Some(v.to_string()))
        } else if let Some(v) = rest.strip_prefix("contains ").or(rest.strip_prefix("contains\t")) {
            (AssertionOp::Contains, Some(v.trim().to_string()))
        } else if let Some(v) = rest.strip_prefix("matches ").or(rest.strip_prefix("matches\t")) {
            (AssertionOp::Matches, Some(v.trim().to_string()))
        } else if let Some(v) = rest.strip_prefix("in ").or(rest.strip_prefix("in\t")) {
            // Strip surrounding brackets if present: `[admin, user]` → `admin, user`
            let v = v.trim();
            let v = v
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .unwrap_or(v);
            (AssertionOp::In, Some(v.trim().to_string()))
        } else {
            // Bare value → treat as Equals (e.g. `- status: 201`)
            let v = strip_surrounding_quotes(rest);
            (AssertionOp::Equals, Some(v.to_string()))
        };

        assertions.push(Assertion { path, op, value });
    }
    assertions
}

fn strip_surrounding_quotes(s: &str) -> &str {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn parse_steps(source: &str) -> Vec<PipelineStep> {
    let step_re =
        regex::Regex::new(r#"^\d+\.\s+\*\*(?P<name>[^*]+)\*\*\s*[-→>]+\s*`(?P<path>[^`]+)`"#)
            .expect("valid step regex");
    let capture_re =
        regex::Regex::new(r#"Capture:\s*`?(?P<src>[^`\s]+)`?\s+as\s+`?(?P<name>[A-Za-z0-9_]+)`?"#)
            .expect("valid capture regex");
    let inject_re = regex::Regex::new(r#"Inject:\s*(?P<vars>.+)$"#).expect("valid inject regex");
    let assert_re = regex::Regex::new(r#"Assert:\s*(?P<assert>.+)$"#).expect("valid assert regex");
    let var_re = regex::Regex::new(r#"`(?P<var>[^`]+)`"#).expect("valid variable regex");

    let mut steps = Vec::new();
    for line in source.lines() {
        if let Some(caps) = step_re.captures(line) {
            steps.push(PipelineStep {
                name: caps["name"].trim().to_string(),
                endpoint: caps["path"].trim().to_string(),
                inject: Vec::new(),
                capture: Vec::new(),
                assert: Vec::new(),
            });
            continue;
        }
        let Some(step) = steps.last_mut() else {
            continue;
        };
        if let Some(caps) = capture_re.captures(line) {
            step.capture.push(Capture {
                source: caps["src"].trim().to_string(),
                name: caps["name"].trim().to_string(),
            });
        } else if let Some(caps) = inject_re.captures(line) {
            step.inject.extend(
                var_re
                    .captures_iter(&caps["vars"])
                    .map(|caps| caps["var"].trim().to_string()),
            );
        } else if let Some(caps) = assert_re.captures(line) {
            step.assert.push(caps["assert"].trim().to_string());
        }
    }
    steps
}

fn raw_section(source: &str, name: &str) -> Option<String> {
    let heading = format!("## {name}");
    let mut in_section = false;
    let mut lines = Vec::new();
    for line in source.lines() {
        if line.trim() == heading {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if in_section {
            lines.push(line);
        }
    }
    in_section.then(|| lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint_doc() -> &'static str {
        r#"---
resource: users
protocol: http
method: GET
path: /users/:id
tags: [users, read]
version: 1
env: [dev]
auth: bearer
timeout: 5000
retry:
  attempts: 1
  backoff: fixed
---
# Get user by id

Fetches a user.

## Request

```http
GET {{baseUrl}}/users/:id
Authorization: Bearer {{authToken}}
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{"id":"{{id}}"}
```

## Tests

```agent-task
- Verify status.
```
"#
    }

    #[test]
    fn parses_endpoint() {
        let endpoint = parse_endpoint(endpoint_doc(), "api-docs/users/get-user.md").unwrap();
        assert_eq!(endpoint.schema.resource, "users");
        assert_eq!(endpoint.title, "Get user by id");
        assert!(endpoint.request.contains("GET"));
        assert!(endpoint.tests.is_some());
    }

    #[test]
    fn requires_frontmatter() {
        let err = parse_endpoint("# Missing", "bad.md").unwrap_err();
        assert!(matches!(err, ParseError::MissingFrontmatter { .. }));
    }

    #[test]
    fn rejects_multiple_request_http_blocks() {
        let source = endpoint_doc().replace(
            "Authorization: Bearer {{authToken}}\n```",
            "Authorization: Bearer {{authToken}}\n```\n\n```http\nGET /other\n```",
        );
        let err = parse_endpoint(&source, "bad.md").unwrap_err();
        assert!(matches!(err, ParseError::MultipleHttpBlocks { .. }));
    }

    #[test]
    fn parses_pipeline_steps() {
        let source = r#"---
type: pipeline
name: user-onboarding
continue-on-error: false
parallel: false
---
# User onboarding

## Steps

1. **Create user** -> `users/create-user.md`
   - Capture: `response.body.id` as `userId`
2. **Login** -> `users/login.md`
   - Inject: `userId`
   - Assert: `response.status == 200`
"#;
        let pipeline = parse_pipeline(source, "api-docs/pipelines/user.md").unwrap();
        assert_eq!(pipeline.steps.len(), 2);
        assert_eq!(pipeline.steps[0].capture[0].name, "userId");
        assert_eq!(pipeline.steps[1].inject, vec!["userId"]);
    }

    #[test]
    fn env_parser_rejects_secret() {
        let err = parse_env_config(
            r#"# Environments

## dev

```yaml
baseUrl: https://example.com
token: sk_test_123
```
"#,
            "api-docs/_shared/env.md",
        )
        .unwrap_err();
        assert!(matches!(err, ParseError::SecretDetected { .. }));
    }

    #[test]
    fn env_parser_reads_values() {
        let env = parse_env_config(
            r#"# Environments

## dev

```yaml
baseUrl: https://example.com
retries: 2
```
"#,
            "api-docs/_shared/env.md",
        )
        .unwrap();
        assert_eq!(
            env.envs["dev"]["baseUrl"],
            "https://example.com".to_string()
        );
        assert_eq!(env.envs["dev"]["retries"], "2");
    }

    #[test]
    fn reports_missing_request_section() {
        let source = endpoint_doc().replace("## Request", "## Example");
        let err = parse_endpoint(&source, "bad.md").unwrap_err();
        assert!(matches!(err, ParseError::MissingSection { .. }));
    }

    #[test]
    fn reports_invalid_yaml() {
        let source = endpoint_doc().replace("resource: users", "resource: [");
        let err = parse_endpoint(&source, "bad.md").unwrap_err();
        assert!(matches!(err, ParseError::InvalidYaml { .. }));
    }

    #[test]
    fn reports_invalid_pipeline_type() {
        let source = r#"---
type: workflow
name: demo
---
# Demo

## Steps
"#;
        let err = parse_pipeline(source, "pipeline.md").unwrap_err();
        assert!(matches!(err, ParseError::Invalid { .. }));
    }

    #[test]
    fn parses_notes_section() {
        let source = format!("{}\n## Notes\n\nRemember this.", endpoint_doc());
        let endpoint = parse_endpoint(&source, "endpoint.md").unwrap();
        assert_eq!(endpoint.notes.unwrap(), "Remember this.");
    }
}
