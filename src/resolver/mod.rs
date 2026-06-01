//! Variable resolution and secret masking.

use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Variable source label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    /// Pipeline capture or inject.
    Pipeline,
    /// CLI `--var`.
    Cli,
    /// Endpoint frontmatter.
    Endpoint,
    /// Shared environment markdown.
    Env,
    /// `.env.local`.
    DotEnvLocal,
    /// OS environment variable.
    OsEnv,
}

/// Resolution context with MarkApiDown priority ordering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Context {
    pipeline: BTreeMap<String, String>,
    cli: BTreeMap<String, String>,
    endpoint: BTreeMap<String, String>,
    env: BTreeMap<String, String>,
    dotenv: BTreeMap<String, String>,
    os: BTreeMap<String, String>,
}

impl Context {
    /// Insert a variable into the selected source.
    pub fn insert(&mut self, source: SourceKind, key: impl Into<String>, value: impl Into<String>) {
        let map = match source {
            SourceKind::Pipeline => &mut self.pipeline,
            SourceKind::Cli => &mut self.cli,
            SourceKind::Endpoint => &mut self.endpoint,
            SourceKind::Env => &mut self.env,
            SourceKind::DotEnvLocal => &mut self.dotenv,
            SourceKind::OsEnv => &mut self.os,
        };
        map.insert(key.into(), value.into());
    }

    /// Resolve one variable according to priority.
    pub fn get(&self, key: &str) -> Option<&str> {
        [
            &self.pipeline,
            &self.cli,
            &self.endpoint,
            &self.env,
            &self.dotenv,
            &self.os,
        ]
        .into_iter()
        .find_map(|map| map.get(key).map(String::as_str))
    }

    /// Return variables from one source as owned key-value pairs.
    pub fn entries_for(&self, source: SourceKind) -> Vec<(String, String)> {
        let map = match source {
            SourceKind::Pipeline => &self.pipeline,
            SourceKind::Cli => &self.cli,
            SourceKind::Endpoint => &self.endpoint,
            SourceKind::Env => &self.env,
            SourceKind::DotEnvLocal => &self.dotenv,
            SourceKind::OsEnv => &self.os,
        };
        map.iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

/// Resolution errors.
#[derive(Debug, Error)]
pub enum ResolveError {
    /// A variable is missing.
    #[error("unresolved variable \"{name}\"\nFix: define {name} in .env.local, pass --var {name}=..., or set MAD_{env_name}.")]
    MissingVariable {
        /// Variable name.
        name: String,
        /// Environment variable spelling.
        env_name: String,
    },
    /// A nested variable remains after resolution.
    #[error("nested variable remained after resolution: {name}\nFix: define final values directly; MarkApiDown v1.0 does not recursively resolve variables.")]
    NestedVariable {
        /// Variable name.
        name: String,
    },
    /// Secret was detected in a non-secret source.
    #[error("possible secret detected in {location}\nFix: move this value to .env.local or MAD_* environment variables.")]
    SecretDetected {
        /// Source name.
        location: String,
    },
}

/// Resolve `{{var}}` placeholders in a template.
pub fn resolve(template: &str, ctx: &Context) -> Result<String, ResolveError> {
    let var_re = Regex::new(r"\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}").expect("valid var regex");
    let mut out = String::with_capacity(template.len());
    let mut last = 0;
    for caps in var_re.captures_iter(template) {
        let mat = caps.get(0).expect("whole match exists");
        out.push_str(&template[last..mat.start()]);
        let name = caps.get(1).expect("name match exists").as_str();
        let value = ctx.get(name).ok_or_else(|| ResolveError::MissingVariable {
            name: name.to_string(),
            env_name: to_env_name(name),
        })?;
        out.push_str(value);
        last = mat.end();
    }
    out.push_str(&template[last..]);
    if let Some(caps) = var_re.captures(&out) {
        let name = caps.get(1).expect("name match exists").as_str().to_string();
        return Err(ResolveError::NestedVariable { name });
    }
    Ok(out)
}

/// Detect whether a string contains a likely secret.
pub fn detect_secret(input: &str) -> bool {
    let patterns = [
        r"Bearer\s+eyJ[A-Za-z0-9_-]+",
        r"\b[a-fA-F0-9]{33,}\b",
        r"\b(?:sk_|pk_live_)[A-Za-z0-9_]+\b",
    ];
    patterns.iter().any(|pattern| {
        Regex::new(pattern)
            .expect("valid secret regex")
            .is_match(input)
    })
}

/// Refuse secrets in versioned sources.
pub fn ensure_no_secret(input: &str, source: &str) -> Result<(), ResolveError> {
    if detect_secret(input) {
        Err(ResolveError::SecretDetected {
            location: source.to_string(),
        })
    } else {
        Ok(())
    }
}

/// Mask sensitive values for logs and reports.
pub fn mask(input: &str) -> String {
    let bearer = Regex::new(r"(?i)(Bearer\s+)[^\s\r\n]+").expect("valid bearer mask regex");
    let basic = Regex::new(r"(?i)(Basic\s+)[^\s\r\n]+").expect("valid basic mask regex");
    let token_assign = Regex::new(r"(?i)\b(authToken|token|password|secret)=([^\s&]+)")
        .expect("valid token mask regex");
    let masked = bearer.replace_all(input, "${1}****");
    let masked = basic.replace_all(&masked, "${1}****");
    token_assign.replace_all(&masked, "$1=****").to_string()
}

fn to_env_name(name: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && idx > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_by_priority() {
        let mut ctx = Context::default();
        ctx.insert(SourceKind::OsEnv, "id", "os");
        ctx.insert(SourceKind::Env, "id", "env");
        ctx.insert(SourceKind::Cli, "id", "cli");
        assert_eq!(resolve("{{id}}", &ctx).unwrap(), "cli");
    }

    #[test]
    fn rejects_nested_vars() {
        let mut ctx = Context::default();
        ctx.insert(SourceKind::Cli, "baseUrl", "https://{{host}}");
        let err = resolve("{{baseUrl}}", &ctx).unwrap_err();
        assert!(matches!(err, ResolveError::NestedVariable { .. }));
    }

    #[test]
    fn detects_secrets() {
        assert!(detect_secret("Authorization: Bearer eyJabc"));
        assert!(detect_secret("sk_test_123"));
        assert!(detect_secret("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(!detect_secret("not secret"));
    }

    #[test]
    fn masks_auth() {
        assert_eq!(
            mask("Authorization: Bearer abc123"),
            "Authorization: Bearer ****"
        );
        assert_eq!(mask("token=abc123"), "token=****");
    }
}
