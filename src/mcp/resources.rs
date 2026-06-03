//! MCP Resources protocol handlers.

use std::path::Path;

use serde_json::{json, Value};

use crate::parser::parse_endpoint;

pub(super) fn handle_resources_list() -> Value {
    let root = Path::new("api-docs");
    let mut resources: Vec<Value> = Vec::new();
    if root.exists() {
        collect_resource_uris(root, root, &mut resources);
    }
    json!({ "resources": resources })
}

fn collect_resource_uris(root: &Path, dir: &Path, out: &mut Vec<Value>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();

    for p in paths {
        if p.is_dir() {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if !matches!(name.as_ref(), "_shared" | "flows" | "pipelines") {
                collect_resource_uris(root, &p, out);
            }
        } else if p.extension().is_some_and(|e| e == "md") {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if matches!(
                name.as_ref(),
                "README.md" | "reqbook.md" | "mad.md" | "env.md"
            ) {
                continue;
            }
            let rel = p.strip_prefix(root).unwrap_or(&p);
            let uri = format!("rqb://spec/{}", rel.display());
            let description = std::fs::read_to_string(&p)
                .ok()
                .and_then(|s| parse_endpoint(&s, &p).ok())
                .map(|ep| format!("{} {}", ep.schema.method.as_str(), ep.schema.path))
                .unwrap_or_default();
            out.push(json!({
                "uri":         uri,
                "name":        p.file_name().unwrap_or_default().to_string_lossy(),
                "mimeType":    "text/markdown",
                "description": description,
            }));
        }
    }
}

pub(super) fn handle_resources_read(params: &Value) -> Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Invalid params: uri is required".to_string()))?;

    let rel = uri
        .strip_prefix("rqb://spec/")
        .ok_or_else(|| (-32000, format!("unsupported URI scheme: {uri}")))?;

    let file_path = Path::new("api-docs").join(rel);
    if !file_path.exists() {
        return Err((
            -32000,
            format!("{}: resource not found", file_path.display()),
        ));
    }

    let text = std::fs::read_to_string(&file_path)
        .map_err(|e| (-32000, format!("{}: {e}", file_path.display())))?;

    Ok(json!({
        "contents": [{
            "uri":      uri,
            "mimeType": "text/markdown",
            "text":     text,
        }]
    }))
}
