//! Cross-agent skill and slash-command installation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

const SYNC: &str = include_str!("../../skills/trellis-sync/SKILL.md");
const DEBUG: &str = include_str!("../../skills/trellis-debug/SKILL.md");

// ─── Slash-command definitions ────────────────────────────────────────────────

/// A slash-command installed into the agent's commands directory.
struct CommandDef {
    slug: &'static str,
    description: &'static str,
    prompt: &'static str,
}

/// All Trellis slash commands, in order.
const COMMANDS: &[CommandDef] = &[
    CommandDef {
        slug: "trellis-sync",
        description: "Sync Trellis api-docs/ specs with source code — init, import, enrich, or author",
        prompt: r#"Follow the trellis-sync skill decision tree to keep api-docs/ aligned with source code.

If `api-docs/` does not exist, initialise first:
- Detect project name (package.json → Cargo.toml → pyproject.toml → go.mod → pom.xml → README heading → git remote → dirname).
- Detect base URL (.env PORT/HOST → docker-compose ports → framework default → ask).
- Run: `trellis init --name "<name>" --dev-url "<url>" --yes`

Then import routes:
```bash
trellis import project ${ARGUMENTS:-.}
```

Read output:
- `✓ Found OpenAPI spec` or `✓ Fetched live spec` → done, run `trellis index`.
- `⚠ No OpenAPI spec` → enrich each partial spec by reading handler source (params, body, response shape).
- `no routes found` → scan manually: `rg --files src/ | rg -i "route\|controller\|handler"`, extract each route, create spec via MCP `trellis_author`.

Always validate after writing: `trellis validate <file>`. Run `trellis index` at the end."#,
    },
    CommandDef {
        slug: "trellis-debug",
        description: "Trace and debug an API call or pipeline using Trellis specs",
        prompt: r#"Follow the trellis-debug skill decision tree to diagnose the API issue in $ARGUMENTS.

Single endpoint:
1. Locate the spec: `rg -n "^method:\|^path:" api-docs/apis/` or use MCP `trellis_list_specs`.
2. Validate: `trellis validate <file>` — fix any structural errors first.
3. Dry-run to inspect what's sent: `trellis exec <file> --env=dev --dry-run`
4. Execute: `trellis exec <file> --env=dev --var key=value`
5. On mismatch: compare expected vs actual status/body. Check if spec is outdated.

Exit codes: 2=invalid spec, 3=engine error, 4=network/DNS, 5=secret detected.

Pipeline:
1. Locate: `rg --files api-docs/flows/`
2. Execute: `trellis flow <file> --env=dev`
3. On failure: identify first failing step, debug it as a single endpoint with its captured inputs.
4. Check capture expressions match actual response shape (`response.body.id` vs `response.body.userId`).

Never print raw auth tokens. Confirm before running against prod."#,
    },
    CommandDef {
        slug: "trellis-exec",
        description: "Execute a Trellis endpoint spec and report the result",
        prompt: r#"Run the Trellis endpoint spec specified in $ARGUMENTS and report the result.

```bash
trellis exec $ARGUMENTS --env=dev
```

Report: endpoint file, environment, method + URL (mask auth headers), HTTP status, duration, and whether the diff passed.
If no file is specified, search `api-docs/**/*.md` for an endpoint matching the user's description and run that.
On failure include the exit code, error message, and the fix suggestion from trellis output."#,
    },
    CommandDef {
        slug: "trellis-flow",
        description: "Execute a Trellis pipeline and report step results",
        prompt: r#"Run the Trellis pipeline specified in $ARGUMENTS and report each step's result.

```bash
trellis flow $ARGUMENTS --env=dev
```

Report: pipeline name, environment, each step's endpoint + status + diff outcome, and overall pass/fail.
If no file is specified, search `api-docs/flows/**/*.md` for a pipeline matching the user's description.
On failure include which step failed, what was captured from previous steps, and the suggested fix."#,
    },
    CommandDef {
        slug: "trellis-validate",
        description: "Validate Trellis endpoint specs in a file or directory",
        prompt: r#"Validate the Trellis spec(s) at $ARGUMENTS (defaults to `api-docs/` if not given).

```bash
trellis validate ${ARGUMENTS:-api-docs/}
```

Report: number of files checked, any validation errors with file paths and line references, and the exit code.
Exit 2 = invalid spec. Exit 5 = secret detected in a versioned file."#,
    },
    CommandDef {
        slug: "trellis-import",
        description: "Scan the current project for API routes and import them as Trellis specs",
        prompt: r#"Run the Trellis project importer on the path in $ARGUMENTS (defaults to current directory).

```bash
trellis import project ${ARGUMENTS:-.}
```

Report the strategy used (OpenAPI file / live server / static scan), how many routes were found,
and how many spec files were created. Run `trellis index` after a successful import.
If the output includes a Tip with a framework export command, offer to run it."#,
    },
    CommandDef {
        slug: "trellis-import-curl",
        description: "Import a curl command from the clipboard or user input as a Trellis endpoint spec",
        prompt: r#"Ask the user to paste a `curl` command (e.g. copied from browser DevTools → Copy as cURL).

Once you have the curl text, run:

```bash
echo '<CURL_COMMAND>' | trellis import curl
```

Or save it to a temp file and run `trellis import curl /tmp/curl.txt`.

After import:
- Show the path of the created spec file.
- Remind the user to set `baseUrl` in `api-docs/_shared/env.md` if it's a new host.
- Offer to run `trellis exec <new-file>` to verify the endpoint works."#,
    },
    CommandDef {
        slug: "trellis-serve",
        description: "Start the Trellis web preview server",
        prompt: r#"Start the Trellis web preview server so the user can browse and run specs in a browser.

```bash
trellis serve
```

Report the URL printed by trellis (e.g. `http://127.0.0.1:8080`).
Tell the user they can open it in a browser to browse endpoints, click Run on any spec, and paste curl commands to import new endpoints."#,
    },
    CommandDef {
        slug: "trellis-mock",
        description: "Start the Trellis mock server to replay recorded API responses",
        prompt: r#"Start the Trellis mock server so the frontend can work without a live backend.

```bash
trellis mock ${ARGUMENTS:-api-docs/} --port 4001
```

The mock server reads every `## Expected response` block from endpoint specs and serves those
responses over HTTP. Path parameters like `/users/:id` are matched automatically.

Report:
- The base URL (e.g. `http://127.0.0.1:4001`)
- The number of routes loaded and their method + path
- Any duplicate routes that were skipped

To add artificial latency (useful for testing loading states):
```bash
trellis mock api-docs/ --port 4001 --latency 300
```"#,
    },
    CommandDef {
        slug: "trellis-mcp-setup",
        description: "Register the Trellis MCP server with your AI agent",
        prompt: r#"Register the Trellis MCP server with Claude Code so Trellis tools are available
to the AI directly (no bash required).

Run:
```bash
claude mcp add trellis -- trellis mcp
```

After registration, the following tools become available inside Claude Code:
- `trellis_exec`       — execute an endpoint spec
- `trellis_flow`       — run a pipeline
- `trellis_validate`   — validate specs in a file or directory
- `trellis_list_specs` — list all endpoint specs with method + path
- `trellis_read_spec`  — read the full content of a spec file
- `trellis_author`     — create a spec file, or update one only after explicit user approval

Trellis spec files are also exposed as **MCP Resources** under the `trellis://spec/` URI scheme,
so models can browse and read specs directly via the resources protocol.

Verify registration:
```bash
claude mcp list
```"#,
    },
];

