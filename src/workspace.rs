//! Workspace and collection root detection.
//!
//! A "collection" is an `api-docs/` directory with a `reqbook.md` manifest.
//! Resolution priority:
//!   1. Explicit `--config` path (unchanged from prior behaviour).
//!   2. Git repo root: `git rev-parse --show-toplevel` → `<root>/api-docs/`.
//!   3. Global default: `~/.rqb/workspace/default/api-docs/`.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const ENV_FILE: &str = "env.md";
pub const ENV_TEMPLATE_FILE: &str = "env.template.md";

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

/// `~/.rqb/workspace/default/api-docs/`   used when not in a git repo.
pub fn global_default_dir() -> PathBuf {
    home_dir().join(".rqb/workspace/default/api-docs")
}

/// `~/.rqb/workspace/scratch/api-docs/`   unsaved ad-hoc requests.
pub fn scratch_dir() -> PathBuf {
    home_dir().join(".rqb/workspace/scratch/api-docs")
}

/// Ensure the scratch workspace exists and has a minimal `reqbook.md`.
pub fn ensure_scratch_workspace() -> std::io::Result<PathBuf> {
    let dir = scratch_dir();
    std::fs::create_dir_all(dir.join("apis/scratch"))?;
    let config = dir.join("reqbook.md");
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

// ─── Workspace management (used by the web preview server and Tauri app) ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub path: String,
    pub name: String,
    pub last_opened: Option<String>,
}

/// All named workspaces under `~/.rqb/workspace/*/api-docs/`.
pub fn list_all_workspaces() -> Vec<WorkspaceEntry> {
    let base = home_dir().join(".rqb/workspace");
    let mut entries = Vec::new();
    let Ok(dir) = fs::read_dir(&base) else {
        return entries;
    };
    for entry in dir.flatten() {
        let api_docs = entry.path().join("api-docs");
        if !api_docs.is_dir() {
            continue;
        }
        let name = workspace_name_from_dir(&api_docs)
            .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
        entries.push(WorkspaceEntry {
            path: entry.path().to_string_lossy().into_owned(),
            name,
            last_opened: None,
        });
    }
    entries
}

/// Load recent workspaces from `~/.rqb/workspaces-history.json`.
pub fn load_history() -> Vec<WorkspaceEntry> {
    let path = home_dir().join(".rqb/workspaces-history.json");
    let Ok(data) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<WorkspaceEntry>>(&data).unwrap_or_default()
}

/// Prepend `path` to the history file (max 10 entries, deduplicated by path).
pub fn save_to_history(path: &Path, name: &str) {
    let mut entries = load_history();
    entries.retain(|e| e.path != path.to_string_lossy());
    entries.insert(
        0,
        WorkspaceEntry {
            path: path.to_string_lossy().into_owned(),
            name: name.to_owned(),
            last_opened: Some(chrono_now()),
        },
    );
    entries.truncate(10);
    let hist = home_dir().join(".rqb/workspaces-history.json");
    if let Some(parent) = hist.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        let _ = fs::write(&hist, json);
    }
}

/// Non-interactive workspace scaffold — equivalent to `rqb init --yes`.
///
/// `dir` is the project root (parent of `api-docs/`). Creates the standard
/// directory structure without interactive prompts.
pub fn init_workspace_dir(dir: &Path, name: &str) -> Result<()> {
    let collection = dir.join("api-docs");
    fs::create_dir_all(collection.join("_shared"))?;
    fs::create_dir_all(collection.join("apis/posts"))?;
    fs::create_dir_all(collection.join("flows"))?;
    write_if_new(&collection.join("reqbook.md"), &project_config(name))?;
    write_if_new(
        &shared_env_template_path(&collection),
        &default_env_config(),
    )?;
    write_if_new(&shared_env_path(&collection), &default_env_config())?;
    write_if_new(
        &collection.join("apis/posts/get-posts.md"),
        EXAMPLE_ENDPOINT,
    )?;
    ensure_env_files_gitignored(&collection)?;
    Ok(())
}

/// Return the `.gitignore` path that protects local environment files for a collection.
pub fn gitignore_path_for_collection(collection: &Path) -> PathBuf {
    project_root_for_collection(collection).join(".gitignore")
}

/// Gitignore entries Reqbook expects for local environment data.
pub fn required_env_gitignore_entries(collection: &Path) -> Vec<String> {
    vec![".env.local".to_string(), env_md_gitignore_entry(collection)]
}

pub fn shared_env_path(collection: &Path) -> PathBuf {
    collection.join("_shared").join(ENV_FILE)
}

pub fn shared_env_template_path(collection: &Path) -> PathBuf {
    collection.join("_shared").join(ENV_TEMPLATE_FILE)
}

pub fn default_env_config() -> String {
    env_config_with_base_url("http://localhost:8080")
}

pub fn env_config_with_base_url(base_url: &str) -> String {
    format!("# Environments\n\n## dev\n\n```yaml\nbaseUrl: {base_url}\npostId: 1\n```\n")
}

/// Return missing local environment entries from the collection project's `.gitignore`.
pub fn missing_env_gitignore_entries(collection: &Path) -> Vec<String> {
    let gitignore = gitignore_path_for_collection(collection);
    let existing = fs::read_to_string(gitignore).unwrap_or_default();
    required_env_gitignore_entries(collection)
        .into_iter()
        .filter(|entry| !gitignore_has_entry(&existing, entry))
        .collect()
}

