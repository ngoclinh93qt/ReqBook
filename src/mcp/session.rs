//! Session persistence and context helpers.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    parser::parse_env_config,
    resolver::{Context, SourceKind},
};

use super::util::collection_root_for;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct Session {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) env: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub(super) vars: std::collections::BTreeMap<String, String>,
}

pub(super) fn session_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rqb-session.json")
}

pub(super) fn read_session() -> Session {
    std::fs::read_to_string(session_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub(super) fn write_session(session: &Session) {
    let _ = serde_json::to_string_pretty(session)
        .ok()
        .and_then(|s| std::fs::write(session_path(), s).ok());
}

/// Resolve the effective env, preferring explicit arg → session → default "dev".
pub(super) fn resolve_env<'a>(args: &'a Value, session: &'a Session) -> &'a str {
    args.get("env")
        .and_then(|v| v.as_str())
        .or(session.env.as_deref())
        .unwrap_or("dev")
}

/// Build a context for a concrete spec or pipeline path.
///
/// MCP clients should behave like the CLI: env.md, .env.local, and RQB_*/MAD_*
/// are loaded automatically, then session vars and explicit tool vars override.
pub(super) fn build_context_for_path(
    args: &Value,
    session: &Session,
    path: impl AsRef<Path>,
    env: &str,
) -> Context {
    let mut context = Context::default();
    load_env_file(path.as_ref(), env, &mut context);
    load_dotenv_local(path.as_ref(), &mut context);
    load_rqb_env(&mut context);
    load_session_and_args(args, session, &mut context);
    context
}

fn load_session_and_args(args: &Value, session: &Session, context: &mut Context) {
    for (k, v) in &session.vars {
        context.insert(SourceKind::Env, k, v);
    }
    if let Some(vars) = args.get("vars").and_then(|v| v.as_object()) {
        for (k, v) in vars {
            if let Some(val) = v.as_str() {
                context.insert(SourceKind::Cli, k, val);
            }
        }
    }
}

fn load_env_file(path: &Path, env: &str, context: &mut Context) {
    let root = api_docs_root_for_path(path);
    let env_path = root.join("_shared/env.md");
    let Ok(source) = std::fs::read_to_string(&env_path) else {
        return;
    };
    let Ok(config) = parse_env_config(&source, &env_path) else {
        return;
    };
    if let Some(values) = config.envs.get(env) {
        for (key, value) in values {
            context.insert(SourceKind::Env, key, value);
        }
    }
}

fn load_dotenv_local(path: &Path, context: &mut Context) {
    let root = api_docs_root_for_path(path);
    let Some(project_root) = root.parent() else {
        return;
    };
    let dotenv = project_root.join(".env.local");
    let Ok(source) = std::fs::read_to_string(dotenv) else {
        return;
    };
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            context.insert(
                SourceKind::DotEnvLocal,
                key.trim(),
                value.trim().trim_matches('"').trim_matches('\''),
            );
        }
    }
}

fn api_docs_root_for_path(path: &Path) -> std::path::PathBuf {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    for ancestor in start.ancestors() {
        if ancestor.join("_shared/env.md").exists()
            || ancestor.file_name().is_some_and(|name| name == "api-docs")
        {
            return ancestor.to_path_buf();
        }
    }
    collection_root_for(path.to_string_lossy().as_ref())
}

fn load_rqb_env(context: &mut Context) {
    for (key, value) in std::env::vars() {
        if let Some(name) = key.strip_prefix("RQB_") {
            context.insert(SourceKind::OsEnv, env_name_to_var(name), value);
        } else if let Some(name) = key.strip_prefix("MAD_") {
            context.insert(SourceKind::OsEnv, env_name_to_var(name), value);
        }
    }
}

fn env_name_to_var(name: &str) -> String {
    let mut parts = name.split('_').filter(|part| !part.is_empty());
    let Some(first) = parts.next() else {
        return String::new();
    };
    let mut out = first.to_ascii_lowercase();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(&chars.as_str().to_ascii_lowercase());
        }
    }
    out
}
