//! Markdown parsing for Reqbook specs.

mod schema;

pub use schema::*;

use std::{collections::BTreeMap, path::Path};

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};
use thiserror::Error;

use crate::resolver::ensure_no_secret;

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
    #[error("{path}: possible secret detected\nFix: move this value to .env.local or RQB_* environment variables.")]
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
    let (expected_response, expected_info) =
        exactly_one_code_with_info(&doc, "Expected response", "http", &path)?;
    let error_responses = optional_many_code(&doc, "Error responses", "http");
    let tests = optional_one_code(&doc, "Tests", "agent-task", &path)?;
    let notes = raw_section(body, "Notes")
        .map(|notes| notes.trim().to_string())
        .filter(|notes| !notes.is_empty());
    let assertions = parse_assertions(doc.section_text("Assertions").as_deref().unwrap_or(""));
    let response_schema = optional_schema_code(&doc, &path)?;
    let response_match = response_match_from_schema_or_fence(&schema, &expected_info);
    let response_ignore = schema
        .response
        .as_ref()
        .map(|response| response.ignore.clone())
        .unwrap_or_default();

    if response_match == ResponseMatchMode::Schema && response_schema.is_none() {
        return Err(ParseError::MissingSection {
            path: path.clone(),
            name: "```json``` in ## Schema".to_string(),
        });
    }

    Ok(Endpoint {
        source: Some(path),
        schema,
        title,
        description,
        request,
        expected_response,
        error_responses,
        response_match,
        response_ignore,
        response_schema,
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
    let Some(rest) = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))
    else {
        return Err(ParseError::MissingFrontmatter {
            path: path.to_string(),
        });
    };
    let Some(end) = rest.find("\n---") else {
        return Err(ParseError::MissingFrontmatter {
            path: path.to_string(),
        });
    };
    let frontmatter = rest[..end].strip_suffix('\r').unwrap_or(&rest[..end]);
    let body_start = end + "\n---".len();
    let body = rest.get(body_start..).unwrap_or_default();
    let body = body
        .strip_prefix("\r\n")
        .or_else(|| body.strip_prefix('\n'))
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
    exactly_one_code_with_info(doc, section, lang, path).map(|(code, _)| code)
}

fn exactly_one_code_with_info(
    doc: &MarkdownDoc,
    section: &str,
    lang: &str,
    path: &str,
) -> Result<(String, String), ParseError> {
    let Some(section_data) = doc.sections.get(section) else {
        return Err(ParseError::MissingSection {
            path: path.to_string(),
            name: format!("## {section}"),
        });
    };
    let blocks = matching_code_blocks(section_data, &[lang]);
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
    let blocks = matching_code_blocks(section_data, &[lang]);
    match blocks.len() {
        0 => Ok(None),
        1 => Ok(Some(blocks[0].0.clone())),
        _ => Err(ParseError::Invalid {
            path: path.to_string(),
            message: format!("multiple {lang} blocks in ## {section}"),
            fix: format!("keep one `{lang}` block in ## {section}"),
        }),
    }
}

fn optional_many_code(doc: &MarkdownDoc, section: &str, lang: &str) -> Vec<String> {
    doc.sections
        .get(section)
        .map(|section_data| {
            matching_code_blocks(section_data, &[lang])
                .into_iter()
                .map(|(code, _)| code)
                .collect()
        })
        .unwrap_or_default()
}

fn optional_schema_code(doc: &MarkdownDoc, path: &str) -> Result<Option<String>, ParseError> {
    let Some(section_data) = doc.sections.get("Schema") else {
        return Ok(None);
    };
    let blocks = matching_code_blocks(section_data, &["json", "json-schema", "schema"]);
    match blocks.len() {
        0 => Ok(None),
        1 => Ok(Some(blocks[0].0.clone())),
        _ => Err(ParseError::Invalid {
            path: path.to_string(),
            message: "multiple schema blocks in ## Schema".to_string(),
            fix: "keep one `json` or `json schema` block in ## Schema".to_string(),
        }),
    }
}

fn matching_code_blocks(section: &Section, langs: &[&str]) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    for (info, values) in &section.code {
        let token = info.split_whitespace().next().unwrap_or("");
        if langs.iter().any(|lang| token.eq_ignore_ascii_case(lang)) {
            blocks.extend(values.iter().cloned().map(|code| (code, info.clone())));
        }
    }
    blocks
}

