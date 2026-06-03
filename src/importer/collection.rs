//! First-pass local API client collection importer.

use std::path::Path;

use anyhow::{Context as AnyhowContext, Result};
use walkdir::WalkDir;

use crate::resolver::ensure_no_secret;

use super::{parse_url, resource_slug, sentence_case, ImportedEndpoint};

const METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

/// Import a local API client collection directory.
pub fn import_dir(path: &Path) -> Result<(String, Vec<ImportedEndpoint>)> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("API client collection")
        .to_string();
    let mut endpoints = Vec::new();
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let file = entry.path();
        if !file.extension().is_some_and(|ext| ext == "bru") {
            continue;
        }
        let source =
            std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
        ensure_no_secret(&source, &file.display().to_string())?;
        if let Some(endpoint) = parse_bru(&source, file) {
            endpoints.push(endpoint);
        }
    }
    endpoints.sort_by(|a, b| a.path.cmp(&b.path).then(a.method.cmp(&b.method)));
    Ok((name, endpoints))
}

fn parse_bru(source: &str, file: &Path) -> Option<ImportedEndpoint> {
    let meta = block(source, "meta").unwrap_or_default();
    let title_hint = field(&meta, "name").or_else(|| {
        file.file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.replace(['_', '-'].as_slice(), " "))
    });

    let (method, method_block) = METHODS
        .iter()
        .find_map(|method| block(source, method).map(|block| (method.to_uppercase(), block)))?;
    let raw_url = field(&method_block, "url")?;
    let (path, request_url) = parse_url(&raw_url);
    let resource = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|part| !part.is_empty())
        .map(resource_slug)
        .unwrap_or_else(|| "resources".to_string());

    let mut request = format!("{method} {request_url}");
    if let Some(headers) = block(source, "headers") {
        for line in headers.lines().filter_map(header_line) {
            request.push('\n');
            request.push_str(&line);
        }
    }
    if let Some(body) = block(source, "body:json").or_else(|| block(source, "body:text")) {
        let body = body.trim();
        if !body.is_empty() {
            request.push_str("\n\n");
            request.push_str(body);
        }
    }

    let status = if method == "POST" { 201 } else { 200 };
    let reason = if status == 201 { "Created" } else { "OK" };
    let title = title_hint.unwrap_or_else(|| format!("{method} {path}"));

    Some(ImportedEndpoint {
        resource,
        method,
        path,
        title: sentence_case(&title),
        description: "Imported from a local API client collection.".to_string(),
        request,
        expected_response: format!(
            "HTTP/1.1 {status} {reason}\nContent-Type: application/json\n\n{{}}"
        ),
        tests: Some("- Verify the response status and body shape.".to_string()),
        notes: Some(
            "Imported from a local API client collection. Update `## Expected response` after the first real run."
                .to_string(),
        ),
        tags: Vec::new(),
        auth: None,
    })
}

fn block(source: &str, name: &str) -> Option<String> {
    let marker = format!("{name} {{");
    let mut in_block = false;
    let mut lines = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if !in_block && trimmed == marker {
            in_block = true;
            continue;
        }
        if in_block && trimmed == "}" {
            break;
        }
        if in_block {
            lines.push(line.trim_end());
        }
    }
    in_block.then(|| lines.join("\n"))
}

fn field(block: &str, name: &str) -> Option<String> {
    for line in block.lines() {
        let (key, value) = line.trim().split_once(':')?;
        if key.trim() == name {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn header_line(line: &str) -> Option<String> {
    let (name, value) = line.trim().split_once(':')?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() || value.is_empty() {
        None
    } else {
        Some(format!("{name}: {value}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::render_endpoint;

    #[test]
    fn parses_collection_request_file() {
        let source = r#"meta {
  name: Get User
  type: http
}

get {
  url: {{baseUrl}}/users/{{userId}}
  body: none
}

headers {
  Accept: application/json
}
"#;
        let endpoint = parse_bru(source, Path::new("get-user.bru")).unwrap();
        assert_eq!(endpoint.method, "GET");
        assert_eq!(endpoint.path, "/users/:userId");
        let md = render_endpoint(&endpoint);
        crate::parser::parse_endpoint(&md, "test.md").unwrap();
    }
}
