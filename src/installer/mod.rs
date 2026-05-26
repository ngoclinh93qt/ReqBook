//! Cross-agent skill installation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

const AUTHOR: &str = include_str!("../../skills/trellis-author/SKILL.md");
const EXEC: &str = include_str!("../../skills/trellis-exec/SKILL.md");
const FLOW: &str = include_str!("../../skills/trellis-flow/SKILL.md");

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
}

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

/// Install all Trellis skills for one explicit agent or all detected agents.
pub fn install(root: &Path, agent: Option<&str>) -> Result<Vec<InstalledFile>, InstallError> {
    let agents = if let Some(agent) = agent {
        vec![
            Agent::parse(agent).ok_or_else(|| InstallError::UnknownAgent {
                name: agent.to_string(),
            })?,
        ]
    } else {
        let detected: Vec<_> = detect_agents(root)
            .into_iter()
            .filter_map(|status| status.detected.then_some(status.agent))
            .collect();
        if detected.is_empty() {
            return Err(InstallError::NoAgentDetected);
        }
        detected
    };

    let skills = canonical_skills()?;
    let mut installed = Vec::new();
    for agent in agents {
        for skill in &skills {
            let path = target_path(root, agent, &skill.meta.name);
            let contents = render(agent, skill);
            write_file(&path, &contents)?;
            installed.push(InstalledFile { agent, path });
        }
    }
    Ok(installed)
}

/// Remove installed Trellis skills for one explicit agent or all known workspace agents.
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
            let path = target_path(root, agent, &skill.meta.name);
            if path.exists() {
                fs::remove_file(&path).map_err(|source| InstallError::Io {
                    path: path.clone(),
                    source,
                })?;
                removed.push(path);
            }
        }
    }
    Ok(removed)
}

fn canonical_skills() -> Result<Vec<SkillSource>, InstallError> {
    [AUTHOR, EXEC, FLOW].into_iter().map(parse_skill).collect()
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

fn target_path(root: &Path, agent: Agent, name: &str) -> PathBuf {
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

fn render(agent: Agent, skill: &SkillSource) -> String {
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
        assert_eq!(installed.len(), 6);
        let cursor =
            fs::read_to_string(dir.path().join(".cursor/rules/trellis-author.mdc")).unwrap();
        assert!(cursor.contains("alwaysApply: false"));
        let copilot = fs::read_to_string(
            dir.path()
                .join(".github/instructions/trellis-author.instructions.md"),
        )
        .unwrap();
        assert!(copilot.contains("applyTo: \"api-docs/**/*.md\""));
    }
}