fn response_match_from_schema_or_fence(
    schema: &EndpointSchema,
    expected_info: &str,
) -> ResponseMatchMode {
    if let Some(mode) = schema
        .response
        .as_ref()
        .and_then(|response| response.match_mode)
    {
        return mode;
    }

    expected_info
        .split_whitespace()
        .skip(1)
        .find_map(|token| match token.to_ascii_lowercase().as_str() {
            "strict" => Some(ResponseMatchMode::Strict),
            "schema" => Some(ResponseMatchMode::Schema),
            "shape" => Some(ResponseMatchMode::Shape),
            _ => None,
        })
        .unwrap_or(ResponseMatchMode::Shape)
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

        let (op, value) = if rest.eq_ignore_ascii_case("exists") {
            (AssertionOp::Exists, None)
        } else if let Some(v) = rest
            .strip_prefix("equals ")
            .or(rest.strip_prefix("equals\t"))
        {
            let v = strip_surrounding_quotes(v.trim());
            (AssertionOp::Equals, Some(v.to_string()))
        } else if let Some(v) = rest
            .strip_prefix("contains ")
            .or(rest.strip_prefix("contains\t"))
        {
            (AssertionOp::Contains, Some(v.trim().to_string()))
        } else if let Some(v) = rest
            .strip_prefix("matches ")
            .or(rest.strip_prefix("matches\t"))
        {
            (AssertionOp::Matches, Some(v.trim().to_string()))
        } else if let Some(v) = rest.strip_prefix("in ").or(rest.strip_prefix("in\t")) {
            let v = v.trim();
            let v = v
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .unwrap_or(v);
            (AssertionOp::In, Some(v.trim().to_string()))
        } else {
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
    let var_re = regex::Regex::new(
        r#"`(?P<quoted>[A-Za-z_][A-Za-z0-9_]*)`|(?P<bare>[A-Za-z_][A-Za-z0-9_]*)"#,
    )
    .expect("valid variable regex");

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
            step.inject
                .extend(var_re.captures_iter(&caps["vars"]).filter_map(|caps| {
                    caps.name("quoted")
                        .or_else(|| caps.name("bare"))
                        .map(|mat| mat.as_str().to_string())
                }));
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
    fn parses_endpoint_with_crlf_frontmatter_and_body() {
        let source = endpoint_doc().replace('\n', "\r\n");
        let endpoint = parse_endpoint(&source, "api-docs/users/get-user.md").unwrap();

        assert_eq!(endpoint.schema.resource, "users");
        assert_eq!(endpoint.title, "Get user by id");
        assert!(endpoint.expected_response.contains("HTTP/1.1 200 OK"));
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
    fn parses_bare_pipeline_inject_variables() {
        let source = r#"---
type: pipeline
name: user-onboarding
---
# User onboarding

## Steps

1. **Create user** -> `users/create-user.md`
   - Capture: `response.body.id` as userId
2. **Login** -> `users/login.md`
   - Inject: userId, sessionId
"#;
        let pipeline = parse_pipeline(source, "api-docs/pipelines/user.md").unwrap();
        assert_eq!(pipeline.steps[1].inject, vec!["userId", "sessionId"]);
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
    fn reads_response_match_from_frontmatter() {
        let source = endpoint_doc().replace(
            "retry:\n  attempts: 1\n  backoff: fixed",
            "retry:\n  attempts: 1\n  backoff: fixed\nresponse:\n  match: strict\n  ignore: [body.id]",
        );
        let endpoint = parse_endpoint(&source, "endpoint.md").unwrap();
        assert_eq!(endpoint.response_match, ResponseMatchMode::Strict);
        assert_eq!(endpoint.response_ignore, vec!["body.id"]);
    }

    #[test]
    fn reads_response_match_from_http_fence() {
        let source = endpoint_doc().replace(
            "```http\nHTTP/1.1 200 OK",
            "```http strict\nHTTP/1.1 200 OK",
        );
        let endpoint = parse_endpoint(&source, "endpoint.md").unwrap();
        assert_eq!(endpoint.response_match, ResponseMatchMode::Strict);
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

    #[test]
    fn parses_bulleted_notes_section() {
        let source = format!(
            "{}\n## Notes\n\n- `reason` must be `damaged`.\n- The server returns `404 not_found`.",
            endpoint_doc()
        );
        let endpoint = parse_endpoint(&source, "endpoint.md").unwrap();
        let notes = endpoint.notes.unwrap();

        assert!(notes.contains("- `reason` must be `damaged`."));
        assert!(notes.contains("- The server returns `404 not_found`."));
    }
}
