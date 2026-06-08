//! Business logic: spec collection, flow collection, execution, env loading.

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::Result;

use crate::{
    engine::{self, ExecOpts},
    importer::{self, project as project_importer},
    parser::{parse_endpoint, parse_env_config, parse_pipeline, EnvConfig, Pipeline},
    resolver::{Context, SourceKind},
};

use super::types::{
    FlowCaptureResponse, FlowEntry, FlowResponse, FlowStepResponse, ResourceGroup,
    RuntimeOverrides, ScanProjectResponse, ScanRoute, SpecEntry, APIS_DIR, API_DOCS_DIR, FLOWS_DIR,
    LEGACY_FLOWS_DIR,
};

pub(super) fn safe_rel_path(rel_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(rel_path);
    if rel_path.trim().is_empty() || path.is_absolute() {
        return Err("path must be a non-empty relative path".to_string());
    }

    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err("path must not contain `..`".to_string());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("path must be relative".to_string());
            }
        }
    }

    if out.as_os_str().is_empty() {
        return Err("path must be a non-empty relative path".to_string());
    }
    Ok(out)
}

pub(super) fn ensure_api_docs_target(root: &Path, target: &Path) -> Result<(), String> {
    let api_docs = root.join(API_DOCS_DIR);
    let api_docs = api_docs.canonicalize().map_err(|e| {
        format!(
            "api-docs root is not available: {}\nFix: create api-docs/ before saving.",
            e
        )
    })?;

    let checked = if target.exists() {
        target
            .canonicalize()
            .map_err(|e| format!("invalid path: {e}"))?
    } else {
        let parent = target
            .parent()
            .ok_or_else(|| "path must have a parent".to_string())?;
        let parent = parent
            .canonicalize()
            .map_err(|e| format!("invalid parent path: {e}"))?;
        let file_name = target
            .file_name()
            .ok_or_else(|| "path must include a file name".to_string())?;
        parent.join(file_name)
    };

    if !checked.starts_with(&api_docs) {
        return Err("path must stay inside api-docs/".to_string());
    }
    Ok(())
}

pub(super) fn doc_path(root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let rel = safe_rel_path(rel_path)?;
    let api_docs = root.join(API_DOCS_DIR);
    let direct = api_docs.join(&rel);
    if direct.exists() {
        ensure_api_docs_target(root, &direct)?;
        return Ok(direct);
    }
    if let Ok(rest) = rel.strip_prefix(LEGACY_FLOWS_DIR) {
        let modern = api_docs.join(FLOWS_DIR).join(rest);
        if modern.exists() {
            ensure_api_docs_target(root, &modern)?;
            return Ok(modern);
        }
    }
    Ok(direct)
}

pub(super) fn spec_path(root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let rel = safe_rel_path(rel_path)?;
    let api_docs = root.join(API_DOCS_DIR);
    let direct = api_docs.join(&rel);
    if direct.exists() || rel.starts_with(APIS_DIR) {
        if direct.exists() {
            ensure_api_docs_target(root, &direct)?;
        }
        Ok(direct)
    } else {
        let nested = api_docs.join(APIS_DIR).join(&rel);
        if nested.exists() {
            ensure_api_docs_target(root, &nested)?;
            Ok(nested)
        } else {
            Ok(direct)
        }
    }
}

pub(super) fn is_flow_rel_path(rel_path: &str) -> bool {
    rel_path.starts_with(&format!("{FLOWS_DIR}/"))
        || rel_path.starts_with(&format!("{LEGACY_FLOWS_DIR}/"))
}

fn endpoint_roots(root: &Path) -> Vec<PathBuf> {
    let api_docs = root.join(API_DOCS_DIR);
    let apis = api_docs.join(APIS_DIR);
    if apis.exists() {
        vec![apis]
    } else {
        vec![api_docs]
    }
}

fn flow_roots(root: &Path) -> Vec<PathBuf> {
    let api_docs = root.join(API_DOCS_DIR);
    [FLOWS_DIR, LEGACY_FLOWS_DIR]
        .into_iter()
        .map(|dir| api_docs.join(dir))
        .filter(|path| path.exists())
        .collect()
}

pub(super) async fn run_exec(
    file_path: &Path,
    root: &Path,
    env: &str,
    overrides: RuntimeOverrides,
) -> Result<crate::engine::Execution> {
    let source = fs::read_to_string(file_path)?;
    let mut endpoint = parse_endpoint(&source, file_path)?;
    endpoint.request = apply_runtime_overrides(&endpoint.request, &overrides);
    let mut context = load_env_context(root, env);
    for (k, v) in overrides.vars {
        context.insert(SourceKind::Cli, k, v);
    }
    Ok(engine::execute(
        &endpoint,
        env,
        ExecOpts {
            context,
            ..ExecOpts::default()
        },
    )
    .await?)
}

