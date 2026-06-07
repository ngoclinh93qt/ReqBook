//! Cross-agent skill, slash-command, and MCP installation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::{json, Value};
use thiserror::Error;

const RQB: &str = include_str!("../../skills/rqb/SKILL.md");

// ─── Slash-command definitions ────────────────────────────────────────────────

/// A slash-command installed into the agent's commands directory.
/// `content` is the full markdown file content (including YAML frontmatter).
struct CommandDef {
    slug: &'static str,
    content: &'static str,
}

/// All Reqbook slash commands, in order.
const COMMANDS: &[CommandDef] = &[
    CommandDef {
        slug: "rqb",
        content: include_str!("../../commands/rqb.md"),
    },
    CommandDef {
        slug: "rqb-debug",
        content: include_str!("../../commands/rqb-debug.md"),
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
    /// Windsurf (Codeium) workspace rules.
    Windsurf,
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
            "windsurf" | "droid" => Some(Self::Windsurf),
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
            Self::Windsurf => "windsurf",
        }
    }

    /// Whether this agent supports slash commands.
    fn supports_commands(self) -> bool {
        matches!(self, Self::ClaudeCode | Self::CodexCli)
    }
}

const ALL_AGENTS: &[Agent] = &[
    Agent::ClaudeCode,
    Agent::CodexCli,
    Agent::Antigravity,
    Agent::OpenCode,
    Agent::Cursor,
    Agent::Copilot,
    Agent::Windsurf,
];

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
    #[error("unknown agent `{name}`\nFix: use one of claude-code, codex-cli, antigravity, opencode, cursor, copilot, windsurf.")]
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
    /// Existing agent config could not be updated safely.
    #[error("{path}: invalid config: {message}\nFix: correct the file, then rerun the installer.")]
    InvalidConfig {
        /// Path.
        path: PathBuf,
        /// Error detail.
        message: String,
    },
    /// Home directory was required but unavailable.
    #[error(
        "home directory not available for {agent}\nFix: set HOME or install this MCP server manually."
    )]
    HomeDirUnavailable {
        /// Agent name.
        agent: &'static str,
    },
    /// Canonical skill metadata is invalid.
    #[error("canonical skill metadata is invalid: {source}\nFix: correct skills/*/SKILL.md frontmatter.")]
    InvalidSkill {
        /// Source error.
        #[source]
        source: serde_yaml::Error,
    },
    /// Unknown skill name.
    #[error("unknown skill \"{name}\". Available skills: rqb")]
    UnknownSkill {
        /// Name provided by the user.
        name: String,
    },
    /// Unknown slash command slug.
    #[error("unknown command \"{name}\". Available commands: rqb, rqb-debug")]
    UnknownCommand {
        /// Slug provided by the user.
        name: String,
    },
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

struct SkillSource {
    source: String,
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
        (
            Agent::Windsurf,
            root.join(".windsurf").exists() || root.join(".windsurfrules").exists(),
        ),
    ]
    .into_iter()
    .map(|(agent, detected)| AgentStatus { agent, detected })
    .collect()
}

/// Install all Reqbook skills **and slash commands** for one explicit agent or
/// all detected agents.
pub fn install(root: &Path, agent: Option<&str>) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let skills = canonical_skills()?;
    let mut installed = Vec::new();

    for agent in agents {
        for skill in &skills {
            let path = skill_target_path(root, agent, &skill.meta.name);
            let contents = render_skill(agent, skill);
            write_file(&path, &contents)?;
            installed.push(InstalledFile { agent, path });
        }

        if agent.supports_commands() {
            for cmd in COMMANDS {
                let path = command_target_path(root, agent, cmd.slug);
                write_file(&path, cmd.content)?;
                installed.push(InstalledFile { agent, path });
            }
        }
    }
    Ok(installed)
}

/// Install Reqbook MCP server configuration for one explicit agent or all detected agents.
pub fn install_mcp(root: &Path, agent: Option<&str>) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let mut installed = Vec::new();

    for agent in agents {
        let path = mcp_target_path(root, agent)?;
        write_mcp_config(agent, &path)?;
        installed.push(InstalledFile { agent, path });
    }

    Ok(installed)
}

/// Install only skills (no slash commands) for one explicit agent or all detected agents.
pub fn install_skills(
    root: &Path,
    agent: Option<&str>,
) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let skills = canonical_skills()?;
    let mut installed = Vec::new();
    for agent in agents {
        for skill in &skills {
            let path = skill_target_path(root, agent, &skill.meta.name);
            let contents = render_skill(agent, skill);
            write_file(&path, &contents)?;
            installed.push(InstalledFile { agent, path });
        }
    }
    Ok(installed)
}

