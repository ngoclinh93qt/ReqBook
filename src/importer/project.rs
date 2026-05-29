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

use anyhow::{Context as _, Result};
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

// ─── Static OpenAPI spec candidates ─────────────────────────────────────────

/// Candidate paths (relative to project root) where a committed OpenAPI/Swagger
/// spec may live.  Checked in order; first match wins.
const STATIC_SPEC_CANDIDATES: &[&str] = &[
    "openapi.yaml",
    "openapi.json",
    "swagger.yaml",
    "swagger.json",
    "docs/swagger.json",              // Gin + swaggo
    "docs/openapi.yaml",
    "docs/openapi.json",
    "api/openapi.yaml",
    "api/openapi.json",
    "spec/openapi.yaml",
    "src/openapi.yaml",
    "src/openapi.json",
    "public/swagger.json",
    "public/openapi.json",
    "storage/api-docs/api-docs.json", // Laravel l5-swagger
];

/// OpenAPI/Swagger endpoint paths to probe on a running server.
const LIVE_SPEC_PATHS: &[&str] = &[
    "/openapi.json",              // FastAPI, Hono, generic
    "/openapi.yaml",
    "/v3/api-docs",               // Spring Boot (springdoc)
    "/v2/api-docs",               // Spring Boot (springfox)
    "/api-json",                  // NestJS
    "/api/json",
    "/swagger/v1/swagger.json",   // ASP.NET Core (Swashbuckle)
    "/api/schema/",               // Django REST (drf-spectacular)
    "/schema/",
    "/documentation/json",        // Fastify
    "/api/documentation/json",    // Laravel
    "/swagger.json",
];

// ─── Framework metadata ───────────────────────────────────────────────────────

/// Detected framework with guidance for obtaining a full OpenAPI spec.
pub struct FrameworkInfo {
    /// Human-readable framework name.
    pub name: &'static str,
    /// CLI command that exports an OpenAPI spec *without* starting an HTTP
    /// server (empty string if none is known).
    pub export_cmd: &'static str,
    /// Default development HTTP ports to probe when checking a running server.
    pub default_ports: Vec<u16>,
    /// Framework-specific OpenAPI paths (probed before the generic defaults).
    pub openapi_paths: Vec<&'static str>,
}

/// What source ultimately produced the imported endpoints.
pub enum ImportSource {
    /// Found a committed OpenAPI/Swagger spec file.
    StaticFile(PathBuf),
    /// Fetched a live spec from a running development server.
    RunningServer(String),
    /// Fell back to regex-based source-code scan; may include a framework hint.
    StaticScan(Option<FrameworkInfo>),
}

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
    // Axum 0.7 wildcard routes use `/*path`; Trellis keeps path params as `:path`.
    let normed = normed.replace("/*", "/:").replace(":*", ":");
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

// ─── Smart import (priority chain) ───────────────────────────────────────────

/// Search `root` for a committed OpenAPI/Swagger spec file.
///
/// Returns the first path that exists and whose content looks like an OpenAPI
/// document.  Also searches `spec/swagger/` recursively (Rails/rswag layout).
pub fn find_static_openapi(root: &Path) -> Option<PathBuf> {
    // Flat candidate list.
    for candidate in STATIC_SPEC_CANDIDATES {
        let path = root.join(candidate);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if content.contains("openapi") || content.contains("swagger") {
                    return Some(path);
                }
            }
        }
    }

    // Rails/rswag: spec/swagger/**/*.yaml|json
    let rswag_dir = root.join("spec/swagger");
    if rswag_dir.is_dir() {
        if let Some(p) = first_spec_in_dir(&rswag_dir, 2) {
            return Some(p);
        }
    }

    None
}

/// Recurse up to `depth` levels under `dir` looking for a YAML/JSON spec file.
fn first_spec_in_dir(dir: &Path, depth: u8) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = first_spec_in_dir(&path, depth - 1) {
                return Some(found);
            }
        } else if path
            .extension()
            .map(|e| e == "yaml" || e == "json")
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