// ─── Agent enum ───────────────────────────────────────────────────────────────

/// Supported AI agent targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    /// Claude Code workspace skills.
    ClaudeCode,
    /// Codex CLI global skills.
    CodexCli,
    /// Antigravity workspace or global skills.
    Antigravity,
    /// OpenCode workspace skills.
    OpenCode,
    /// Cursor project rules.
    Cursor,
    /// GitHub Copilot instructions.
    Copilot,
}

impl Agent {
    /// Parse CLI agent name.
    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "claude-code" | "claude" => Some(Self::ClaudeCode),
            "codex-cli" | "codex" => Some(Self::CodexCli),
            "antigravity" => Some(Self::Antigravity),
            "opencode" => Some(Self::OpenCode),
            "cursor" => Some(Self::Cursor),
            "copilot" | "github-copilot" => Some(Self::Copilot),
            _ => None,
        }
    }

    /// Stable CLI name.
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::CodexCli => "codex-cli",
            Self::Antigravity => "antigravity",
            Self::OpenCode => "opencode",
            Self::Cursor => "cursor",
            Self::Copilot => "copilot",
        }
    }

    /// Whether this agent supports slash commands.
    fn supports_commands(self) -> bool {
        matches!(self, Self::ClaudeCode | Self::CodexCli)
    }
}

// ─── Result types ─────────────────────────────────────────────────────────────

/// Installed file record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledFile {
    /// Agent target.
    pub agent: Agent,
    /// Written path.
    pub path: PathBuf,
}

