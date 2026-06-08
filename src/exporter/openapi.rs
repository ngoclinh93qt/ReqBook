//! OpenAPI 3.x exporter.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context as AnyhowContext, Result};
use serde_json::{json, Map, Value};

use crate::parser::{parse_endpoint, AuthMode, Endpoint};

/// Export endpoint specs under `api-docs/` as an OpenAPI document.
pub fn export(root: &Path) -> Result<Value> {
    let mut paths = Map::new();
    let mut uses_bearer = false;
    let mut uses_basic = false;

    for file in endpoint_files(root)? {
        let source = std::fs::read_to_string(&file)
            .with_context(|| format!("reading {}", file.display()))?;
        let endpoint = parse_endpoint(&source, &file)
            .with_context(|| format!("parsing {}", file.display()))?;
        let path_key = openapi_path(&endpoint.schema.path);
        let method_key = endpoint.schema.method.as_str().to_ascii_lowercase();
        let operation = operation_for(&endpoint);
        if matches!(endpoint.schema.auth, Some(AuthMode::Bearer)) {
            uses_bearer = true;
        }
        if matches!(endpoint.schema.auth, Some(AuthMode::Basic)) {
            uses_basic = true;
        }
        let path_item = paths
            .entry(path_key)
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(map) = path_item.as_object_mut() {
            map.insert(method_key, operation);
        }
    }

    let mut root_doc = json!({
        "openapi": "3.0.3",
        "info": {
            "title": project_name(root).unwrap_or_else(|| "Reqbook API".to_string()),
            "version": "1.0.0"
        },
        "paths": Value::Object(paths)
    });

    if uses_bearer || uses_basic {
        let mut schemes = Map::new();
        if uses_bearer {
            schemes.insert(
                "BearerAuth".to_string(),
                json!({"type": "http", "scheme": "bearer"}),
            );
        }
        if uses_basic {
            schemes.insert(
                "BasicAuth".to_string(),
                json!({"type": "http", "scheme": "basic"}),
            );
        }
        root_doc["components"] = json!({ "securitySchemes": schemes });
    }

    Ok(root_doc)
}

/// Export as pretty JSON or YAML.
pub fn export_string(root: &Path, as_json: bool) -> Result<String> {
    let value = export(root)?;
    if as_json {
        Ok(serde_json::to_string_pretty(&value)?)
    } else {
        Ok(serde_yaml::to_string(&value)?)
    }
}

fn endpoint_files(root: &Path) -> Result<Vec<PathBuf>> {
    let search_root = if root.join("apis").exists() {
        root.join("apis")
    } else {
        root.to_path_buf()
    };
    let mut files = Vec::new();
    collect_endpoint_files(&search_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_endpoint_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !matches!(name.as_ref(), "_shared" | "flows" | "pipelines") {
                collect_endpoint_files(&path, out)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !matches!(
                name.as_ref(),
                "README.md" | "reqbook.md" | "mad.md" | "env.md" | "env.template.md"
            ) {
                out.push(path);
            }
        }
    }
    Ok(())
}

fn operation_for(endpoint: &Endpoint) -> Value {
    let (status, response_body, content_type) =
        parse_expected_response(&endpoint.expected_response);
    let mut op = Map::new();
    op.insert("summary".to_string(), json!(endpoint.title));
    if !endpoint.description.trim().is_empty() {
        op.insert("description".to_string(), json!(endpoint.description));
    }
    op.insert(
        "operationId".to_string(),
        json!(operation_id(
            endpoint.schema.method.as_str(),
            &endpoint.schema.path,
            &endpoint.title
        )),
    );
    op.insert(
        "tags".to_string(),
        json!(if endpoint.schema.tags.is_empty() {
            vec![endpoint.schema.resource.clone()]
        } else {
            endpoint.schema.tags.clone()
        }),
    );

    let parameters = path_parameters(&endpoint.schema.path);
    if !parameters.is_empty() {
        op.insert("parameters".to_string(), Value::Array(parameters));
    }

    if let Some(request_body) = request_body(&endpoint.request) {
        op.insert("requestBody".to_string(), request_body);
    }

    if matches!(endpoint.schema.auth, Some(AuthMode::Bearer)) {
        op.insert("security".to_string(), json!([{ "BearerAuth": [] }]));
    } else if matches!(endpoint.schema.auth, Some(AuthMode::Basic)) {
        op.insert("security".to_string(), json!([{ "BasicAuth": [] }]));
    }

    let mut response = Map::new();
    response.insert("description".to_string(), json!(http_reason(status)));
    if !response_body.trim().is_empty() {
        let media_type = if content_type.contains("json") {
            "application/json"
        } else {
            "text/plain"
        };
        let example =
            serde_json::from_str::<Value>(&response_body).unwrap_or(Value::String(response_body));
        response.insert(
            "content".to_string(),
            json!({ media_type: { "example": example } }),
        );
    }
    op.insert(
        "responses".to_string(),
        json!({ status.to_string(): response }),
    );

    Value::Object(op)
}

fn parse_expected_response(source: &str) -> (u16, String, String) {
    let mut parts = source.splitn(2, "\n\n");
    let head = parts.next().unwrap_or_default();
    let body = parts.next().unwrap_or_default().to_string();
    let mut status = 200;
    let mut content_type = "application/json".to_string();
    for (idx, line) in head.lines().enumerate() {
        if idx == 0 {
            status = line
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(200);
        } else if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-type") {
                content_type = value.trim().to_string();
            }
        }
    }
    (status, body, content_type)
}

fn request_body(source: &str) -> Option<Value> {
    let (_, body) = source.split_once("\n\n")?;
    if body.trim().is_empty() {
        return None;
    }
    let example =
        serde_json::from_str::<Value>(body).unwrap_or_else(|_| Value::String(body.to_string()));
    Some(json!({
        "required": true,
        "content": {
            "application/json": {
                "example": example
            }
        }
    }))
}

fn path_parameters(path: &str) -> Vec<Value> {
    let re = regex::Regex::new(r":([A-Za-z_][A-Za-z0-9_]*)").expect("valid path param regex");
    re.captures_iter(path)
        .map(|caps| {
            json!({
                "name": caps[1].to_string(),
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
            })
        })
        .collect()
}

fn openapi_path(path: &str) -> String {
    let re = regex::Regex::new(r":([A-Za-z_][A-Za-z0-9_]*)").expect("valid path param regex");
    re.replace_all(path, "{$1}").to_string()
}

fn operation_id(method: &str, path: &str, title: &str) -> String {
    let base = if title.trim().is_empty() { path } else { title };
    let mut out = method.to_ascii_lowercase();
    for part in base
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

fn project_name(root: &Path) -> Option<String> {
    let source = read_project_manifest(root)?;
    for line in source.lines() {
        if let Some(name) = line.strip_prefix("name:") {
            let name = name.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn read_project_manifest(root: &Path) -> Option<String> {
    for filename in ["reqbook.md", "mad.md"] {
        if let Ok(source) = std::fs::read_to_string(root.join(filename)) {
            return Some(source);
        }
    }
    None
}

fn http_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        _ => "Success",
    }
}

#[allow(dead_code)]
fn sorted_map(value: &Value) -> BTreeMap<String, Value> {
    value
        .as_object()
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_colon_params_to_openapi_params() {
        assert_eq!(openapi_path("/users/:userId"), "/users/{userId}");
        assert_eq!(path_parameters("/users/:userId").len(), 1);
    }

    #[test]
    fn operation_id_is_stable() {
        assert_eq!(
            operation_id("GET", "/users/:userId", "Get user"),
            "getGetUser"
        );
    }
}
