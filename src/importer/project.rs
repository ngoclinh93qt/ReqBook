//! Scan a project's source code for API route definitions and produce Trellis
//! endpoint specs.
//!
//! Supports: Express/Fastify (JS/TS), FastAPI/Flask/Django (Python),
//! Axum/Actix (Rust), Gin/Chi/Echo/net-http (Go), Spring Boot (Java/Kotlin),
//! Laravel (PHP), Rails (Ruby), ASP.NET Core (C#).

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use regex::Regex;

use super::{convert_path_params, resource_slug, ImportedEndpoint};
use crate::importer::curl::title_from_path;

// ─── Directory skip list ──────────────────────────────────────────────────────

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    "vendor",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".svelte-kit",
    "out",
    ".cache",
    "coverage",
    ".tox",
    "venv",
    ".venv",
    "env",
    ".env",
    "api-docs",
];

// ─── Route pattern ────────────────────────────────────────────────────────────

struct RoutePattern {
    /// File extensions this pattern applies to (without leading dot).
    extensions: &'static [&'static str],
    /// Compiled regex. Group 1 = method (or path if method unknown), Group 2 = path (or empty).
    regex: Regex,
    /// Which capture group holds the HTTP method (1-based; 0 = derive from context).
    method_group: usize,
    /// Which capture group holds the URL path (1-based).
    path_group: usize,
    /// Default HTTP method when `method_group` is 0 or the group doesn't match.
    default_method: &'static str,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Scan `root` for API route definitions and return an endpoint per unique
/// `(method, path)` pair.
///
/// Returns `(project_name, endpoints)` matching the importer interface.
pub fn import(root: &Path) -> Result<(String, Vec<ImportedEndpoint>)> {
    let patterns = build_patterns();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut endpoints: Vec<ImportedEndpoint> = Vec::new();

    walk(root, root, &patterns, &mut seen, &mut endpoints);

    // Sort for deterministic output.
    endpoints.sort_by(|a, b| a.path.cmp(&b.path).then(a.method.cmp(&b.method)));

    let name = detect_project_title(root).unwrap_or_else(|| root.display().to_string());
    Ok((name, endpoints))
}

// ─── Walker ───────────────────────────────────────────────────────────────────

fn walk(
    _root: &Path,
    dir: &Path,
    patterns: &[RoutePattern],
    seen: &mut HashSet<(String, String)>,
    out: &mut Vec<ImportedEndpoint>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();

    for path in paths {
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&dir_name) {
                continue;
            }
            walk(_root, &path, patterns, seen, out);
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            scan_file(&path, &ext, patterns, seen, out);
        }
    }
}

