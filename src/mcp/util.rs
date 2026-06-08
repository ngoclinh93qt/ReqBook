//! Shared utilities: error classification, timestamps, HTTP reasons, spec walking.

use std::path::Path;

use crate::{engine::EngineError, resolver::ResolveError};

pub(super) fn classify_engine_error(err: &EngineError) -> (&'static str, Option<&'static str>) {
    match err {
        EngineError::UnsupportedProtocol { .. } => ("UNSUPPORTED_PROTOCOL", None),
        EngineError::Resolve { source, .. } => match source {
            ResolveError::MissingVariable { .. } => (
                "VAR_MISSING",
                Some("Define missing variables in _shared/env.md [<env>] or pass via vars: {...}"),
            ),
            _ => ("VALIDATION_ERROR", None),
        },
        EngineError::Network { .. } => (
            "NETWORK_ERROR",
            Some("Check baseUrl in env.md and ensure the server is running"),
        ),
        EngineError::InvalidRequest { .. } => ("VALIDATION_ERROR", None),
        EngineError::InvalidExpected { .. } => ("VALIDATION_ERROR", None),
        EngineError::Http { .. } => ("VALIDATION_ERROR", None),
        EngineError::UnsupportedEnvironment { .. } => (
            "ENV_NOT_ALLOWED",
            Some("Run with an allowed env from endpoint frontmatter or update env: [...] after review"),
        ),
    }
}

pub(super) fn hint_for_error_type(error_type: &str) -> Option<&'static str> {
    match error_type {
        "VAR_MISSING" => {
            Some("Define missing variables in _shared/env.md [<env>] or pass via vars: {...}")
        }
        "AUTH_FAILED" => Some("Check bearer token or credentials in _shared/env.md"),
        "NETWORK_ERROR" => Some("Check baseUrl in env.md and ensure the server is running"),
        "CONTRACT_MISMATCH" => {
            Some("Update ## Expected response in the spec to match actual, or fix the API")
        }
        "SPEC_PARSE_ERROR" => {
            Some("Fix YAML frontmatter or markdown section structure in the spec file")
        }
        "ENV_NOT_ALLOWED" => Some(
            "Run with an allowed env from endpoint frontmatter or update env: [...] after review",
        ),
        _ => None,
    }
}

pub(super) fn now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn epoch_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let total_min = secs / 60;
    let mi = total_min % 60;
    let total_h = total_min / 60;
    let h = total_h % 24;
    let total_days = total_h / 24;
    let (y, mo, d) = days_to_ymd(total_days);
    (y, mo, d, h, mi, s)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    let mut rem = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if rem < days_in_year {
            break;
        }
        rem -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let months = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u64;
    for days_in_month in months {
        if rem < days_in_month {
            break;
        }
        rem -= days_in_month;
        mo += 1;
    }
    (y, mo, rem + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

pub(super) fn http_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}

/// Find the collection root by walking up from a spec path looking for `_shared/`, `api-docs`, or `apis`.
pub(super) fn collection_root_for(spec_path: &str) -> std::path::PathBuf {
    let p = Path::new(spec_path);
    if let Some(parent) = p.parent() {
        let mut candidate = parent.to_path_buf();
        loop {
            if candidate.join("_shared").exists() {
                return candidate;
            }
            if let Some(name) = candidate.file_name() {
                let n = name.to_string_lossy();
                if n == "api-docs" || n == "apis" {
                    return candidate;
                }
            }
            match candidate.parent() {
                Some(p) => candidate = p.to_path_buf(),
                None => break,
            }
        }
        if let Some(ancestor) = Path::new(spec_path).ancestors().nth(2) {
            return ancestor.to_path_buf();
        }
    }
    Path::new(".").to_path_buf()
}

pub(super) fn spec_rel(spec_path: &str, root: &Path) -> String {
    Path::new(spec_path)
        .strip_prefix(root)
        .unwrap_or_else(|_| Path::new(spec_path))
        .to_string_lossy()
        .to_string()
}

pub(super) fn walk_specs(root: &Path, dir: &Path, mut cb: impl FnMut(&Path, String)) {
    walk_specs_inner(root, dir, &mut cb);
}

fn walk_specs_inner(root: &Path, dir: &Path, cb: &mut impl FnMut(&Path, String)) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if !matches!(name.as_ref(), "_shared" | "flows" | "pipelines") {
                walk_specs_inner(root, &p, cb);
            }
        } else if p.extension().is_some_and(|e| e == "md") {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if matches!(
                name.as_ref(),
                "README.md" | "reqbook.md" | "mad.md" | "env.md" | "env.template.md"
            ) {
                continue;
            }
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            cb(&p, rel);
        }
    }
}
