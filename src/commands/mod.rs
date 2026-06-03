//! CLI command implementations and shared helpers.

pub mod check;
pub mod context;
pub mod doctor;
pub mod exec;
pub mod export;
pub mod import;
pub mod init;
pub mod install;
pub mod request;
pub mod serve;
pub mod validate;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context as AnyhowContext, Result};
use reqbook::{
    parser,
    report::{ConsoleReporter, JsonReporter, JunitReporter, MarkdownReporter, Reporter},
    resolver::{Context, SourceKind},
    Execution,
};

use crate::OutputFormat;

// ─── Shared context builders ──────────────────────────────────────────────────

pub(crate) fn context_from_vars(vars: &[String]) -> Result<Context> {
    let mut context = Context::default();
    for var in vars {
        let Some((key, value)) = var.split_once('=') else {
            bail!("{var}: invalid --var\nFix: pass variables as --var key=value.");
        };
        context.insert(SourceKind::Cli, key.trim(), value.trim());
    }
    Ok(context)
}

pub(crate) fn execution_context(path: &Path, env: &str, vars: &[String]) -> Result<Context> {
    let mut context = Context::default();
    load_env_file(path, env, &mut context)?;
    load_dotenv_local(path, &mut context)?;
    load_rqb_env(&mut context);
    let cli_context = context_from_vars(vars)?;
    merge_context(&mut context, cli_context, SourceKind::Cli);
    Ok(context)
}

pub(crate) fn load_env_file(path: &Path, env: &str, context: &mut Context) -> Result<()> {
    let Some(root) = find_api_docs_root(path) else {
        return Ok(());
    };
    let env_path = root.join("_shared/env.md");
    if !env_path.exists() {
        return Ok(());
    }
    let source = read_text(&env_path, "reading environment variables")?;
    let config = parser::parse_env_config(&source, &env_path)?;
    if let Some(values) = config.envs.get(env) {
        for (key, value) in values {
            context.insert(SourceKind::Env, key, value);
        }
    }
    Ok(())
}

pub(crate) fn load_dotenv_local(path: &Path, context: &mut Context) -> Result<()> {
    let Some(root) = find_api_docs_root(path) else {
        return Ok(());
    };
    let Some(project_root) = root.parent() else {
        return Ok(());
    };
    let dotenv = project_root.join(".env.local");
    if !dotenv.exists() {
        return Ok(());
    }
    let source = read_text(&dotenv, "reading .env.local")?;
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
    Ok(())
}

pub(crate) fn load_rqb_env(context: &mut Context) {
    for (key, value) in std::env::vars() {
        if let Some(name) = key.strip_prefix("RQB_") {
            context.insert(SourceKind::OsEnv, env_name_to_var(name), value);
        } else if let Some(name) = key.strip_prefix("MAD_") {
            context.insert(SourceKind::OsEnv, env_name_to_var(name), value);
        }
    }
}

pub(crate) fn merge_context(target: &mut Context, source: Context, kind: SourceKind) {
    for (key, value) in source.entries_for(kind) {
        target.insert(kind, key, value);
    }
}

pub(crate) fn find_api_docs_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    for ancestor in start.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "api-docs") {
            return Some(ancestor.to_path_buf());
        }
    }
    let candidate = Path::new("api-docs");
    candidate.exists().then(|| candidate.to_path_buf())
}

pub(crate) fn env_name_to_var(name: &str) -> String {
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

// ─── Output helpers ───────────────────────────────────────────────────────────

pub(crate) fn print_report(format: OutputFormat, execution: &Execution) -> Result<()> {
    let output = match format {
        OutputFormat::Console => ConsoleReporter.report(execution)?,
        OutputFormat::Junit => JunitReporter.report(execution)?,
        OutputFormat::Json => JsonReporter.report(execution)?,
        OutputFormat::Markdown => MarkdownReporter.report(execution)?,
    };
    println!("{output}");
    Ok(())
}

pub(crate) fn read_text(path: &Path, action: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| {
        format!(
            "{} while {}\nFix: check that the path exists and is readable.",
            path.display(),
            action
        )
    })
}

pub(crate) fn regenerate_index(root: &Path) -> Result<()> {
    let files = if root.exists() {
        validate::markdown_files(root)?
    } else {
        Vec::new()
    };
    let mut lines = vec![
        "# API docs".to_string(),
        String::new(),
        "Generated by `rqb index`. Do not edit by hand.".to_string(),
        String::new(),
    ];
    for file in files {
        if file.file_name().is_some_and(|name| name == "README.md") {
            continue;
        }
        let rel = file.strip_prefix(root).unwrap_or(&file);
        lines.push(format!("- [{}]({})", rel.display(), rel.display()));
    }
    std::fs::write(root.join("README.md"), lines.join("\n"))?;
    println!("indexed: {}", root.join("README.md").display());
    Ok(())
}
