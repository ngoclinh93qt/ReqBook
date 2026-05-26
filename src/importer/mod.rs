//! Import tools for Postman, Insomnia, OpenAPI specs, raw curl commands,
//! and project source-code route scanning.

pub mod curl;
pub mod insomnia;
pub mod openapi;
pub mod postman;
pub mod project;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

/// A single endpoint ready to be written as a Trellis markdown file.
#[derive(Debug, Clone, Default)]
pub struct ImportedEndpoint {
    pub resource: String,
    pub method: String,
    pub path: String,
    pub title: String,
    pub description: String,
    pub request: String,
    pub expected_response: String,
    pub tests: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
}

/// Write imported endpoints under `root/api-docs/` and return paths written.
/// Never overwrites existing files.
pub fn write_endpoints(root: &Path, endpoints: &[ImportedEndpoint]) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for ep in endpoints {
        let resource = if ep.resource.is_empty() {
            "resources".to_string()
        } else {
            ep.resource.clone()
        };
        let dir = root.join("api-docs").join(&resource);
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating directory {}", dir.display()))?;

        let slug = make_slug(&ep.method, &ep.title);
        let path = dir.join(format!("{slug}.md"));
        if path.exists() {
            continue;
        }
        let content = render_endpoint(ep);
        fs::write(&path, &content).with_context(|| format!("writing {}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

/// Render one `ImportedEndpoint` as a Trellis markdown string.
pub(crate) fn render_endpoint(ep: &ImportedEndpoint) -> String {
    let resource = if ep.resource.is_empty() {
        "resources".to_string()
    } else {
        ep.resource.clone()
    };
    let method = if ep.method.is_empty() {
        "GET".to_string()
    } else {
        ep.method.to_uppercase()
    };
    let path = if ep.path.is_empty() {
        "/".to_string()
    } else {
        ep.path.clone()
    };

    // Build tags list: always include the resource tag; add extras.
    let mut tags: Vec<String> = vec![resource.clone()];
    for t in &ep.tags {
        if t != &resource {
            tags.push(t.clone());
        }
    }
    let tags_str = tags.join(", ");

    let mut out = String::new();

    // Frontmatter
    out.push_str("---\n");
    out.push_str(&format!("resource: {resource}\n"));
    out.push_str("protocol: http\n");
    out.push_str(&format!("method: {method}\n"));
    out.push_str(&format!("path: {path}\n"));
    out.push_str(&format!("tags: [{tags_str}]\n"));
    out.push_str("version: 1\n");
    out.push_str("auth: none\n");
    out.push_str("timeout: 5000\n");
    out.push_str("retry:\n");
    out.push_str("  attempts: 0\n");
    out.push_str("  backoff: fixed\n");
    out.push_str("---\n");

    // Title
    out.push_str(&format!("# {}\n", ep.title));
    out.push('\n');

    // Description
    if !ep.description.is_empty() {
        out.push_str(&ep.description);
        out.push('\n');
        out.push('\n');
    }

    // Request section
    out.push_str("## Request\n");
    out.push('\n');
    out.push_str("```http\n");
    out.push_str(&ep.request);
    out.push('\n');
    out.push_str("```\n");
    out.push('\n');

    // Expected response section
    out.push_str("## Expected response\n");
    out.push('\n');
    out.push_str("```http\n");
    out.push_str(&ep.expected_response);
    out.push('\n');
    out.push_str("```\n");

    // Optional tests section
    if let Some(tests) = &ep.tests {
        out.push('\n');
        out.push_str("## Tests\n");
        out.push('\n');
        out.push_str("```agent-task\n");
        out.push_str(tests);
        out.push('\n');
        out.push_str("```\n");
    }

    // Optional notes section
    if let Some(notes) = &ep.notes {
        out.push('\n');
        out.push_str("## Notes\n");
        out.push('\n');
        out.push_str(notes);
        out.push('\n');
    }

    out
}

pub(crate) fn make_slug(method: &str, title: &str) -> String {
    let prefix = method.to_lowercase();
    let rest = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();
    // Collapse consecutive dashes and strip leading/trailing
    let rest = collapse_dashes(&rest);
    format!("{prefix}-{rest}")
}

fn collapse_dashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !last_dash {
                out.push(c);
            }
            last_dash = true;
        } else {
            out.push(c);
            last_dash = false;
        }
    }
    // Trim leading/trailing dashes
    out.trim_matches('-').to_string()
}