/// Agent detection status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    /// Agent target.
    pub agent: Agent,
    /// Whether the agent was detected.
    pub detected: bool,
}

/// Installer error.
#[derive(Debug, Error)]
pub enum InstallError {
    /// Unknown agent name.
    #[error("unknown agent `{name}`\nFix: use one of claude-code, codex-cli, antigravity, opencode, cursor, copilot.")]
    UnknownAgent {
        /// Name provided by the user.
        name: String,
    },
    /// No agent could be detected.
    #[error("no supported agent detected\nFix: pass --agent=<name> or create the agent config directory first.")]
    NoAgentDetected,
    /// File IO failed.
    #[error(
        "{path}: file operation failed: {source}\nFix: check filesystem permissions and path."
    )]
    Io {
        /// Path.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Canonical skill metadata is invalid.
    #[error("canonical skill metadata is invalid: {source}\nFix: correct skills/*/SKILL.md frontmatter.")]
    InvalidSkill {
        /// Source error.
        #[source]
        source: serde_yaml::Error,
    },
    /// Unknown skill name.
    #[error("unknown skill \"{name}\". Available skills: trellis-sync, trellis-debug")]
    UnknownSkill {
        /// Name provided by the user.
        name: String,
    },
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

struct SkillSource {
    source: &'static str,
    meta: SkillFrontmatter,
    body: String,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// List detectable agents for a workspace.
pub fn detect_agents(root: &Path) -> Vec<AgentStatus> {
    [
        (Agent::ClaudeCode, root.join(".claude").exists()),
        (
            Agent::CodexCli,
            root.join(".codex").exists()
                || dirs::home_dir()
                    .map(|home| home.join(".codex").exists())
                    .unwrap_or(false),
        ),
        (
            Agent::Antigravity,
            root.join(".agents").exists()
                || root.join(".agent").exists()
                || dirs::home_dir()
                    .map(|home| home.join(".gemini/antigravity").exists())
                    .unwrap_or(false),
        ),
        (Agent::OpenCode, root.join(".opencode").exists()),
        (Agent::Cursor, root.join(".cursor").exists()),
        (Agent::Copilot, root.join(".github").exists()),
    ]
    .into_iter()
    .map(|(agent, detected)| AgentStatus { agent, detected })
    .collect()
}

/// Install all Trellis skills **and slash commands** for one explicit agent or
/// all detected agents.
pub fn install(root: &Path, agent: Option<&str>) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let skills = canonical_skills()?;
    let mut installed = Vec::new();

    for agent in agents {
        // Install skills (SKILL.md / .mdc / .instructions.md).
        for skill in &skills {
            let path = skill_target_path(root, agent, &skill.meta.name);
            let contents = render_skill(agent, skill);
            write_file(&path, &contents)?;
            installed.push(InstalledFile { agent, path });
        }

        // Install slash commands for agents that support them.
        if agent.supports_commands() {
            for cmd in COMMANDS {
                let path = command_target_path(root, agent, cmd.slug);
                let contents = render_command(cmd);
                write_file(&path, &contents)?;
                installed.push(InstalledFile { agent, path });
            }
        }
    }
    Ok(installed)
}

/// Install one specific skill by name for one explicit agent or all detected agents.
/// Returns `InstallError::UnknownSkill` if the name doesn't match any canonical skill.
pub fn install_skill(root: &Path, agent: Option<&str>, skill_name: &str) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let all_skills = canonical_skills()?;
    let skill = all_skills
        .into_iter()
        .find(|s| s.meta.name == skill_name)
        .ok_or_else(|| InstallError::UnknownSkill { name: skill_name.to_string() })?;
    let mut installed = Vec::new();
    for agent in agents {
        let path = skill_target_path(root, agent, &skill.meta.name);
        let contents = render_skill(agent, &skill);
        write_file(&path, &contents)?;
        installed.push(InstalledFile { agent, path });
    }
    Ok(installed)
}