/// Install one specific skill by name.
pub fn install_skill(
    root: &Path,
    agent: Option<&str>,
    skill_name: &str,
) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let all_skills = canonical_skills()?;
    let skill = all_skills
        .into_iter()
        .find(|s| s.meta.name == skill_name)
        .ok_or_else(|| InstallError::UnknownSkill {
            name: skill_name.to_string(),
        })?;
    let mut installed = Vec::new();
    for agent in agents {
        let path = skill_target_path(root, agent, &skill.meta.name);
        let contents = render_skill(agent, &skill);
        write_file(&path, &contents)?;
        installed.push(InstalledFile { agent, path });
    }
    Ok(installed)
}

/// Install all slash commands for one explicit agent or all detected agents.
/// Only agents that support commands (Claude Code, Codex CLI) will receive files.
pub fn install_commands(
    root: &Path,
    agent: Option<&str>,
) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let mut installed = Vec::new();
    for agent in agents {
        if !agent.supports_commands() {
            continue;
        }
        for cmd in COMMANDS {
            let path = command_target_path(root, agent, cmd.slug);
            write_file(&path, cmd.content)?;
            installed.push(InstalledFile { agent, path });
        }
    }
    Ok(installed)
}

/// Install one specific slash command by slug.
pub fn install_command(
    root: &Path,
    agent: Option<&str>,
    slug: &str,
) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let cmd =
        COMMANDS
            .iter()
            .find(|c| c.slug == slug)
            .ok_or_else(|| InstallError::UnknownCommand {
                name: slug.to_string(),
            })?;
    let mut installed = Vec::new();
    for agent in agents {
        if !agent.supports_commands() {
            continue;
        }
        let path = command_target_path(root, agent, cmd.slug);
        write_file(&path, cmd.content)?;
        installed.push(InstalledFile { agent, path });
    }
    Ok(installed)
}