pub(super) fn apply_runtime_overrides(source: &str, overrides: &RuntimeOverrides) -> String {
    if overrides.path_params.is_empty() && overrides.headers.is_empty() && overrides.body.is_none()
    {
        return source.to_string();
    }

    let mut parts = source.splitn(2, "\n\n");
    let head = parts.next().unwrap_or_default();
    let original_body = parts.next().unwrap_or_default();
    let mut lines = head.lines();
    let Some(request_line) = lines.next() else {
        return source.to_string();
    };

    let mut request_parts = request_line.split_whitespace();
    let Some(method) = request_parts.next() else {
        return source.to_string();
    };
    let Some(mut url) = request_parts.next().map(ToOwned::to_owned) else {
        return source.to_string();
    };

    for (name, value) in &overrides.path_params {
        if !value.is_empty() {
            url = url.replace(&format!(":{name}"), value);
        }
    }

    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_string(), value.trim().to_string());
        }
    }
    for (name, value) in &overrides.headers {
        if !name.trim().is_empty() {
            headers.insert(name.trim().to_string(), value.trim().to_string());
        }
    }

    let mut next = format!("{method} {url}");
    for (name, value) in headers {
        next.push('\n');
        next.push_str(&name);
        next.push_str(": ");
        next.push_str(&value);
    }

    let body = overrides.body.as_deref().unwrap_or(original_body);
    if !body.is_empty() {
        next.push_str("\n\n");
        next.push_str(body);
    }
    next
}

pub(super) fn load_env_context(root: &Path, env: &str) -> Context {
    let env_path = root.join("api-docs/_shared/env.md");
    let mut context = Context::default();
    if let Ok(source) = fs::read_to_string(&env_path) {
        if let Ok(config) = parse_env_config(&source, &env_path) {
            if let Some(vars) = config.envs.get(env) {
                for (key, value) in vars {
                    context.insert(SourceKind::Env, key, value);
                }
            }
        }
    }
    context
}

pub(super) fn scan_project(root: &Path, write_missing: bool) -> Result<ScanProjectResponse> {
    let started = std::time::Instant::now();
    let (project_name, endpoints) = project_importer::import(root)?;
    let existing = existing_endpoint_keys(root);
    let mut routes = Vec::with_capacity(endpoints.len());
    let mut missing = Vec::new();

    for endpoint in endpoints {
        let key = endpoint_key(&endpoint.method, &endpoint.path);
        let exists = existing.contains(&key);
        routes.push(ScanRoute {
            method: endpoint.method.clone(),
            path: endpoint.path.clone(),
            title: endpoint.title.clone(),
            resource: endpoint.resource.clone(),
            exists,
        });
        if !exists {
            missing.push(endpoint);
        }
    }

    let written = if write_missing && !missing.is_empty() {
        importer::write_endpoints(root, &missing)?
            .into_iter()
            .map(|path| {
                path.strip_prefix(root.join(API_DOCS_DIR))
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .trim_start_matches(std::path::MAIN_SEPARATOR)
                    .to_string()
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(ScanProjectResponse {
        project_name,
        routes_found: routes.len(),
        missing_count: missing.len(),
        existing_count: routes.iter().filter(|route| route.exists).count(),
        duration_ms: started.elapsed().as_millis(),
        routes,
        written,
    })
}

fn existing_endpoint_keys(root: &Path) -> std::collections::HashSet<(String, String)> {
    let mut keys = std::collections::HashSet::new();
    for dir in endpoint_roots(root) {
        collect_existing_keys(&dir, &mut keys);
    }
    keys
}

fn collect_existing_keys(dir: &Path, keys: &mut std::collections::HashSet<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_existing_keys(&path, keys);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if matches!(
                name,
                "README.md"
                    | "reqbook.md"
                    | "mad.md"
                    | "env.md"
                    | "env.template.md"
                    | "auth.md"
                    | "variables.md"
            ) {
                continue;
            }
            if let Ok(source) = fs::read_to_string(&path) {
                if let Ok(endpoint) = parse_endpoint(&source, &path) {
                    keys.insert(endpoint_key(
                        endpoint.schema.method.as_str(),
                        &endpoint.schema.path,
                    ));
                }
            }
        }
    }
}

fn endpoint_key(method: &str, path: &str) -> (String, String) {
    (
        method.to_uppercase(),
        path.trim_end_matches('/').to_string(),
    )
}

pub(super) fn collect_specs(root: &Path) -> (String, Vec<ResourceGroup>) {
    let api_docs = root.join(API_DOCS_DIR);
    let project_name = read_project_name(&api_docs).unwrap_or_else(|| "API Specs".to_string());
    let mut groups: BTreeMap<String, ResourceGroup> = BTreeMap::new();
    for endpoint_root in endpoint_roots(root) {
        if endpoint_root.exists() {
            collect_recursive(&api_docs, &endpoint_root, &mut groups);
        }
    }
    (project_name, groups.into_values().collect())
}

pub(super) fn collect_flows(root: &Path) -> Vec<FlowEntry> {
    let mut flows = Vec::new();
    for flow_root in flow_roots(root) {
        collect_flows_recursive(&root.join(API_DOCS_DIR), &flow_root, &mut flows);
    }
    flows.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    flows
}

fn collect_flows_recursive(api_docs: &Path, dir: &Path, flows: &mut Vec<FlowEntry>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_flows_recursive(api_docs, &path, flows);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            if let Ok(source) = fs::read_to_string(&path) {
                if let Ok(flow) = parse_pipeline(&source, &path) {
                    let rel = path
                        .strip_prefix(api_docs)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();
                    flows.push(FlowEntry {
                        name: flow.schema.name,
                        title: flow.title,
                        rel_path: rel,
                        steps: flow.steps.len(),
                    });
                }
            }
        }
    }
}