/// Detect the primary web framework used in the project by inspecting manifest
/// files.  Returns `None` if the framework cannot be determined.
pub fn detect_framework(root: &Path) -> Option<FrameworkInfo> {
    // ── JavaScript / TypeScript (package.json) ─────────────────────────────
    if let Ok(raw) = fs::read_to_string(root.join("package.json")) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            let has_dep = |name: &str| -> bool {
                val["dependencies"][name].is_string()
                    || val["devDependencies"][name].is_string()
            };
            if has_dep("@nestjs/core") {
                return Some(FrameworkInfo {
                    name: "NestJS",
                    export_cmd: "",
                    default_ports: vec![3000],
                    openapi_paths: vec!["/api-json", "/api/json", "/openapi.json"],
                });
            }
            if has_dep("fastify") {
                return Some(FrameworkInfo {
                    name: "Fastify",
                    export_cmd: "",
                    default_ports: vec![3000],
                    openapi_paths: vec!["/documentation/json", "/openapi.json"],
                });
            }
            if has_dep("hono") {
                return Some(FrameworkInfo {
                    name: "Hono",
                    export_cmd: "",
                    default_ports: vec![3000, 8787],
                    openapi_paths: vec!["/openapi.json", "/doc"],
                });
            }
            if has_dep("express") {
                return Some(FrameworkInfo {
                    name: "Express",
                    export_cmd: "",
                    default_ports: vec![3000, 8080],
                    openapi_paths: vec!["/openapi.json", "/swagger.json", "/api-docs.json"],
                });
            }
        }
    }

    // ── Python (requirements.txt / pyproject.toml) ─────────────────────────
    let py_content = {
        let mut s = String::new();
        for f in ["requirements.txt", "pyproject.toml", "setup.cfg"] {
            if let Ok(raw) = fs::read_to_string(root.join(f)) {
                s.push_str(&raw.to_lowercase());
            }
        }
        s
    };
    if !py_content.is_empty() {
        if py_content.contains("fastapi") {
            return Some(FrameworkInfo {
                name: "FastAPI",
                export_cmd: concat!(
                    "python -c \"import json; from main import app; ",
                    "print(json.dumps(app.openapi()))\" > openapi.json"
                ),
                default_ports: vec![8000],
                openapi_paths: vec!["/openapi.json"],
            });
        }
        if py_content.contains("djangorestframework") || py_content.contains("django-rest-framework") {
            return Some(FrameworkInfo {
                name: "Django REST Framework",
                export_cmd: "python manage.py spectacular --file openapi.yaml",
                default_ports: vec![8000, 8080],
                openapi_paths: vec!["/api/schema/", "/schema/", "/openapi.json"],
            });
        }
        if py_content.contains("django") {
            return Some(FrameworkInfo {
                name: "Django",
                export_cmd: "python manage.py spectacular --file openapi.yaml  # requires drf-spectacular",
                default_ports: vec![8000],
                openapi_paths: vec!["/api/schema/", "/openapi.json"],
            });
        }
        if py_content.contains("flask") {
            return Some(FrameworkInfo {
                name: "Flask",
                export_cmd: "",
                default_ports: vec![5000],
                openapi_paths: vec!["/openapi.json", "/swagger.json", "/apispec.json"],
            });
        }
        if py_content.contains("litestar") || py_content.contains("starlette") {
            return Some(FrameworkInfo {
                name: "Litestar/Starlette",
                export_cmd: "",
                default_ports: vec![8000],
                openapi_paths: vec!["/schema/openapi.json", "/openapi.json"],
            });
        }
    }

    // ── Rust (Cargo.toml) ─────────────────────────────────────────────────
    if let Ok(raw) = fs::read_to_string(root.join("Cargo.toml")) {
        let lower = raw.to_lowercase();
        if lower.contains("axum") {
            return Some(FrameworkInfo {
                name: "Axum",
                export_cmd: "",
                default_ports: vec![3000, 8080],
                openapi_paths: vec!["/openapi.json", "/api-doc/openapi.json"],
            });
        }
        if lower.contains("actix-web") {
            return Some(FrameworkInfo {
                name: "Actix-web",
                export_cmd: "",
                default_ports: vec![8080],
                openapi_paths: vec!["/openapi.json", "/swagger-ui/openapi.json"],
            });
        }
        if lower.contains("rocket") {
            return Some(FrameworkInfo {
                name: "Rocket",
                export_cmd: "",
                default_ports: vec![8000],
                openapi_paths: vec!["/openapi.json"],
            });
        }
    }

    // ── Go (go.mod) ──────────────────────────────────────────────────────
    if let Ok(raw) = fs::read_to_string(root.join("go.mod")) {
        if raw.contains("gin-gonic/gin") {
            return Some(FrameworkInfo {
                name: "Gin",
                export_cmd: "swag init  # generates docs/swagger.json",
                default_ports: vec![8080],
                openapi_paths: vec!["/swagger/doc.json", "/openapi.json"],
            });
        }
        if raw.contains("labstack/echo") {
            return Some(FrameworkInfo {
                name: "Echo",
                export_cmd: "",
                default_ports: vec![8080, 1323],
                openapi_paths: vec!["/openapi.json", "/swagger.json"],
            });
        }
        if raw.contains("gofiber/fiber") {
            return Some(FrameworkInfo {
                name: "Fiber",
                export_cmd: "",
                default_ports: vec![3000, 8080],
                openapi_paths: vec!["/openapi.json", "/swagger.json"],
            });
        }
        if raw.contains("go-chi/chi") {
            return Some(FrameworkInfo {
                name: "Chi",
                export_cmd: "",
                default_ports: vec![8080, 3000],
                openapi_paths: vec!["/openapi.json"],
            });
        }
        // Generic Go project
        return Some(FrameworkInfo {
            name: "Go",
            export_cmd: "swag init  # for Gin/swaggo → trellis import openapi docs/swagger.json",
            default_ports: vec![8080, 3000],
            openapi_paths: vec!["/openapi.json", "/swagger/doc.json"],
        });
    }

    // ── Java / Kotlin (pom.xml / build.gradle) ────────────────────────────
    if root.join("pom.xml").exists()
        || root.join("build.gradle").exists()
        || root.join("build.gradle.kts").exists()
    {
        return Some(FrameworkInfo {
            name: "Spring Boot",
            export_cmd: "mvn springdoc-openapi:generate",
            default_ports: vec![8080],
            openapi_paths: vec!["/v3/api-docs", "/v2/api-docs", "/openapi.json"],
        });
    }

    // ── PHP (composer.json → Laravel) ─────────────────────────────────────
    if let Ok(raw) = fs::read_to_string(root.join("composer.json")) {
        if raw.contains("laravel/framework") {
            return Some(FrameworkInfo {
                name: "Laravel",
                export_cmd: "php artisan l5-swagger:generate  # requires l5-swagger",
                default_ports: vec![8000],
                openapi_paths: vec!["/api/documentation/json", "/openapi.json"],
            });
        }
    }

    // ── Ruby (Gemfile → Rails) ────────────────────────────────────────────
    if let Ok(raw) = fs::read_to_string(root.join("Gemfile")) {
        if raw.contains("rails") {
            return Some(FrameworkInfo {
                name: "Rails",
                export_cmd: "rake rswag:specs:swaggerize  # requires rswag",
                default_ports: vec![3000],
                openapi_paths: vec!["/api-docs/v1/swagger.yaml", "/openapi.json"],
            });
        }
    }

    // ── C# (*.csproj → ASP.NET Core) ─────────────────────────────────────
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .map(|e| e == "csproj")
                .unwrap_or(false)
            {
                return Some(FrameworkInfo {
                    name: "ASP.NET Core",
                    export_cmd: "",
                    default_ports: vec![5000, 7000, 5001],
                    openapi_paths: vec!["/swagger/v1/swagger.json", "/openapi.json"],
                });
            }
        }
    }

    None
}

