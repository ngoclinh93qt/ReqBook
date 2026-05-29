//! Workspace and collection root detection.
//!
//! A "collection" is an `api-docs/` directory with a `mad.md` manifest.
//! Resolution priority:
//!   1. Explicit `--config` path (unchanged from prior behaviour).
//!   2. Git repo root: `git rev-parse --show-toplevel` → `<root>/api-docs/`.
//!   3. Global default: `~/.mad/workspace/default/api-docs/`.

use std::path::{Path, PathBuf};

/// Returns the `api-docs/` directory for the current context.
///
/// Pass `explicit` when `--config <path>` was provided; pass `None` for
/// auto-detection (git repo root → global default).
pub fn collection_root(explicit_config: Option<&Path>) -> PathBuf {
    if let Some(p) = explicit_config {
        return p.parent().unwrap_or(p).to_path_buf();
    }
    if let Some(root) = git_root() {
        return root.join("api-docs");
    }
    global_default_dir()
}

/// `~/.mad/workspace/default/api-docs/`   used when not in a git repo.
pub fn global_default_dir() -> PathBuf {
    home_dir().join(".mad/workspace/default/api-docs")
}

/// `~/.mad/workspace/scratch/api-docs/`   unsaved ad-hoc requests.
pub fn scratch_dir() -> PathBuf {
    home_dir().join(".mad/workspace/scratch/api-docs")
}

/// Ensure the scratch workspace exists and has a minimal `mad.md`.
pub fn ensure_scratch_workspace() -> std::io::Result<PathBuf> {
    let dir = scratch_dir();
    std::fs::create_dir_all(dir.join("apis/scratch"))?;
    let config = dir.join("mad.md");
    if !config.exists() {
        std::fs::write(
            &config,
            "---\nname: scratch\nversion: 1\ndefault-env: dev\n---\n# Scratch\n\nUnsaved ad-hoc requests.\n",
        )?;
    }
    Ok(dir)
}

fn git_root() -> Option<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8(out.stdout).ok()?;
        Some(PathBuf::from(s.trim()))
    } else {
        None
    }
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
}