pub(super) fn flow_to_response(
    flow: Pipeline,
    raw_source: String,
    rel_path: String,
) -> FlowResponse {
    FlowResponse {
        name: flow.schema.name,
        title: flow.title,
        description: flow.schema.description,
        rel_path,
        raw_source,
        steps: flow
            .steps
            .into_iter()
            .map(|step| FlowStepResponse {
                name: step.name,
                endpoint: step.endpoint,
                inject: step.inject,
                capture: step
                    .capture
                    .into_iter()
                    .map(|capture| FlowCaptureResponse {
                        source: capture.source,
                        name: capture.name,
                    })
                    .collect(),
                assert: step.assert,
            })
            .collect(),
    }
}

fn collect_recursive(api_docs: &Path, dir: &Path, groups: &mut BTreeMap<String, ResourceGroup>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_recursive(api_docs, &path, groups);
        } else if path.extension().is_some_and(|e| e == "md") {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if matches!(
                name,
                "README.md"
                    | "reqbook.md"
                    | "mad.md"
                    | "env.md"
                    | "env.template.md"
                    | "auth.md"
                    | "variables.md"
            ) {
                continue;
            }
            if let Ok(source) = fs::read_to_string(&path) {
                if let Ok(ep) = parse_endpoint(&source, &path) {
                    let rel_path = path
                        .strip_prefix(api_docs)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();
                    let entry = SpecEntry {
                        method: ep.schema.method.as_str().to_string(),
                        path: ep.schema.path.clone(),
                        title: ep.title.clone(),
                        rel_path,
                    };
                    groups
                        .entry(ep.schema.resource.clone())
                        .or_insert_with(|| ResourceGroup {
                            resource: ep.schema.resource.clone(),
                            specs: Vec::new(),
                        })
                        .specs
                        .push(entry);
                }
            }
        }
    }
}

fn read_project_name(api_docs: &Path) -> Option<String> {
    let source = read_project_manifest(api_docs)?;
    let rest = source.strip_prefix("---\n")?;
    for line in rest.lines() {
        if line == "---" {
            break;
        }
        if let Some(name) = line.strip_prefix("name: ") {
            return Some(name.trim().to_string());
        }
    }
    None
}

fn read_project_manifest(api_docs: &Path) -> Option<String> {
    for filename in ["reqbook.md", "mad.md"] {
        if let Ok(source) = fs::read_to_string(api_docs.join(filename)) {
            return Some(source);
        }
    }
    None
}

pub(super) fn render_env_config(config: &EnvConfig) -> String {
    let mut out = String::from("# Environments\n");
    for (env, vars) in &config.envs {
        out.push_str(&format!("\n## {env}\n\n```yaml\n"));
        for (k, v) in vars {
            let needs_quotes = v.is_empty() || v.starts_with(['{', '[', '#']) || v.contains(": ");
            if needs_quotes {
                let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
                out.push_str(&format!("{k}: \"{escaped}\"\n"));
            } else {
                out.push_str(&format!("{k}: {v}\n"));
            }
        }
        out.push_str("```\n");
    }
    out
}