/// Remove installed Reqbook skills and slash commands.
pub fn uninstall(root: &Path, agent: Option<&str>) -> Result<Vec<PathBuf>, InstallError> {
    let agents = if let Some(agent) = agent {
        vec![
            Agent::parse(agent).ok_or_else(|| InstallError::UnknownAgent {
                name: agent.to_string(),
            })?,
        ]
    } else {
        ALL_AGENTS.to_vec()
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
pub fn uninstall_skill(
    root: &Path,
    agent: Option<&str>,
    skill_name: &str,
) -> Result<Vec<PathBuf>, InstallError> {
    let agents = if let Some(agent) = agent {
        vec![
            Agent::parse(agent).ok_or_else(|| InstallError::UnknownAgent {
                name: agent.to_string(),
            })?,
        ]
    } else {
        ALL_AGENTS.to_vec()
    };
    let all_skills = canonical_skills()?;
    let skill = all_skills
        .into_iter()
        .find(|s| s.meta.name == skill_name)
        .ok_or_else(|| InstallError::UnknownSkill {
            name: skill_name.to_string(),
        })?;
    let mut removed = Vec::new();
    for agent in agents {
        let path = skill_target_path(root, agent, &skill.meta.name);
        if path.exists() {
            fs::remove_file(&path).map_err(|source| InstallError::Io {
                path: path.clone(),
                source,
            })?;
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
    [RQB].into_iter().map(parse_skill).collect()
}

fn parse_skill(source: &'static str) -> Result<SkillSource, InstallError> {
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
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
        Agent::CodexCli => root.join(format!(".agents/skills/{name}/SKILL.md")),
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
        Agent::Windsurf => root.join(format!(".windsurf/rules/{name}.md")),
    }
}

fn mcp_target_path(root: &Path, agent: Agent) -> Result<PathBuf, InstallError> {
    let path = match agent {
        Agent::ClaudeCode => root.join(".mcp.json"),
        Agent::CodexCli => root.join(".codex/config.toml"),
        Agent::Antigravity => home_dir(agent)?.join(".gemini/antigravity/mcp_config.json"),
        Agent::OpenCode => root.join("opencode.json"),
        Agent::Cursor => root.join(".cursor/mcp.json"),
        Agent::Copilot => root.join(".vscode/mcp.json"),
        Agent::Windsurf => home_dir(agent)?.join(".codeium/windsurf/mcp_config.json"),
    };
    Ok(path)
}

fn home_dir(agent: Agent) -> Result<PathBuf, InstallError> {
    dirs::home_dir().ok_or(InstallError::HomeDirUnavailable {
        agent: agent.name(),
    })
}

/// Path for a slash-command file.
fn command_target_path(root: &Path, agent: Agent, slug: &str) -> PathBuf {
    match agent {
        Agent::ClaudeCode => root.join(format!(".claude/commands/{slug}.md")),
        Agent::CodexCli => dirs::home_dir()
            .unwrap_or_else(|| root.to_path_buf())
            .join(format!(".codex/commands/{slug}.md")),
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
        Agent::Windsurf => format!(
            "---\ntrigger: agent_requested\ndescription: {}\n---\n\n{}",
            skill.meta.description, skill.body
        ),
        Agent::ClaudeCode | Agent::CodexCli | Agent::Antigravity | Agent::OpenCode => {
            skill.source.to_string()
        }
    }
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

fn write_mcp_config(agent: Agent, path: &Path) -> Result<(), InstallError> {
    match agent {
        Agent::CodexCli => write_codex_mcp_config(path),
        Agent::OpenCode => merge_json_mcp_server(
            path,
            "mcp",
            json!({
                "type": "local",
                "command": ["rqb", "mcp"],
                "enabled": true,
                "timeout": 10000
            }),
            Some(("schema", "https://opencode.ai/config.json")),
        ),
        Agent::Copilot => merge_json_mcp_server(
            path,
            "servers",
            json!({
                "type": "stdio",
                "command": "rqb",
                "args": ["mcp"]
            }),
            None,
        ),
        Agent::ClaudeCode | Agent::Antigravity | Agent::Cursor | Agent::Windsurf => {
            merge_json_mcp_server(
                path,
                "mcpServers",
                json!({
                    "type": "stdio",
                    "command": "rqb",
                    "args": ["mcp"]
                }),
                None,
            )
        }
    }
}

fn write_codex_mcp_config(path: &Path) -> Result<(), InstallError> {
    let existing = read_optional_string(path)?;
    let block = r#"[mcp_servers.rqb]
command = "rqb"
args = ["mcp"]
enabled = true
startup_timeout_sec = 10
tool_timeout_sec = 60
"#;
    let updated = upsert_toml_table(&existing, "mcp_servers.rqb", block);
    write_file(path, &updated)
}

fn upsert_toml_table(contents: &str, table: &str, block: &str) -> String {
    let header = format!("[{table}]");
    let mut output = Vec::new();
    let mut skipping = false;

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == header {
            skipping = true;
            continue;
        }
        if skipping && trimmed.starts_with('[') {
            skipping = false;
        }
        if !skipping {
            output.push(line);
        }
    }

    let mut rendered = output.join("\n");
    if !rendered.trim().is_empty() {
        rendered.push_str("\n\n");
    }
    rendered.push_str(block.trim_end());
    rendered.push('\n');
    rendered
}

fn merge_json_mcp_server(
    path: &Path,
    root_key: &str,
    server: Value,
    schema: Option<(&str, &str)>,
) -> Result<(), InstallError> {
    let existing = read_optional_string(path)?;
    let mut root = if existing.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(&existing).map_err(|source| InstallError::InvalidConfig {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?
    };

    let Some(object) = root.as_object_mut() else {
        return Err(InstallError::InvalidConfig {
            path: path.to_path_buf(),
            message: "top-level value must be a JSON object".to_string(),
        });
    };

    if let Some((key, value)) = schema {
        object
            .entry(format!("${key}"))
            .or_insert_with(|| Value::String(value.to_string()));
    }

    let entry = object
        .entry(root_key.to_string())
        .or_insert_with(|| json!({}));
    let Some(servers) = entry.as_object_mut() else {
        return Err(InstallError::InvalidConfig {
            path: path.to_path_buf(),
            message: format!("`{root_key}` must be a JSON object"),
        });
    };
    servers.insert("rqb".to_string(), server);

    let rendered =
        serde_json::to_string_pretty(&root).map_err(|source| InstallError::InvalidConfig {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
    write_file(path, &(rendered + "\n"))
}

fn read_optional_string(path: &Path) -> Result<String, InstallError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(source) => Err(InstallError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
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
        assert_eq!(installed.len(), 2); // 1 skill each
        let cursor = fs::read_to_string(dir.path().join(".cursor/rules/rqb.mdc")).unwrap();
        assert!(cursor.contains("alwaysApply: false"));
        let copilot =
            fs::read_to_string(dir.path().join(".github/instructions/rqb.instructions.md"))
                .unwrap();
        assert!(copilot.contains("applyTo: \"api-docs/**/*.md\""));
    }

    #[test]
    fn parses_skill_frontmatter_with_crlf() {
        let skill =
            parse_skill("---\r\nname: rqb\r\ndescription: Test skill\r\n---\r\n\r\n# Reqbook\r\n")
                .unwrap();

        assert_eq!(skill.meta.name, "rqb");
        assert!(skill.body.contains("# Reqbook"));
        assert!(skill.source.starts_with("---\n"));
    }

    #[test]
    fn installs_slash_commands_for_claude_code() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let installed = install(dir.path(), Some("claude-code")).unwrap();

        // 1 skill + 2 commands = 3
        assert_eq!(installed.len(), 3);

        for slug in &["rqb", "rqb-debug"] {
            let path = dir.path().join(format!(".claude/commands/{slug}.md"));
            assert!(path.exists(), "{slug} command not created");
            let content = fs::read_to_string(&path).unwrap();
            assert!(
                content.contains("description:"),
                "{slug}: missing description"
            );
        }

        let skill = dir.path().join(".claude/skills/rqb/SKILL.md");
        assert!(skill.exists(), "rqb skill not created");
    }

    #[test]
    fn installs_codex_skill_to_agents_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".codex")).unwrap();
        let installed = install_skills(dir.path(), Some("codex-cli")).unwrap();

        assert_eq!(installed.len(), 1);
        assert!(dir.path().join(".agents/skills/rqb/SKILL.md").exists());
    }

    #[test]
    fn installs_mcp_configs_for_project_scoped_agents() {
        let dir = tempfile::tempdir().unwrap();

        let installed = install_mcp(dir.path(), Some("claude-code")).unwrap();
        assert_eq!(installed.len(), 1);
        let claude = fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
        assert!(claude.contains("\"mcpServers\""));
        assert!(claude.contains("\"command\": \"rqb\""));

        let installed = install_mcp(dir.path(), Some("cursor")).unwrap();
        assert_eq!(installed.len(), 1);
        let cursor = fs::read_to_string(dir.path().join(".cursor/mcp.json")).unwrap();
        assert!(cursor.contains("\"mcpServers\""));
        assert!(cursor.contains("\"args\": ["));

        let installed = install_mcp(dir.path(), Some("copilot")).unwrap();
        assert_eq!(installed.len(), 1);
        let copilot = fs::read_to_string(dir.path().join(".vscode/mcp.json")).unwrap();
        assert!(copilot.contains("\"servers\""));
        assert!(copilot.contains("\"type\": \"stdio\""));

        let installed = install_mcp(dir.path(), Some("opencode")).unwrap();
        assert_eq!(installed.len(), 1);
        let opencode = fs::read_to_string(dir.path().join("opencode.json")).unwrap();
        assert!(opencode.contains("\"$schema\": \"https://opencode.ai/config.json\""));
        assert!(opencode.contains("\"type\": \"local\""));
    }

    #[test]
    fn installs_codex_mcp_config_without_dropping_existing_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join(".codex/config.toml");
        write_file(
            &config,
            r#"model = "gpt-5"

[mcp_servers.other]
command = "other"
"#,
        )
        .unwrap();

        install_mcp(dir.path(), Some("codex-cli")).unwrap();

        let content = fs::read_to_string(config).unwrap();
        assert!(content.contains("model = \"gpt-5\""));
        assert!(content.contains("[mcp_servers.other]"));
        assert!(content.contains("[mcp_servers.rqb]"));
        assert!(content.contains("args = [\"mcp\"]"));
    }

    #[test]
    fn install_skills_only_does_not_create_commands() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let installed = install_skills(dir.path(), Some("claude-code")).unwrap();
        assert_eq!(installed.len(), 1); // only skills
        assert!(!dir.path().join(".claude/commands").exists());
    }

    #[test]
    fn install_commands_only_does_not_create_skills() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let installed = install_commands(dir.path(), Some("claude-code")).unwrap();
        assert_eq!(installed.len(), COMMANDS.len());
        assert!(!dir.path().join(".claude/skills").exists());
    }

    #[test]
    fn install_single_command_by_slug() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let installed = install_command(dir.path(), Some("claude-code"), "rqb").unwrap();
        assert_eq!(installed.len(), 1);
        assert!(dir.path().join(".claude/commands/rqb.md").exists());
        assert!(!dir.path().join(".claude/commands/rqb-debug.md").exists());
    }

    #[test]
    fn install_single_command_unknown_slug_errors() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let result = install_command(dir.path(), Some("claude-code"), "rqb-nonexistent");
        assert!(matches!(result, Err(InstallError::UnknownCommand { .. })));
    }

    #[test]
    fn uninstalls_slash_commands() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        install(dir.path(), Some("claude-code")).unwrap();

        let cmd = dir.path().join(".claude/commands/rqb.md");
        assert!(cmd.exists());

        uninstall(dir.path(), Some("claude-code")).unwrap();
        assert!(!cmd.exists(), "command file should be removed");
    }

    #[test]
    fn cursor_does_not_get_commands() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".cursor")).unwrap();
        let installed = install(dir.path(), Some("cursor")).unwrap();
        // Only 1 skill, no commands
        assert_eq!(installed.len(), 1);
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