/// Convert path parameter syntax to `:param` style.
/// `{{userId}}` → `:userId`, `{userId}` → `:userId`
pub(crate) fn convert_path_params(path: &str) -> String {
    // Match {{param}} first, then {param}
    let re_double = regex::Regex::new(r"\{\{([^}]+)\}\}").expect("valid regex");
    let re_single = regex::Regex::new(r"\{([^}]+)\}").expect("valid regex");
    let step1 = re_double.replace_all(path, ":$1");
    let step2 = re_single.replace_all(&step1, ":$1");
    step2.into_owned()
}

/// Slugify a name for use as a resource folder name.
pub(crate) fn resource_slug(name: &str) -> String {
    let s = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();
    collapse_dashes(&s)
}

/// Normalize `{{ var }}` to `{{var}}`.
pub(crate) fn normalize_variables(s: &str) -> String {
    let re = regex::Regex::new(r"\{\{\s*([^}]+?)\s*\}\}").expect("valid regex");
    re.replace_all(s, "{{$1}}").into_owned()
}

/// Extract Trellis path and request URL from a raw URL.
/// Returns `(trellis_path, request_url)` where:
/// - trellis_path: `/users/:id`
/// - request_url: `{{baseUrl}}/users/:id`
pub(crate) fn parse_url(raw: &str) -> (String, String) {
    let raw = normalize_variables(raw);
    let path_part: String = if raw.starts_with("{{") {
        // {{baseUrl}}/path... → find first }/ and take from the /
        raw.find("}/")
            .map(|i| raw[i + 1..].to_string())
            .unwrap_or_else(|| "/".to_string())
    } else if let Some(idx) = raw.find("://") {
        let after = &raw[idx + 3..];
        after
            .find('/')
            .map(|i| after[i..].to_string())
            .unwrap_or_else(|| "/".to_string())
    } else if raw.starts_with('/') {
        raw.clone()
    } else {
        raw.find('/')
            .map(|i| raw[i..].to_string())
            .unwrap_or_else(|| "/".to_string())
    };
    let path_only = path_part.split('?').next().unwrap_or("/").to_string();
    let path = convert_path_params(&path_only);
    let request_url = format!("{{{{baseUrl}}}}{path}");
    (path, request_url)
}

pub(crate) fn sentence_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + &chars.as_str().to_lowercase()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_path_params_double_braces() {
        assert_eq!(convert_path_params("/users/{{userId}}"), "/users/:userId");
    }

    #[test]
    fn convert_path_params_single_braces() {
        assert_eq!(convert_path_params("/users/{userId}"), "/users/:userId");
    }

    #[test]
    fn parse_url_with_base_var() {
        let (path, url) = parse_url("{{baseUrl}}/users/{{userId}}");
        assert_eq!(path, "/users/:userId");
        assert_eq!(url, "{{baseUrl}}/users/:userId");
    }

    #[test]
    fn parse_url_with_http_scheme() {
        let (path, url) = parse_url("https://example.com/api/v1/users");
        assert_eq!(path, "/api/v1/users");
        assert_eq!(url, "{{baseUrl}}/api/v1/users");
    }

    #[test]
    fn normalize_variables_trims_spaces() {
        assert_eq!(normalize_variables("{{ baseUrl }}"), "{{baseUrl}}");
    }

    #[test]
    fn resource_slug_lowercases_and_hyphenates() {
        assert_eq!(resource_slug("My Resource"), "my-resource");
    }

    #[test]
    fn make_slug_produces_method_prefix() {
        let slug = make_slug("GET", "Get user by id");
        assert_eq!(slug, "get-get-user-by-id");
    }

    #[test]
    fn sentence_case_capitalizes() {
        assert_eq!(sentence_case("hello world"), "Hello world");
    }
}