/// Probe a running local development server for an OpenAPI spec.
///
/// Tries each port × path combination with a short timeout.  Returns
/// `(url, spec_content)` for the first combination that returns a successful
/// response containing OpenAPI/Swagger content.
pub async fn try_running_server(
    ports: &[u16],
    framework_paths: &[&'static str],
) -> Option<(String, String)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;

    // Framework-specific paths first, then generic defaults.
    let mut paths: Vec<&str> = framework_paths.to_vec();
    for &p in LIVE_SPEC_PATHS {
        if !paths.contains(&p) {
            paths.push(p);
        }
    }

    for &port in ports {
        for &path in &paths {
            let url = format!("http://localhost:{port}{path}");
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        if text.contains("openapi") || text.contains("swagger") {
                            return Some((url, text));
                        }
                    }
                }
            }
        }
    }
    None
}

/// Smart project import   runs the following priority chain and returns the
/// first strategy that yields results:
///
/// 1. **Explicit URL** (`url_hint`)   fetch spec directly, no scan needed.
/// 2. **Static spec file**   look for a committed `openapi.yaml` / `swagger.json`
///    in common project locations.
/// 3. **Running dev server**   probe localhost on framework-specific and common
///    ports for a live OpenAPI endpoint.
/// 4. **Static code scan**   regex-based route detection (existing behaviour),
///    with a framework hint printed to guide the user towards a better import.
pub async fn smart_import(
    root: &Path,
    port_hint: Option<u16>,
    url_hint: Option<&str>,
) -> Result<(String, Vec<ImportedEndpoint>, ImportSource)> {
    // ── 1. Explicit URL ───────────────────────────────────────────────────
    if let Some(url) = url_hint {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .danger_accept_invalid_certs(true)
            .build()?;
        let text = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("fetching {url}"))?
            .text()
            .await
            .with_context(|| format!("reading response from {url}"))?;
        let (name, endpoints) = crate::importer::openapi::import(&text)
            .with_context(|| format!("parsing OpenAPI spec from {url}"))?;
        return Ok((name, endpoints, ImportSource::RunningServer(url.to_string())));
    }

    // ── 2. Static spec file ───────────────────────────────────────────────
    if let Some(spec_path) = find_static_openapi(root) {
        let content = fs::read_to_string(&spec_path)
            .with_context(|| format!("reading {}", spec_path.display()))?;
        let (name, endpoints) = crate::importer::openapi::import(&content)
            .with_context(|| format!("parsing {}", spec_path.display()))?;
        return Ok((name, endpoints, ImportSource::StaticFile(spec_path)));
    }

    // ── 3. Running dev server ─────────────────────────────────────────────
    let framework = detect_framework(root);

    let mut ports: Vec<u16> = Vec::new();
    if let Some(p) = port_hint {
        ports.push(p);
    }
    if let Some(fw) = &framework {
        for &p in &fw.default_ports {
            if !ports.contains(&p) {
                ports.push(p);
            }
        }
    }
    for p in [8000u16, 8080, 3000, 5000, 4000, 8888, 1323] {
        if !ports.contains(&p) {
            ports.push(p);
        }
    }

    let fw_paths: Vec<&'static str> = framework
        .as_ref()
        .map(|fw| fw.openapi_paths.clone())
        .unwrap_or_default();

    if let Some((url, content)) = try_running_server(&ports, &fw_paths).await {
        let (name, endpoints) = crate::importer::openapi::import(&content)
            .with_context(|| format!("parsing OpenAPI spec from {url}"))?;
        return Ok((name, endpoints, ImportSource::RunningServer(url)));
    }

    // ── 4. Static code scan (fallback) ────────────────────────────────────
    let (name, endpoints) = import(root)?;
    Ok((name, endpoints, ImportSource::StaticScan(framework)))
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