/// Remove installed Trellis skills and slash commands.
pub fn uninstall(root: &Path, agent: Option<&str>) -> Result<Vec<PathBuf>, InstallError> {
    let agents = if let Some(agent) = agent {
        vec![
            Agent::parse(agent).ok_or_else(|| InstallError::UnknownAgent {
                name: agent.to_string(),
            })?,
        ]
    } else {
        vec![
            Agent::ClaudeCode,
            Agent::CodexCli,
            Agent::Antigravity,
            Agent::OpenCode,
            Agent::Cursor,
            Agent::Copilot,
        ]
    };

    let skills = canonical_skills()?;
    let mut removed = Vec::new();

    for agent in agents {
        for skill in &skills {
            let path = skill_target_path(root, agent, &skill.meta.name);
            if path.exists() {
                fs::remove_file(&path).map_err(|source| InstallError::Io {
                    path: path.clone(),
                    source,
                })?;
                removed.push(path);
            }
        }

        if agent.supports_commands() {
            for cmd in COMMANDS {
                let path = command_target_path(root, agent, cmd.slug);
                if path.exists() {
                    fs::remove_file(&path).map_err(|source| InstallError::Io {
                        path: path.clone(),
                        source,
                    })?;
                    removed.push(path);
                }
            }
        }
    }
    Ok(removed)
}

