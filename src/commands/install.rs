//! `rqb install` and `rqb skills` commands.

#[cfg(feature = "install")]
use std::path::Path;

#[cfg(not(feature = "install"))]
use anyhow::bail;
use anyhow::Result;
#[cfg(feature = "install")]
use owo_colors::OwoColorize;

use crate::{InstallCommand, SkillsCommand};

pub(crate) async fn install(command: InstallCommand) -> Result<()> {
    #[cfg(not(feature = "install"))]
    {
        let _ = command;
        bail!(
            "install support is not compiled into this binary\nFix: install Reqbook with default features."
        );
    }

    #[cfg(feature = "install")]
    match command {
        InstallCommand::Skills { name, agent } => {
            let installed = if let Some(skill_name) = name {
                reqbook::installer::install_skill(Path::new("."), agent.as_deref(), &skill_name)?
            } else {
                reqbook::installer::install_skills(Path::new("."), agent.as_deref())?
            };
            for file in &installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            println!("{} skill(s) installed", installed.len());
            Ok(())
        }
        InstallCommand::Slashcmd { name, agent } => {
            let installed = if let Some(slug) = name {
                reqbook::installer::install_command(Path::new("."), agent.as_deref(), &slug)?
            } else {
                reqbook::installer::install_commands(Path::new("."), agent.as_deref())?
            };
            for file in &installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            println!("{} command(s) installed", installed.len());
            Ok(())
        }
        InstallCommand::Mcp { agent } => install_mcp(agent.as_deref()),
        InstallCommand::List => {
            for status in reqbook::installer::detect_agents(Path::new(".")) {
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
            "install support is not compiled into this binary\nFix: install Reqbook with default features."
        );
    }

    #[cfg(feature = "install")]
    match command {
        SkillsCommand::Install { name, agent } => {
            let installed = if let Some(skill_name) = name {
                reqbook::installer::install_skill(Path::new("."), agent.as_deref(), &skill_name)?
            } else {
                reqbook::installer::install_skills(Path::new("."), agent.as_deref())?
            };
            for file in &installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            println!("{} skill(s) installed", installed.len());
            Ok(())
        }
        SkillsCommand::List => {
            for status in reqbook::installer::detect_agents(Path::new(".")) {
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
                reqbook::installer::uninstall_skill(Path::new("."), agent.as_deref(), &skill_name)?
            } else {
                reqbook::installer::uninstall(Path::new("."), agent.as_deref())?
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
pub(crate) fn install_mcp(agent: Option<&str>) -> Result<()> {
    let installed = reqbook::installer::install_mcp(Path::new("."), agent)?;
    for file in &installed {
        println!(
            "installed {} MCP config: {}",
            file.agent.name(),
            file.path.display()
        );
    }
    println!(
        "{} {} MCP config(s) installed. Restart the agent or reload MCP tools to connect.",
        "✓".green(),
        installed.len()
    );
    Ok(())
}

#[cfg(not(feature = "install"))]
#[allow(dead_code)]
pub(crate) fn install_mcp(_agent: Option<&str>) -> Result<()> {
    bail!(
        "install support is not compiled into this binary\nFix: install Reqbook with default features."
    )
}
