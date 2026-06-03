//! Import `.http` / REST Client request files.

use anyhow::Result;

use crate::resolver::ensure_no_secret;

use super::{parse_url, resource_slug, sentence_case, ImportedEndpoint};

/// Parse a `.http` file into MarkApiDown endpoints.
pub fn import(source: &str) -> Result<(String, Vec<ImportedEndpoint>)> {
    ensure_no_secret(source, ".http request file")?;
    let mut endpoints = Vec::new();
    for block in source.split("\n###") {
        if let Some(endpoint) = parse_block(block) {
            endpoints.push(endpoint);
        }
    }
    Ok((".http requests".to_string(), endpoints))
}

fn parse_block(block: &str) -> Option<ImportedEndpoint> {
    let mut title_hint = None;
    let mut request_line = None;
    let mut request_start = 0usize;
    let lines: Vec<&str> = block.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("# @name ") {
            title_hint = Some(name.trim().to_string());
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let method = parts.next()?;
        let url = parts.next()?;
        if is_method(method) && parts.next().is_none() {
            request_line = Some((method.to_uppercase(), url.to_string()));
            request_start = idx + 1;
            break;
        }
    }

    let (method, raw_url) = request_line?;
    let (path, request_url) = parse_url(&raw_url);
    let resource = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|part| !part.is_empty())
        .map(resource_slug)
        .unwrap_or_else(|| "resources".to_string());
    let mut headers = Vec::new();
    let mut body = Vec::new();
    let mut in_body = false;
    for line in lines.iter().skip(request_start) {
        if !in_body && line.trim().is_empty() {
            in_body = true;
            continue;
        }
        if in_body {
            body.push(*line);
        } else if line.contains(':') {
            headers.push(line.trim().to_string());
        }
    }

    let mut request = format!("{method} {request_url}");
    for header in headers {
        request.push('\n');
        request.push_str(&header);
    }
    let body = body.join("\n").trim().to_string();
    if !body.is_empty() {
        request.push_str("\n\n");
        request.push_str(&body);
    }

    let status = if method == "POST" { 201 } else { 200 };
    let reason = if status == 201 { "Created" } else { "OK" };
    let title = title_hint.unwrap_or_else(|| format!("{method} {path}"));
    Some(ImportedEndpoint {
        resource,
        method,
        path,
        title: sentence_case(&title.replace(['_', '-'].as_slice(), " ")),
        description: "Imported from a .http request file.".to_string(),
        request,
        expected_response: format!(
            "HTTP/1.1 {status} {reason}\nContent-Type: application/json\n\n{{}}"
        ),
        tests: Some("- Verify the response status and body shape.".to_string()),
        notes: Some(
            "Imported from `.http`; update the expected response after the first real run."
                .to_string(),
        ),
        tags: Vec::new(),
        auth: None,
    })
}

fn is_method(method: &str) -> bool {
    matches!(
        method.to_ascii_uppercase().as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::render_endpoint;

    #[test]
    fn parses_two_requests() {
        let source = r#"# @name listUsers
GET {{baseUrl}}/users
Accept: application/json

###
POST {{baseUrl}}/users
Content-Type: application/json

{"email":"ada@example.com"}
"#;
        let (_, endpoints) = import(source).unwrap();
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].path, "/users");
        for endpoint in endpoints {
            let md = render_endpoint(&endpoint);
            crate::parser::parse_endpoint(&md, "test.md").unwrap();
        }
    }
}