fn scan_file(
    path: &Path,
    ext: &str,
    patterns: &[RoutePattern],
    seen: &mut HashSet<(String, String)>,
    out: &mut Vec<ImportedEndpoint>,
) {
    let relevant: Vec<&RoutePattern> = patterns
        .iter()
        .filter(|p| p.extensions.contains(&ext))
        .collect();
    if relevant.is_empty() {
        return;
    }
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    for pattern in relevant {
        for cap in pattern.regex.captures_iter(&source) {
            let raw_path = cap
                .get(pattern.path_group)
                .map(|m| m.as_str())
                .unwrap_or("")
                .trim();
            if raw_path.is_empty() || raw_path.contains("{{") || raw_path.contains("${") {
                // Skip template strings and variable-only paths.
                continue;
            }

            let method = if pattern.method_group > 0 {
                cap.get(pattern.method_group)
                    .map(|m| m.as_str().to_uppercase())
                    .unwrap_or_else(|| pattern.default_method.to_string())
            } else {
                // Try to derive from nearby context (simple heuristic: look for Flask methods=[...]).
                if let Some(methods_cap) = cap.get(pattern.path_group + 1) {
                    // Flask-style: methods=['GET','POST']
                    let methods_str = methods_cap.as_str().to_uppercase();
                    if methods_str.contains("POST") {
                        "POST".to_string()
                    } else if methods_str.contains("PUT") {
                        "PUT".to_string()
                    } else if methods_str.contains("PATCH") {
                        "PATCH".to_string()
                    } else if methods_str.contains("DELETE") {
                        "DELETE".to_string()
                    } else {
                        pattern.default_method.to_string()
                    }
                } else {
                    pattern.default_method.to_string()
                }
            };

            // Normalise path: convert {param} → :param, strip trailing slash.
            let norm_path = normalise_path(raw_path);

            let key = (method.clone(), norm_path.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);

            let resource = resource_slug(&resource_from_path(&norm_path));
            let title = title_from_path(&method, &norm_path);

            out.push(ImportedEndpoint {
                resource,
                method: method.clone(),
                path: norm_path.clone(),
                title,
                description: String::new(),
                request: format!("{method} {{{{baseUrl}}}}{norm_path}"),
                expected_response: "HTTP/1.1 200 OK\nContent-Type: application/json\n\n{}"
                    .to_string(),
                tests: None,
                notes: Some(format!("Imported from: `{}`", path.display())),
                tags: vec![],
            });
        }
    }
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

fn normalise_path(raw: &str) -> String {
    // Ensure leading slash.
    let with_slash = if raw.starts_with('/') {
        raw.to_string()
    } else {
        format!("/{raw}")
    };
    // Convert {param} / <param> / :param styles to :param.
    let normed = convert_path_params(&with_slash);
    // Convert <type:name> (Flask) and <name> to :name.
    let re_angle = Regex::new(r"<(?:[^:>]+:)?([^>]+)>").expect("valid");
    let normed = re_angle.replace_all(&normed, ":$1").into_owned();
    // Strip trailing slash (except root).
    if normed.len() > 1 && normed.ends_with('/') {
        normed.trim_end_matches('/').to_string()
    } else {
        normed
    }
}

fn resource_from_path(path: &str) -> String {
    path.trim_start_matches('/')
        .split('/')
        .next()
        .filter(|s| !s.is_empty() && !s.starts_with(':'))
        .unwrap_or("resources")
        .to_string()
}

/// Try to read a project title from common manifest files.
fn detect_project_title(root: &Path) -> Option<String> {
    // package.json
    if let Ok(raw) = fs::read_to_string(root.join("package.json")) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(n) = val["name"].as_str() {
                let name = n.split('/').next_back().unwrap_or(n);
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    // Cargo.toml
    if let Ok(raw) = fs::read_to_string(root.join("Cargo.toml")) {
        let mut in_pkg = false;
        for line in raw.lines() {
            let t = line.trim();
            if t == "[package]" {
                in_pkg = true;
                continue;
            }
            if t.starts_with('[') {
                in_pkg = false;
            }
            if in_pkg {
                if let Some(rest) = t.strip_prefix("name") {
                    let name = rest
                        .trim()
                        .strip_prefix('=')
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    }
    None
}

// ─── Pattern definitions ──────────────────────────────────────────────────────

fn build_patterns() -> Vec<RoutePattern> {
    vec![
        // ── Node.js / Express / Fastify / Koa ────────────────────────────────
        RoutePattern {
            extensions: &["js", "ts", "mjs", "cjs"],
            regex: Regex::new(
                r#"(?:app|router|server|fastify|koa)\s*\.\s*(get|post|put|patch|delete|head|options)\s*\(\s*['"`]([^'"`\n]+)['"`]"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── FastAPI ──────────────────────────────────────────────────────────
        RoutePattern {
            extensions: &["py"],
            regex: Regex::new(
                r#"@(?:\w+)\s*\.\s*(get|post|put|patch|delete|head|options)\s*\(\s*["']([^"'\n]+)["']"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── Flask route decorator ────────────────────────────────────────────
        RoutePattern {
            extensions: &["py"],
            // Path in group 1; optional methods= in group 2 (used for method derivation).
            regex: Regex::new(
                r#"@\w+\.route\s*\(\s*["']([^"'\n]+)["'](?:[^)]*methods\s*=\s*\[([^\]]*)\])?"#,
            )
            .expect("valid"),
            method_group: 0, // derived from group 2 (methods=[...])
            path_group: 1,
            default_method: "GET",
        },
        // ── Django path() / re_path() ────────────────────────────────────────
        RoutePattern {
            extensions: &["py"],
            regex: Regex::new(
                r#"(?:re_)?path\s*\(\s*r?["']([^"'\n]+)["']"#,
            )
            .expect("valid"),
            method_group: 0,
            path_group: 1,
            default_method: "GET",
        },
        // ── Axum (.route) ────────────────────────────────────────────────────
        RoutePattern {
            extensions: &["rs"],
            regex: Regex::new(
                r#"\.route\s*\(\s*"([^"\n]+)"\s*,\s*(get|post|put|patch|delete|head|options)\s*\("#,
            )
            .expect("valid"),
            method_group: 2,
            path_group: 1,
            default_method: "GET",
        },
        // ── Actix-web attribute macros ────────────────────────────────────────
        RoutePattern {
            extensions: &["rs"],
            regex: Regex::new(
                r#"#\[\s*(get|post|put|patch|delete|head)\s*\(\s*"([^"\n]+)"\s*\)\s*\]"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── Go: Gin / Chi / Echo / Fiber ─────────────────────────────────────
        RoutePattern {
            extensions: &["go"],
            regex: Regex::new(
                r#"\.\s*(GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS)\s*\(\s*"([^"\n]+)""#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── Go: net/http HandleFunc ───────────────────────────────────────────
        RoutePattern {
            extensions: &["go"],
            regex: Regex::new(
                r#"(?:mux|http|mux\w*)\s*\.\s*HandleFunc\s*\(\s*"([^"\n]+)""#,
            )
            .expect("valid"),
            method_group: 0,
            path_group: 1,
            default_method: "GET",
        },
        // ── Spring Boot (@GetMapping etc.) ───────────────────────────────────
        RoutePattern {
            extensions: &["java", "kt"],
            regex: Regex::new(
                r#"@(Get|Post|Put|Patch|Delete|Head)Mapping\s*\(\s*(?:value\s*=\s*)?["']([^"'\n]+)["']"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── Spring @RequestMapping ────────────────────────────────────────────
        RoutePattern {
            extensions: &["java", "kt"],
            regex: Regex::new(
                r#"@RequestMapping\s*\([^)]*(?:value|path)\s*=\s*["']([^"'\n]+)["']"#,
            )
            .expect("valid"),
            method_group: 0,
            path_group: 1,
            default_method: "GET",
        },
        // ── Laravel Route:: ───────────────────────────────────────────────────
        RoutePattern {
            extensions: &["php"],
            regex: Regex::new(
                r#"Route\s*::\s*(get|post|put|patch|delete|head|options)\s*\(\s*['"]([^'"\n]+)['"]"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── Rails DSL ─────────────────────────────────────────────────────────
        RoutePattern {
            extensions: &["rb"],
            regex: Regex::new(
                r#"\b(get|post|put|patch|delete|head)\s+['"]([^'"\n]+)['"]"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── ASP.NET Core [HttpGet("/path")] ───────────────────────────────────
        RoutePattern {
            extensions: &["cs"],
            regex: Regex::new(
                r#"\[Http(Get|Post|Put|Patch|Delete)\s*\(\s*["']([^"'\n]+)["']\s*\)\]"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── Hono (TS/JS edge runtime) ─────────────────────────────────────────
        RoutePattern {
            extensions: &["ts", "js"],
            regex: Regex::new(
                r#"(?:app|router|hono)\s*\.\s*(get|post|put|patch|delete)\s*\(\s*['"`]([^'"`\n]+)['"`]"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
        // ── NestJS @Get / @Post decorators ───────────────────────────────────
        RoutePattern {
            extensions: &["ts"],
            regex: Regex::new(
                r#"@(Get|Post|Put|Patch|Delete)\s*\(\s*['"`]([^'"`\n]*)['"`]\s*\)"#,
            )
            .expect("valid"),
            method_group: 1,
            path_group: 2,
            default_method: "GET",
        },
    ]
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_str(source: &str, ext: &str) -> Vec<ImportedEndpoint> {
        let patterns = build_patterns();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        let dummy = PathBuf::from(format!("dummy.{ext}"));
        scan_file(&dummy, ext, &patterns, &mut seen, &mut out);
        // scan_file reads from disk; write to temp file instead
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(format!("test.{ext}"));
        fs::write(&file, source).unwrap();
        scan_file(&file, ext, &patterns, &mut seen, &mut out);
        out
    }

    #[test]
    fn detects_express_routes() {
        let src = r#"
app.get('/users', listUsers);
app.post('/users', createUser);
router.get('/users/:id', getUser);
"#;
        let routes = scan_str(src, "js");
        let methods: Vec<_> = routes.iter().map(|r| r.method.as_str()).collect();
        let paths: Vec<_> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(methods.contains(&"GET"), "GET missing");
        assert!(methods.contains(&"POST"), "POST missing");
        assert!(paths.contains(&"/users"), "path /users missing");
        assert!(paths.contains(&"/users/:id"), "path /users/:id missing");
    }

    #[test]
    fn detects_fastapi_routes() {
        let src = r#"
@app.get("/items")
async def list_items(): ...

@router.post("/items")
async def create_item(): ...
"#;
        let routes = scan_str(src, "py");
        assert!(routes
            .iter()
            .any(|r| r.method == "GET" && r.path == "/items"));
        assert!(routes
            .iter()
            .any(|r| r.method == "POST" && r.path == "/items"));
    }

    #[test]
    fn detects_axum_routes() {
        let src = r#"
Router::new()
    .route("/health", get(health_handler))
    .route("/api/users", post(create_user))
"#;
        let routes = scan_str(src, "rs");
        assert!(routes
            .iter()
            .any(|r| r.method == "GET" && r.path == "/health"));
        assert!(routes
            .iter()
            .any(|r| r.method == "POST" && r.path == "/api/users"));
    }

    #[test]
    fn detects_spring_mappings() {
        let src = r#"
    @GetMapping("/api/users")
    public List<User> listUsers() {}

    @PostMapping("/api/users")
    public User createUser(@RequestBody User user) {}
"#;
        let routes = scan_str(src, "java");
        assert!(routes
            .iter()
            .any(|r| r.method == "GET" && r.path == "/api/users"));
        assert!(routes
            .iter()
            .any(|r| r.method == "POST" && r.path == "/api/users"));
    }

    #[test]
    fn deduplicates_routes() {
        let src = r#"
app.get('/users', handler1);
app.get('/users', handler2);
"#;
        let routes = scan_str(src, "js");
        let get_users: Vec<_> = routes.iter().filter(|r| r.path == "/users").collect();
        assert_eq!(get_users.len(), 1, "duplicates should be collapsed");
    }

    #[test]
    fn normalise_flask_angle_params() {
        assert_eq!(normalise_path("/users/<int:user_id>"), "/users/:user_id");
        assert_eq!(normalise_path("/items/<item_id>"), "/items/:item_id");
    }

    #[test]
    fn normalise_openapi_braces() {
        assert_eq!(normalise_path("/users/{userId}"), "/users/:userId");
    }

    #[test]
    fn import_on_empty_dir_returns_zero() {
        let dir = tempfile::tempdir().unwrap();
        let (_, eps) = import(dir.path()).unwrap();
        assert!(eps.is_empty());
    }

    #[test]
    fn generated_spec_passes_validate() {
        use crate::importer::render_endpoint;
        use crate::parser::parse_endpoint;

        let dir = tempfile::tempdir().unwrap();
        let src_file = dir.path().join("routes.js");
        fs::write(&src_file, "app.get('/health', handler);").unwrap();

        let (_, eps) = import(dir.path()).unwrap();
        assert_eq!(eps.len(), 1);

        let md = render_endpoint(&eps[0]);
        let spec_path = PathBuf::from("api-docs/health/get-health.md");
        parse_endpoint(&md, &spec_path).expect("generated spec must be valid");
    }
}
