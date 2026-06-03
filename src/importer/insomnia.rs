//! Insomnia v4 JSON importer.

use std::collections::HashMap;

use serde_json::Value;

use super::{normalize_variables, parse_url, resource_slug, sentence_case, ImportedEndpoint};

/// Parse an Insomnia v4 JSON export.
///
/// Returns `(workspace_name, endpoints)`.
pub fn import(source: &str) -> anyhow::Result<(String, Vec<ImportedEndpoint>)> {
    let root: Value =
        serde_json::from_str(source).map_err(|e| anyhow::anyhow!("invalid JSON: {e}"))?;

    let resources = root["resources"].as_array().cloned().unwrap_or_default();

    // Collect workspace name
    let name = resources
        .iter()
        .find(|r| r["_type"].as_str() == Some("workspace"))
        .and_then(|r| r["name"].as_str())
        .unwrap_or("Untitled")
        .to_string();

    // Build group id → slug map
    let mut groups: HashMap<String, String> = HashMap::new();
    for r in &resources {
        if r["_type"].as_str() == Some("request_group") {
            let id = r["_id"].as_str().unwrap_or_default().to_string();
            let group_name = r["name"].as_str().unwrap_or("resources");
            groups.insert(id, resource_slug(group_name));
        }
    }

    let mut endpoints = Vec::new();
    for r in &resources {
        if r["_type"].as_str() != Some("request") {
            continue;
        }
        let parent_id = r["parentId"].as_str().unwrap_or_default();
        let resource = groups
            .get(parent_id)
            .cloned()
            .unwrap_or_else(|| "resources".to_string());

        if let Some(ep) = convert_request(r, &resource) {
            endpoints.push(ep);
        }
    }

    Ok((name, endpoints))
}

fn convert_request(r: &Value, resource: &str) -> Option<ImportedEndpoint> {
    let method = r["method"].as_str().unwrap_or("GET").to_uppercase();

    let raw_url = r["url"].as_str().unwrap_or("/");
    let raw_url = normalize_variables(raw_url);
    let (rqb_path, request_url) = parse_url(&raw_url);

    // Headers   Insomnia uses `headers` array with `name`/`value` keys
    let headers = r["headers"].as_array().cloned().unwrap_or_default();
    let mut header_lines = String::new();
    for h in &headers {
        let disabled = h["disabled"].as_bool().unwrap_or(false);
        if disabled {
            continue;
        }
        let key = h["name"].as_str().unwrap_or_default();
        let val = h["value"].as_str().unwrap_or_default();
        if !key.is_empty() {
            header_lines.push_str(&format!("{key}: {val}\n"));
        }
    }

    // Body
    let body_text = r["body"]["text"].as_str().unwrap_or_default().to_string();

    // Build request block
    let mut request = format!("{method} {request_url}\n");
    request.push_str(&header_lines);
    if !body_text.is_empty() {
        request.push('\n');
        request.push_str(&body_text);
    }
    let request = request.trim_end().to_string();

    let raw_title = r["name"].as_str().unwrap_or("Untitled");
    let title = sentence_case(raw_title);

    let expected_response = "HTTP/1.1 200 OK\nContent-Type: application/json\n\n{}".to_string();

    Some(ImportedEndpoint {
        resource: resource.to_string(),
        method,
        path: rqb_path,
        title,
        description: String::new(),
        request,
        expected_response,
        tests: None,
        notes: None,
        tags: Vec::new(),
        auth: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::render_endpoint;

    #[test]
    fn parses_insomnia_fixture() {
        let src = include_str!("../../tests/fixtures/insomnia.json");
        let (name, endpoints) = import(src).unwrap();
        assert_eq!(name, "Demo API");
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].resource, "orders");
        assert_eq!(endpoints[0].method, "GET");
        assert!(endpoints[0].path.contains(":orderId"));
    }

    #[test]
    fn generated_file_passes_validate() {
        let src = include_str!("../../tests/fixtures/insomnia.json");
        let (_, endpoints) = import(src).unwrap();
        for ep in &endpoints {
            let md = render_endpoint(ep);
            crate::parser::parse_endpoint(&md, "test.md").expect("generated file should be valid");
        }
    }
}