/// Remove one specific skill by name.
pub fn uninstall_skill(root: &Path, agent: Option<&str>, skill_name: &str) -> Result<Vec<PathBuf>, InstallError> {
    let agents = if let Some(agent) = agent {
        vec![Agent::parse(agent).ok_or_else(|| InstallError::UnknownAgent { name: agent.to_string() })?]
    } else {
        vec![Agent::ClaudeCode, Agent::CodexCli, Agent::Antigravity, Agent::OpenCode, Agent::Cursor, Agent::Copilot]
    };
    let all_skills = canonical_skills()?;
    let skill = all_skills
        .into_iter()
        .find(|s| s.meta.name == skill_name)
        .ok_or_else(|| InstallError::UnknownSkill { name: skill_name.to_string() })?;
    let mut removed = Vec::new();
    for agent in agents {
        let path = skill_target_path(root, agent, &skill.meta.name);
        if path.exists() {
            fs::remove_file(&path).map_err(|source| InstallError::Io { path: path.clone(), source })?;
            removed.push(path);
        }
    }
    Ok(removed)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn resolve_agents(root: &Path, agent: Option<&str>) -> Result<Vec<Agent>, InstallError> {
    if let Some(agent) = agent {
        Ok(vec![Agent::parse(agent).ok_or_else(|| {
            InstallError::UnknownAgent {
                name: agent.to_string(),
            }
        })?])
    } else {
        let detected: Vec<_> = detect_agents(root)
            .into_iter()
            .filter_map(|status| status.detected.then_some(status.agent))
            .collect();
        if detected.is_empty() {
            return Err(InstallError::NoAgentDetected);
        }
        Ok(detected)
    }
}

fn canonical_skills() -> Result<Vec<SkillSource>, InstallError> {
    [SYNC, DEBUG]
        .into_iter()
        .map(parse_skill)
        .collect()
}

fn parse_skill(source: &'static str) -> Result<SkillSource, InstallError> {
    let rest = source
        .strip_prefix("---\n")
        .ok_or_else(|| InstallError::InvalidSkill {
            source: serde_yaml::from_str::<SkillFrontmatter>("").unwrap_err(),
        })?;
    let Some(end) = rest.find("\n---") else {
        return Err(InstallError::InvalidSkill {
            source: serde_yaml::from_str::<SkillFrontmatter>("").unwrap_err(),
        });
    };
    let frontmatter = &rest[..end];
    let body = rest[end + "\n---".len()..]
        .strip_prefix('\n')
        .unwrap_or_default()
        .to_string();
    let meta = serde_yaml::from_str(frontmatter)
        .map_err(|source| InstallError::InvalidSkill { source })?;
    Ok(SkillSource { source, meta, body })
}

/// Path for a skill file (SKILL.md, .mdc, or .instructions.md).
fn skill_target_path(root: &Path, agent: Agent, name: &str) -> PathBuf {
    match agent {
        Agent::ClaudeCode => root.join(format!(".claude/skills/{name}/SKILL.md")),
        Agent::CodexCli => dirs::home_dir()
            .unwrap_or_else(|| root.to_path_buf())
            .join(format!(".codex/skills/{name}/SKILL.md")),
        Agent::Antigravity => {
            let base = if root.join(".agents").exists() {
                root.join(".agents")
            } else if root.join(".agent").exists() {
                root.join(".agent")
            } else {
                root.join(".agents")
            };
            base.join(format!("skills/{name}/SKILL.md"))
        }
        Agent::OpenCode => root.join(format!(".opencode/skills/{name}/SKILL.md")),
        Agent::Cursor => root.join(format!(".cursor/rules/{name}.mdc")),
        Agent::Copilot => root.join(format!(".github/instructions/{name}.instructions.md")),
    }
}

/// Path for a slash-command file.
fn command_target_path(root: &Path, agent: Agent, slug: &str) -> PathBuf {
    match agent {
        Agent::ClaudeCode => root.join(format!(".claude/commands/{slug}.md")),
        Agent::CodexCli => dirs::home_dir()
            .unwrap_or_else(|| root.to_path_buf())
            .join(format!(".codex/commands/{slug}.md")),
        // Other agents do not support slash commands.
        _ => root.join(format!(".unsupported/commands/{slug}.md")),
    }
}

fn render_skill(agent: Agent, skill: &SkillSource) -> String {
    match agent {
        Agent::Cursor => format!(
            "---\ndescription: {}\nglobs: api-docs/**/*.md\nalwaysApply: false\n---\n\n{}",
            skill.meta.description, skill.body
        ),
        Agent::Copilot => format!(
            "---\napplyTo: \"api-docs/**/*.md\"\n---\n\n{}\n\n{}",
            skill.meta.description, skill.body
        ),
        Agent::ClaudeCode | Agent::CodexCli | Agent::Antigravity | Agent::OpenCode => {
            skill.source.to_string()
        }
    }
}

fn render_command(cmd: &CommandDef) -> String {
    format!(
        "---\ndescription: {}\n---\n{}\n",
        cmd.description, cmd.prompt
    )
}

fn write_file(path: &Path, contents: &str) -> Result<(), InstallError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| InstallError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, contents).map_err(|source| InstallError::Io {
        path: path.to_path_buf(),
        source,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_cursor_and_copilot_formats() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".cursor")).unwrap();
        fs::create_dir(dir.path().join(".github")).unwrap();
        let mut installed = install(dir.path(), Some("cursor")).unwrap();
        installed.extend(install(dir.path(), Some("copilot")).unwrap());
        assert_eq!(installed.len(), 4); // 2 skills each
        let cursor =
            fs::read_to_string(dir.path().join(".cursor/rules/trellis-sync.mdc")).unwrap();
        assert!(cursor.contains("alwaysApply: false"));
        assert!(dir
            .path()
            .join(".cursor/rules/trellis-debug.mdc")
            .exists());
        let copilot = fs::read_to_string(
            dir.path()
                .join(".github/instructions/trellis-sync.instructions.md"),
        )
        .unwrap();
        assert!(copilot.contains("applyTo: \"api-docs/**/*.md\""));
        assert!(dir
            .path()
            .join(".github/instructions/trellis-debug.instructions.md")
            .exists());
    }

    #[test]
    fn installs_slash_commands_for_claude_code() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let installed = install(dir.path(), Some("claude-code")).unwrap();

        // 2 skills + 10 commands = 12
        assert_eq!(installed.len(), 12);

        let cmd_path = dir.path().join(".claude/commands/trellis-exec.md");
        assert!(cmd_path.exists(), "trellis-exec command not created");
        let content = fs::read_to_string(&cmd_path).unwrap();
        assert!(content.contains("description:"));
        assert!(content.contains("trellis exec"));

        let import_cmd = dir.path().join(".claude/commands/trellis-import.md");
        assert!(import_cmd.exists(), "trellis-import command not created");
        let sync_skill = dir.path().join(".claude/skills/trellis-sync/SKILL.md");
        assert!(
            sync_skill.exists(),
            "trellis-sync skill not created"
        );
        let sync_cmd = dir.path().join(".claude/commands/trellis-sync.md");
        assert!(sync_cmd.exists(), "trellis-sync command not created");
        let debug_cmd = dir.path().join(".claude/commands/trellis-debug.md");
        assert!(debug_cmd.exists(), "trellis-debug command not created");
    }

    #[test]
    fn uninstalls_slash_commands() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        install(dir.path(), Some("claude-code")).unwrap();

        let exec_cmd = dir.path().join(".claude/commands/trellis-exec.md");
        assert!(exec_cmd.exists());

        uninstall(dir.path(), Some("claude-code")).unwrap();
        assert!(!exec_cmd.exists(), "command file should be removed");
    }

    #[test]
    fn cursor_does_not_get_commands() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".cursor")).unwrap();
        let installed = install(dir.path(), Some("cursor")).unwrap();
        // Only 2 skills, no commands
        assert_eq!(installed.len(), 2);
        assert!(!dir.path().join(".cursor/commands").exists());
    }

    #[test]
    fn all_commands_installed() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let installed = install(dir.path(), Some("claude-code")).unwrap();
        let cmd_paths: Vec<_> = installed
            .iter()
            .filter(|f| f.path.to_string_lossy().contains("commands"))
            .collect();
        assert_eq!(cmd_paths.len(), COMMANDS.len());
    }
}
