use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "trellis", version, about = "Markdown-native API specs")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print version information.
    Version,
    /// Validate a Trellis markdown file.
    Validate {
        /// File to validate.
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Version) => println!("{}", env!("CARGO_PKG_VERSION")),
        Some(Command::Validate { path }) => validate_path(path)?,
        None => {
            Cli::command().print_help()?;
            println!();
        }
    }
    Ok(())
}

fn validate_path(path: PathBuf) -> Result<()> {
    let source = fs::read_to_string(&path)?;
    let result =
        if path.ends_with("_shared/env.md") || path.file_name().is_some_and(|n| n == "env.md") {
            trellis::parser::parse_env_config(&source, &path).map(|_| ())
        } else if path
            .parent()
            .and_then(|parent| parent.file_name())
            .is_some_and(|name| name == "pipelines")
        {
            trellis::parser::parse_pipeline(&source, &path).map(|_| ())
        } else {
            trellis::parser::parse_endpoint(&source, &path).map(|_| ())
        };

    match result {
        Ok(()) => {
            println!("valid: {}", path.display());
            Ok(())
        }
        Err(error @ trellis::parser::ParseError::SecretDetected { .. }) => {
            eprintln!("{error}");
            std::process::exit(5);
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
