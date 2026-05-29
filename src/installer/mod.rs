//! Cross-agent skill and slash-command installation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

const MAD: &str = include_str!("../../skills/mad/SKILL.md");

// ─── Slash-command definitions ────────────────────────────────────────────────

/// A slash-command installed into the agent's commands directory.
/// `content` is the full markdown file content (including YAML frontmatter).
struct CommandDef {
    slug: &'static str,
    content: &'static str,
}

/// All MarkApiDown slash commands, in order.
const COMMANDS: &[CommandDef] = &[
    CommandDef {
        slug: "mad",
        content: include_str!("../../.claude/commands/mad.md"),
    },
    CommandDef {
        slug: "mad-debug",
        content: include_str!("../../.claude/commands/mad-debug.md"),
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
    #[error("unknown skill \"{name}\". Available skills: mad")]
    UnknownSkill {
        /// Name provided by the user.
        name: String,
    },
    /// Unknown slash command slug.
    #[error("unknown command \"{name}\". Available commands: mad, mad-debug")]
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

/// Install all MarkApiDown skills **and slash commands** for one explicit agent or
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

/// Install only skills (no slash commands) for one explicit agent or all detected agents.
pub fn install_skills(root: &Path, agent: Option<&str>) -> Result<Vec<InstalledFile>, InstallError> {
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

/// Install all slash commands for one explicit agent or all detected agents.
/// Only agents that support commands (Claude Code, Codex CLI) will receive files.
pub fn install_commands(root: &Path, agent: Option<&str>) -> Result<Vec<InstalledFile>, InstallError> {
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
pub fn install_command(root: &Path, agent: Option<&str>, slug: &str) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = resolve_agents(root, agent)?;
    let cmd = COMMANDS
        .iter()
        .find(|c| c.slug == slug)
        .ok_or_else(|| InstallError::UnknownCommand { name: slug.to_string() })?;
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

/// Remove installed MarkApiDown skills and slash commands.
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
    [MAD]
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
        assert_eq!(installed.len(), 2); // 1 skill each
        let cursor =
            fs::read_to_string(dir.path().join(".cursor/rules/mad.mdc")).unwrap();
        assert!(cursor.contains("alwaysApply: false"));
        let copilot = fs::read_to_string(
            dir.path()
                .join(".github/instructions/mad.instructions.md"),
        )
        .unwrap();
        assert!(copilot.contains("applyTo: \"api-docs/**/*.md\""));
    }

    #[test]
    fn installs_slash_commands_for_claude_code() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let installed = install(dir.path(), Some("claude-code")).unwrap();

        // 1 skill + 2 commands = 3
        assert_eq!(installed.len(), 3);

        for slug in &["mad", "mad-debug"] {
            let path = dir.path().join(format!(".claude/commands/{slug}.md"));
            assert!(path.exists(), "{slug} command not created");
            let content = fs::read_to_string(&path).unwrap();
            assert!(content.contains("description:"), "{slug}: missing description");
        }

        let skill = dir.path().join(".claude/skills/mad/SKILL.md");
        assert!(skill.exists(), "mad skill not created");
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
        let installed = install_command(dir.path(), Some("claude-code"), "mad").unwrap();
        assert_eq!(installed.len(), 1);
        assert!(dir.path().join(".claude/commands/mad.md").exists());
        assert!(!dir.path().join(".claude/commands/mad-debug.md").exists());
    }

    #[test]
    fn install_single_command_unknown_slug_errors() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let result = install_command(dir.path(), Some("claude-code"), "mad-nonexistent");
        assert!(matches!(result, Err(InstallError::UnknownCommand { .. })));
    }

    #[test]
    fn uninstalls_slash_commands() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        install(dir.path(), Some("claude-code")).unwrap();

        let cmd = dir.path().join(".claude/commands/mad.md");
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
