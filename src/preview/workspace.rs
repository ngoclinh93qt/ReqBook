//! Workspace management and git integration handlers.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};

use crate::workspace::{self, WorkspaceEntry};

use super::{
    types::{
        CheckoutBranchBody, CreateWorkspaceBody, GitBranchEntry, GitBranchesResponse,
        OpenWorkspaceBody,
    },
    AppState,
};

// ─── Workspace handlers ───────────────────────────────────────────────────────

pub(super) async fn workspace_current_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let root = state.current_root();
    let name = workspace::workspace_name(&root).unwrap_or_else(|| {
        root.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned())
    });
    Json(WorkspaceEntry {
        path: root.to_string_lossy().into_owned(),
        name,
        last_opened: None,
    })
}

pub(super) async fn workspace_recent_handler() -> impl IntoResponse {
    Json(workspace::load_history())
}

pub(super) async fn workspace_all_handler() -> impl IntoResponse {
    Json(workspace::list_all_workspaces())
}

pub(super) async fn workspace_open_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<OpenWorkspaceBody>,
) -> impl IntoResponse {
    let new_root = PathBuf::from(&body.path);
    if !new_root.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "path does not exist"})),
        )
            .into_response();
    }
    let name = workspace::workspace_name(&new_root).unwrap_or_else(|| {
        new_root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| body.path.clone())
    });
    *state.root.write().unwrap() = new_root.clone();
    workspace::save_to_history(&new_root, &name);
    Json(serde_json::json!({"status": "ok", "name": name})).into_response()
}

pub(super) async fn workspace_create_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateWorkspaceBody>,
) -> impl IntoResponse {
    let dir = PathBuf::from(&body.path);
    let name = body.name.unwrap_or_else(|| {
        dir.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "my-api".to_string())
    });
    if let Err(e) = workspace::init_workspace_dir(&dir, &name) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }
    *state.root.write().unwrap() = dir.clone();
    workspace::save_to_history(&dir, &name);
    Json(serde_json::json!({"status": "ok", "name": name})).into_response()
}

// ─── Git handlers ─────────────────────────────────────────────────────────────

pub(super) async fn git_branches_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match git_branches_for_workspace(&state.current_root()) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

pub(super) async fn git_checkout_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CheckoutBranchBody>,
) -> impl IntoResponse {
    let target = body.branch.trim();
    if target.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "branch is required"})),
        )
            .into_response();
    }

    let workspace_root = state.current_root();
    let Some(repo_root) = git_repo_root(&workspace_root) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "workspace is not inside a git repository"})),
        )
            .into_response();
    };

    let branch_list = match git_branches_for_root(&repo_root) {
        Ok(response) => response,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        }
    };

    let Some(branch) = branch_list
        .branches
        .iter()
        .find(|branch| branch.name == target)
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("unknown branch: {target}")})),
        )
            .into_response();
    };

    let result = if branch.current {
        Ok(String::new())
    } else if branch.remote {
        run_git(&repo_root, &["switch", "--track", &branch.name])
    } else {
        run_git(&repo_root, &["switch", "--", &branch.name])
    };

    match result {
        Ok(_) => match git_branches_for_root(&repo_root) {
            Ok(response) => Json(response).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response(),
        },
        Err(e) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": e}))).into_response(),
    }
}

// ─── Git helpers ──────────────────────────────────────────────────────────────

fn git_branches_for_workspace(root: &Path) -> std::result::Result<GitBranchesResponse, String> {
    let Some(repo_root) = git_repo_root(root) else {
        return Ok(GitBranchesResponse {
            is_repo: false,
            root: None,
            current: None,
            dirty: false,
            branches: Vec::new(),
        });
    };
    git_branches_for_root(&repo_root)
}

fn git_branches_for_root(repo_root: &Path) -> std::result::Result<GitBranchesResponse, String> {
    let current = git_current_branch(repo_root)?;
    let dirty = git_is_dirty(repo_root)?;
    let output = run_git(
        repo_root,
        &[
            "for-each-ref",
            "--format=%(refname)%09%(refname:short)%09%(upstream:short)%09%(HEAD)%09%(objectname:short)%09%(subject)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;

    let mut locals = std::collections::HashSet::new();
    let mut parsed = Vec::new();
    for line in output.lines() {
        let mut parts = line.splitn(6, '\t');
        let full_ref = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default().trim();
        let upstream = parts.next().unwrap_or_default().trim();
        let marker = parts.next().unwrap_or_default().trim();
        let commit = parts.next().unwrap_or_default().trim();
        let summary = parts.next().unwrap_or_default().trim();
        if name.is_empty() || full_ref.ends_with("/HEAD") {
            continue;
        }
        let remote = full_ref.starts_with("refs/remotes/");
        if !remote {
            locals.insert(name.to_string());
        }
        parsed.push(GitBranchEntry {
            name: name.to_string(),
            current: marker == "*",
            remote,
            upstream: non_empty(upstream),
            commit: non_empty(commit),
            summary: non_empty(summary),
        });
    }

    let mut branches: Vec<_> = parsed
        .into_iter()
        .filter(|branch| {
            if !branch.remote {
                return true;
            }
            remote_local_name(&branch.name)
                .map(|local| !locals.contains(local))
                .unwrap_or(true)
        })
        .collect();
    branches.sort_by(|a, b| {
        b.current
            .cmp(&a.current)
            .then_with(|| a.remote.cmp(&b.remote))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(GitBranchesResponse {
        is_repo: true,
        root: Some(repo_root.to_string_lossy().into_owned()),
        current,
        dirty,
        branches,
    })
}

pub(super) fn git_repo_root(root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let path = stdout.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn git_current_branch(repo_root: &Path) -> std::result::Result<Option<String>, String> {
    let branch = run_git(repo_root, &["branch", "--show-current"])?;
    let branch = branch.trim();
    if !branch.is_empty() {
        return Ok(Some(branch.to_string()));
    }
    let commit = run_git(repo_root, &["rev-parse", "--short", "HEAD"])?;
    let commit = commit.trim();
    Ok((!commit.is_empty()).then(|| format!("detached@{commit}")))
}

fn git_is_dirty(repo_root: &Path) -> std::result::Result<bool, String> {
    Ok(!run_git(repo_root, &["status", "--porcelain"])?
        .trim()
        .is_empty())
}

fn run_git(repo_root: &Path, args: &[&str]) -> std::result::Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        return String::from_utf8(output.stdout).map_err(|e| e.to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let msg = if !stderr.is_empty() { stderr } else { stdout };
    Err(if msg.is_empty() {
        format!("git exited with {}", output.status)
    } else {
        msg
    })
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn remote_local_name(remote: &str) -> Option<&str> {
    remote.split_once('/').map(|(_, branch)| branch)
}
