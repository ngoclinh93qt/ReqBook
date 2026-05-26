//! OpenAPI 3.x YAML/JSON importer.

use serde_yaml::Value;

use super::{convert_path_params, resource_slug, sentence_case, ImportedEndpoint};

const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

/// Parse an OpenAPI 3.x YAML or JSON spec.
///
/// Returns `(api_title, endpoints)`.
pub fn import(source: &str) -> anyhow::Result<(String, Vec<ImportedEndpoint>)> {
    let root: Value =
        serde_yaml::from_str(source).map_err(|e| anyhow::anyhow!("invalid YAML/JSON: {e}"))?;

    let title = root["info"]["title"]
        .as_str()
        .unwrap_or("Untitled")
        .to_string();

    let paths = match root["paths"].as_mapping() {
        Some(m) => m.clone(),
        None => return Ok((title, Vec::new())),
    };

    let mut endpoints = Vec::new();

    for (path_key, path_item) in &paths {
        let raw_path = path_key.as_str().unwrap_or("/");
        let trellis_path = convert_path_params(raw_path);

        for method_str in HTTP_METHODS {
            let op = &path_item[method_str];
            if op.is_null() {
                continue;
            }

            let method_upper = method_str.to_uppercase();

            // Resource: first tag or first path segment
            let resource = op["tags"]
                .as_sequence()
                .and_then(|tags| tags.first())
                .and_then(|t| t.as_str())
                .map(resource_slug)
                .unwrap_or_else(|| {
                    trellis_path
                        .trim_start_matches('/')
                        .split('/')
                        .next()
                        .map(resource_slug)
                        .unwrap_or_else(|| "resources".to_string())
                });

            // Title
            let title_str = op["summary"]
                .as_str()
                .map(sentence_case)
                .unwrap_or_else(|| format!("{} {}", method_upper, trellis_path));

            // Description: first line only
            let description = op["description"]
                .as_str()
                .and_then(|d| d.lines().next())
                .unwrap_or_default()
                .to_string();

            // Request block
            let request_url = format!("{{{{baseUrl}}}}{trellis_path}");
            let request = build_request_block(&method_upper, &request_url, op);

            // Expected response
            let expected_response = build_expected_response(op);

            // Generic tests
            let tests = Some("- Verify response status is 200.".to_string());

            endpoints.push(ImportedEndpoint {
                resource,
                method: method_upper,
                path: trellis_path.clone(),
                title: title_str,
                description,
                request,
                expected_response,
                tests,
                notes: None,
                tags: Vec::new(),
            });
        }
    }

    Ok((title, endpoints))
}

fn build_request_block(method: &str, url: &str, op: &Value) -> String {
    let mut out = format!("{method} {url}\n");

    match method {
        "GET" | "DELETE" | "HEAD" | "OPTIONS" => {
            out.push_str("Accept: application/json");
        }
        "POST" | "PUT" | "PATCH" => {
            out.push_str("Content-Type: application/json\n");
            out.push_str("Accept: application/json\n");
            out.push('\n');
            // Try to get a body example from requestBody
            let body = op["requestBody"]["content"]["application/json"]["example"]
                .as_mapping()
                .and_then(|m| {
                    // Convert serde_yaml mapping to JSON pretty string
                    let yaml_val: Value = Value::Mapping(m.clone());
                    serde_json::to_string_pretty(&yaml_val).ok()
                })
                .unwrap_or_else(|| "{}".to_string());
            out.push_str(&body);
        }
        _ => {
            out.push_str("Accept: application/json");
        }
    }

    out.trim_end().to_string()
}

fn build_expected_response(op: &Value) -> String {
    let responses = match op["responses"].as_mapping() {
        Some(m) => m,
        None => return default_response(),
    };

    // Find first 2xx response key
    let two_xx = responses
        .iter()
        .find(|(k, _)| k.as_str().map(|s| s.starts_with('2')).unwrap_or(false));

    let (status_key, resp_val) = match two_xx {
        Some(pair) => pair,
        None => return default_response(),
    };

    let status_code = status_key.as_str().unwrap_or("200");
    let status_text = http_status_text(status_code);

    // Try to get example body from content.application/json.example
    let example = &resp_val["content"]["application/json"]["example"];
    let body = if !example.is_null() {
        serde_json::to_string_pretty(example).unwrap_or_else(|_| "{}".to_string())
    } else {
        "{}".to_string()
    };

    format!("HTTP/1.1 {status_code} {status_text}\nContent-Type: application/json\n\n{body}")
}

fn default_response() -> String {
    "HTTP/1.1 200 OK\nContent-Type: application/json\n\n{}".to_string()
}

fn http_status_text(code: &str) -> &'static str {
    match code {
        "200" => "OK",
        "201" => "Created",
        "202" => "Accepted",
        "204" => "No Content",
        "206" => "Partial Content",
        _ => "OK",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::render_endpoint;

    #[test]
    fn parses_openapi_fixture() {
        let src = include_str!("../../tests/fixtures/openapi.yaml");
        let (name, endpoints) = import(src).unwrap();
        assert_eq!(name, "Demo API");
        assert_eq!(endpoints.len(), 2);
        let get_ep = endpoints
            .iter()
            .find(|ep| ep.method == "GET")
            .expect("should have GET endpoint");
        assert_eq!(get_ep.resource, "products");
        assert!(get_ep.path.contains(":productId"));
    }

    #[test]
    fn generated_file_passes_validate() {
        let src = include_str!("../../tests/fixtures/openapi.yaml");
        let (_, endpoints) = import(src).unwrap();
        for ep in &endpoints {
            let md = render_endpoint(ep);
            crate::parser::parse_endpoint(&md, "test.md").expect("generated file should be valid");
        }
    }
}
