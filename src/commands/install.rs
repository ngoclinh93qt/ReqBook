//! `mad install` and `mad skills` commands.

use std::path::Path;

use anyhow::{bail, Result};
use owo_colors::OwoColorize;

use crate::{InstallCommand, SkillsCommand};

pub(crate) async fn install(command: InstallCommand) -> Result<()> {
    #[cfg(not(feature = "install"))]
    {
        let _ = command;
        bail!(
            "install support is not compiled into this binary\nFix: install MarkApiDown with default features."
        );
    }

    #[cfg(feature = "install")]
    match command {
        InstallCommand::Skills { name, agent } => {
            let installed = if let Some(skill_name) = name {
                mark_api_down::installer::install_skill(
                    Path::new("."),
                    agent.as_deref(),
                    &skill_name,
                )?
            } else {
                mark_api_down::installer::install_skills(Path::new("."), agent.as_deref())?
            };
            for file in &installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            println!("{} skill(s) installed", installed.len());
            Ok(())
        }
        InstallCommand::Slashcmd { name, agent } => {
            let installed = if let Some(slug) = name {
                mark_api_down::installer::install_command(Path::new("."), agent.as_deref(), &slug)?
            } else {
                mark_api_down::installer::install_commands(Path::new("."), agent.as_deref())?
            };
            for file in &installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            println!("{} command(s) installed", installed.len());
            Ok(())
        }
        InstallCommand::Mcp => install_mcp(),
        InstallCommand::List => {
            for status in mark_api_down::installer::detect_agents(Path::new(".")) {
                println!(
                    "{}: {}",
                    status.agent.name(),
                    if status.detected {
                        "detected"
                    } else {
                        "not detected"
                    }
                );
            }
            Ok(())
        }
    }
}

pub(crate) async fn skills(command: SkillsCommand) -> Result<()> {
    #[cfg(not(feature = "install"))]
    {
        let _ = command;
        bail!(
            "install support is not compiled into this binary\nFix: install MarkApiDown with default features."
        );
    }

    #[cfg(feature = "install")]
    match command {
        SkillsCommand::Install { name, agent } => {
            let installed = if let Some(skill_name) = name {
                mark_api_down::installer::install_skill(
                    Path::new("."),
                    agent.as_deref(),
                    &skill_name,
                )?
            } else {
                mark_api_down::installer::install_skills(Path::new("."), agent.as_deref())?
            };
            for file in &installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            println!("{} skill(s) installed", installed.len());
            Ok(())
        }
        SkillsCommand::List => {
            for status in mark_api_down::installer::detect_agents(Path::new(".")) {
                println!(
                    "{}: {}",
                    status.agent.name(),
                    if status.detected {
                        "detected"
                    } else {
                        "not detected"
                    }
                );
            }
            Ok(())
        }
        SkillsCommand::Uninstall { name, agent } => {
            let removed = if let Some(skill_name) = name {
                mark_api_down::installer::uninstall_skill(
                    Path::new("."),
                    agent.as_deref(),
                    &skill_name,
                )?
            } else {
                mark_api_down::installer::uninstall(Path::new("."), agent.as_deref())?
            };
            for path in &removed {
                println!("removed {}", path.display());
            }
            println!("{} file(s) removed", removed.len());
            Ok(())
        }
    }
}

#[cfg(feature = "install")]
pub(crate) fn install_mcp() -> Result<()> {
    println!("Registering MarkApiDown MCP server with Claude Code...");
    let status = std::process::Command::new("claude")
        .args(["mcp", "add", "mad", "--", "mad", "mcp"])
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("{} Registered. Verify with: claude mcp list", "✓".green());
            Ok(())
        }
        Ok(_) => bail!(
            "claude mcp add failed\nFix: run `claude mcp add mad -- mad mcp` manually."
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(
            "claude CLI not found\nFix: install Claude Code, then run `claude mcp add mad -- mad mcp`."
        ),
        Err(e) => bail!(
            "failed to run claude: {e}\nFix: run `claude mcp add mad -- mad mcp` manually."
        ),
    }
}

#[cfg(not(feature = "install"))]
pub(crate) fn install_mcp() -> Result<()> {
    bail!(
        "install support is not compiled into this binary\nFix: install MarkApiDown with default features."
    )
}
