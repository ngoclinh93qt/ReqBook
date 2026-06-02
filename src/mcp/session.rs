//! Session persistence and context helpers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::resolver::{Context, SourceKind};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct Session {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) env: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub(super) vars: std::collections::BTreeMap<String, String>,
}

pub(super) fn session_path() -> std::path::PathBuf {
    std::env::temp_dir().join("mad-session.json")
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

/// Build a context, merging session vars (lower priority) then explicit vars.
pub(super) fn build_context(args: &Value, session: &Session) -> Context {
    let mut context = Context::default();
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
    context
}
