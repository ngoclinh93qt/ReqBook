//! Postman Collection v2.1 JSON importer.

use serde_json::Value;

use super::{parse_url, resource_slug, sentence_case, ImportedEndpoint};

/// Parse a Postman v2.1 JSON export.
///
/// Returns `(collection_name, endpoints)`.
pub fn import(source: &str) -> anyhow::Result<(String, Vec<ImportedEndpoint>)> {
    let root: Value =
        serde_json::from_str(source).map_err(|e| anyhow::anyhow!("invalid JSON: {e}"))?;

    let name = root["info"]["name"]
        .as_str()
        .unwrap_or("Untitled")
        .to_string();

    let items = root["item"].as_array().cloned().unwrap_or_default();
    let mut endpoints = Vec::new();
    collect_items(&items, None, &mut endpoints);

    Ok((name, endpoints))
}

/// Recursively walk Postman item tree.
fn collect_items(items: &[Value], folder: Option<&str>, out: &mut Vec<ImportedEndpoint>) {
    for item in items {
        if let Some(sub_items) = item["item"].as_array() {
            // This is a folder
            let folder_name = item["name"].as_str().unwrap_or("resources");
            let slug = resource_slug(folder_name);
            collect_items(sub_items, Some(&slug), out);
        } else if item["request"].is_object() || item["request"].is_string() {
            // This is a request item
            let resource = folder
                .map(|s| s.to_string())
                .unwrap_or_else(|| "resources".to_string());
            if let Some(ep) = convert_item(item, &resource) {
                out.push(ep);
            }
        }
    }
}

fn convert_item(item: &Value, resource: &str) -> Option<ImportedEndpoint> {
    let req = &item["request"];
    let method = req["method"].as_str().unwrap_or("GET").to_uppercase();

    // URL: can be object with "raw" or a plain string
    let raw_url = if req["url"].is_object() {
        req["url"]["raw"].as_str().unwrap_or("/").to_string()
    } else {
        req["url"].as_str().unwrap_or("/").to_string()
    };

    let (mad_path, request_url) = parse_url(&raw_url);

    // Headers
    let headers = req["header"].as_array().cloned().unwrap_or_default();
    let mut header_lines = String::new();
    for h in &headers {
        let disabled = h["disabled"].as_bool().unwrap_or(false);
        if disabled {
            continue;
        }
        let key = h["key"].as_str().unwrap_or_default();
        let val = h["value"].as_str().unwrap_or_default();
        if !key.is_empty() {
            header_lines.push_str(&format!("{key}: {val}\n"));
        }
    }

    // Body
    let body_text = if req["body"]["mode"].as_str() == Some("raw") {
        req["body"]["raw"].as_str().unwrap_or_default().to_string()
    } else {
        String::new()
    };

    // Build request block
    let mut request = format!("{method} {request_url}\n");
    request.push_str(&header_lines);
    if !body_text.is_empty() {
        request.push('\n');
        request.push_str(&body_text);
    }
    let request = request.trim_end().to_string();

    // Expected response: use first saved example or a default
    let expected_response = build_expected_response(item);

    let raw_title = item["name"].as_str().unwrap_or("Untitled");
    let title = sentence_case(raw_title);

    Some(ImportedEndpoint {
        resource: resource.to_string(),
        method,
        path: mad_path,
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

fn build_expected_response(item: &Value) -> String {
    let responses = item["response"].as_array();
    let first = responses.and_then(|arr| arr.first());

    let Some(resp) = first else {
        return default_response();
    };
    if !resp.is_object() {
        return default_response();
    }

    let code = resp["code"].as_u64().unwrap_or(200);
    let status = resp["status"].as_str().unwrap_or("OK");

    // Content-Type from response headers
    let resp_headers = resp["header"].as_array().cloned().unwrap_or_default();
    let content_type = resp_headers
        .iter()
        .find(|h| {
            h["key"]
                .as_str()
                .map(|k| k.eq_ignore_ascii_case("content-type"))
                .unwrap_or(false)
        })
        .and_then(|h| h["value"].as_str())
        .unwrap_or("application/json");

    let body_raw = resp["body"].as_str().unwrap_or("{}");
    // Pretty-print if valid JSON, otherwise use as-is
    let body = if let Ok(v) = serde_json::from_str::<Value>(body_raw) {
        serde_json::to_string_pretty(&v).unwrap_or_else(|_| body_raw.to_string())
    } else {
        body_raw.to_string()
    };

    format!("HTTP/1.1 {code} {status}\nContent-Type: {content_type}\n\n{body}")
}

fn default_response() -> String {
    "HTTP/1.1 200 OK\nContent-Type: application/json\n\n{}".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::render_endpoint;

    #[test]
    fn parses_postman_fixture() {
        let src = include_str!("../../tests/fixtures/postman.json");
        let (name, endpoints) = import(src).unwrap();
        assert_eq!(name, "Demo API");
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].resource, "users");
        assert_eq!(endpoints[0].method, "GET");
        assert!(endpoints[0].path.contains(":userId"));
    }

    #[test]
    fn generated_file_passes_validate() {
        let src = include_str!("../../tests/fixtures/postman.json");
        let (_, endpoints) = import(src).unwrap();
        for ep in &endpoints {
            let md = render_endpoint(ep);
            crate::parser::parse_endpoint(&md, "test.md").expect("generated file should be valid");
        }
    }
}
