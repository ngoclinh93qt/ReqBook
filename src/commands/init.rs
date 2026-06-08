//! `rqb init` command — scaffold a new api-docs directory.

use std::{fs, path::Path};

use anyhow::{bail, Context as AnyhowContext, Result};
use owo_colors::OwoColorize;
use reqbook::workspace;

use super::regenerate_index;

pub(crate) fn detect_project_name() -> Option<String> {
    if let Ok(raw) = fs::read_to_string("package.json") {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
                let name = n.split('/').next_back().unwrap_or(n).trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    if let Ok(raw) = fs::read_to_string("Cargo.toml") {
        let mut in_package = false;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed == "[package]" {
                in_package = true;
                continue;
            }
            if trimmed.starts_with('[') {
                in_package = false;
            }
            if in_package {
                if let Some(rest) = trimmed.strip_prefix("name") {
                    let rest = rest.trim().strip_prefix('=').unwrap_or("").trim();
                    let name = rest.trim_matches('"').trim_matches('\'').trim().to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    }

    if let Ok(raw) = fs::read_to_string("pyproject.toml") {
        let mut in_section = false;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed == "[project]" || trimmed == "[tool.poetry]" {
                in_section = true;
                continue;
            }
            if trimmed.starts_with('[') {
                in_section = false;
            }
            if in_section {
                if let Some(rest) = trimmed.strip_prefix("name") {
                    let rest = rest.trim().strip_prefix('=').unwrap_or("").trim();
                    let name = rest.trim_matches('"').trim_matches('\'').trim().to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    }

    if let Ok(raw) = fs::read_to_string("go.mod") {
        if let Some(line) = raw.lines().next() {
            if let Some(path) = line.strip_prefix("module ") {
                let name = path
                    .trim()
                    .split('/')
                    .next_back()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    if let Ok(raw) = fs::read_to_string("composer.json") {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
                let name = n.split('/').next_back().unwrap_or(n).trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    if let Ok(raw) = fs::read_to_string("pom.xml") {
        let re = regex::Regex::new(r"<artifactId>([^<]+)</artifactId>").expect("valid regex");
        if let Some(cap) = re.captures(&raw) {
            let name = cap[1].trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    None
}

pub(crate) fn run(args: crate::InitArgs, collection: &Path, yes: bool) -> Result<()> {
    let detected = detect_project_name();
    let default_name = detected.unwrap_or_else(|| "my-api".to_string());
    let name = match args.name {
        Some(name) => name,
        None if yes => default_name,
        None => dialoguer::Input::new()
            .with_prompt("Project name")
            .default(default_name)
            .interact_text()?,
    };
    let dev_url = match args.dev_url {
        Some(url) => url,
        None if yes => "http://localhost:8080".to_string(),
        None => dialoguer::Input::new()
            .with_prompt("Base URL (dev)")
            .default("http://localhost:8080".to_string())
            .interact_text()?,
    };

    println!("Collection: {}", collection.display());
    fs::create_dir_all(collection.join("_shared"))?;
    fs::create_dir_all(collection.join("apis/posts"))?;
    fs::create_dir_all(collection.join("flows"))?;
    write_new(&collection.join("reqbook.md"), &project_config(&name))?;
    let env_config = workspace::env_config_with_base_url(&dev_url);
    write_new(
        &workspace::shared_env_template_path(collection),
        &env_config,
    )?;
    write_new(&workspace::shared_env_path(collection), &env_config)?;
    write_new(
        &collection.join("apis/posts/get-posts.md"),
        example_endpoint(),
    )?;
    workspace::ensure_env_files_gitignored(collection)?;
    regenerate_index(collection)?;

    println!("{} Created reqbook.md (project config)", "✓".green());
    println!("{} Created api-docs/ with 1 example", "✓".green());
    Ok(())
}

fn project_config(name: &str) -> String {
    format!(
        r#"---
name: {name}
version: 1
default-env: dev
---
# {name}

API specs for {name}.

## Defaults

```yaml
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
auth: none
```

## Web preview

```yaml
port: 8080
host: 127.0.0.1
theme: auto
autosave: 2s
```

## Plugins

```yaml
plugins: []
```
"#
    )
}

fn example_endpoint() -> &'static str {
    r#"---
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
GET {{baseUrl}}/posts/:postId
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": 1
}
```

## Tests

```agent-task
- Verify the response status is 200.
- Verify response.body.id equals postId.
```
"#
}

pub(crate) fn write_new(path: &Path, contents: &str) -> Result<()> {
    if path.exists() {
        bail!(
            "{} already exists\nFix: choose an empty directory or remove the existing file intentionally.",
            path.display()
        );
    }
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}