/// Ensure local environment files generated by Reqbook are ignored by git.
pub fn ensure_env_files_gitignored(collection: &Path) -> Result<Vec<String>> {
    let missing = missing_env_gitignore_entries(collection);
    if missing.is_empty() {
        return Ok(missing);
    }

    let gitignore = gitignore_path_for_collection(collection);
    if let Some(parent) = gitignore.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(&gitignore).unwrap_or_default();
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&gitignore)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file)?;
    }
    for entry in &missing {
        writeln!(file, "{entry}")?;
    }
    Ok(missing)
}

fn project_root_for_collection(collection: &Path) -> PathBuf {
    collection
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn env_md_gitignore_entry(collection: &Path) -> String {
    let project_root = project_root_for_collection(collection);
    let env_path = shared_env_path(collection);
    let relative = if project_root == Path::new(".") {
        env_path
    } else {
        env_path
            .strip_prefix(&project_root)
            .unwrap_or(&env_path)
            .to_path_buf()
    };
    path_to_gitignore_entry(&relative)
}

fn path_to_gitignore_entry(path: &Path) -> String {
    let mut entry = path.to_string_lossy().replace('\\', "/");
    while let Some(rest) = entry.strip_prefix("./") {
        entry = rest.to_string();
    }
    entry
}

fn gitignore_has_entry(existing: &str, entry: &str) -> bool {
    let root_entry = format!("/{entry}");
    existing.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == entry
            || trimmed == root_entry
            || (entry == ".env.local" && trimmed == "**/.env.local")
            || (entry.ends_with("/_shared/env.md") && trimmed == "**/_shared/env.md")
    })
}

fn write_if_new(path: &Path, contents: &str) -> Result<()> {
    if !path.exists() {
        fs::write(path, contents)?;
    }
    Ok(())
}

fn project_config(name: &str) -> String {
    format!(
        "---\nname: {name}\nversion: 1\ndefault-env: dev\n---\n# {name}\n\nAPI specs for {name}.\n\n## Defaults\n\n```yaml\ntimeout: 5000\nretry:\n  attempts: 0\n  backoff: fixed\nauth: none\n```\n"
    )
}

const EXAMPLE_ENDPOINT: &str = r#"---
resource: posts
protocol: http
method: GET
path: /posts/:postId
tags: [posts, read]
version: 1
env: [dev]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Get posts

Fetches one post from the configured development API.

## Request

```http
GET {{baseUrl}}/posts/{{postId}}
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": 1,
  "title": "sunt aut facere repellat provident occaecati excepturi optio reprehenderit",
  "body": "quia et suscipit\nsuscipit recusandae consequuntur expedita et cum\nreprehenderit molestiae ut ut quas totam\nnostrum rerum est autem sunt rem eveniet architecto"
}
```
"#;

/// Read the workspace name from `<root>/api-docs/reqbook.md`.
pub fn workspace_name(root: &Path) -> Option<String> {
    workspace_name_from_dir(&root.join("api-docs"))
}

fn workspace_name_from_dir(api_docs: &Path) -> Option<String> {
    let content = read_project_manifest(api_docs)?;
    // Extract `name:` from YAML frontmatter
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("name:") {
            let n = rest.trim().trim_matches('"').trim_matches('\'').to_owned();
            if !n.is_empty() {
                return Some(n);
            }
        }
    }
    None
}

fn read_project_manifest(api_docs: &Path) -> Option<String> {
    for filename in ["reqbook.md", "mad.md"] {
        if let Ok(content) = fs::read_to_string(api_docs.join(filename)) {
            return Some(content);
        }
    }
    None
}

fn chrono_now() -> String {
    // RFC 3339 timestamp without external chrono dep — use std only.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as YYYY-MM-DDTHH:MM:SSZ (approximate, UTC only)
    let s = secs;
    let sec = s % 60;
    let min = (s / 60) % 60;
    let hour = (s / 3600) % 24;
    let days = s / 86400; // days since epoch
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Gregorian calendar approximation from Unix epoch (1970-01-01)
    let mut year = 1970u64;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = [
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
    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensures_env_files_are_gitignored_for_default_collection() {
        let temp = tempfile::tempdir().unwrap();
        let collection = temp.path().join("api-docs");
        fs::create_dir_all(&collection).unwrap();

        let added = ensure_env_files_gitignored(&collection).unwrap();

        assert_eq!(added, vec![".env.local", "api-docs/_shared/env.md"]);
        let gitignore = fs::read_to_string(temp.path().join(".gitignore")).unwrap();
        assert!(gitignore.lines().any(|line| line == ".env.local"));
        assert!(gitignore
            .lines()
            .any(|line| line == "api-docs/_shared/env.md"));

        let added_again = ensure_env_files_gitignored(&collection).unwrap();
        assert!(added_again.is_empty());
    }

    #[test]
    fn uses_collection_relative_env_md_gitignore_entry() {
        let temp = tempfile::tempdir().unwrap();
        let collection = temp.path().join("docs/contracts");
        fs::create_dir_all(&collection).unwrap();

        let added = ensure_env_files_gitignored(&collection).unwrap();

        assert_eq!(added, vec![".env.local", "contracts/_shared/env.md"]);
    }

    #[test]
    fn init_workspace_writes_template_and_local_env() {
        let temp = tempfile::tempdir().unwrap();

        init_workspace_dir(temp.path(), "demo-api").unwrap();

        assert!(temp
            .path()
            .join("api-docs/_shared/env.template.md")
            .exists());
        assert!(temp.path().join("api-docs/_shared/env.md").exists());
        let gitignore = fs::read_to_string(temp.path().join(".gitignore")).unwrap();
        assert!(gitignore
            .lines()
            .any(|line| line == "api-docs/_shared/env.md"));
        assert!(!gitignore
            .lines()
            .any(|line| line == "api-docs/_shared/env.template.md"));
    }
}
