//! `rqb validate` command.

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::Result;
use reqbook::{
    parser::{self, parse_endpoint, parse_pipeline},
    pipeline,
};

use super::read_text;

pub(crate) fn run(path: PathBuf) -> Result<()> {
    let started = Instant::now();
    let mut checked = 0usize;
    let mut errors = Vec::new();
    for file in markdown_files(&path)? {
        checked += 1;
        if let Err(error) = validate_file(&file) {
            errors.push(error.to_string());
        }
    }

    if errors.is_empty() {
        println!(
            "valid: {} ({} markdown files, {}ms)",
            path.display(),
            checked,
            started.elapsed().as_millis()
        );
        Ok(())
    } else {
        for error in &errors {
            eprintln!("{error}");
        }
        let exit = if errors
            .iter()
            .any(|error| error.contains("possible secret detected"))
        {
            5
        } else {
            2
        };
        std::process::exit(exit);
    }
}

pub(crate) fn validate_file(path: &Path) -> Result<()> {
    let source = read_text(path, "validating markdown")?;
    let result = if path.ends_with("_shared/env.md")
        || path.file_name().is_some_and(|name| name == "env.md")
    {
        parser::parse_env_config(&source, path).map(|_| ())
    } else if path
        .file_name()
        .is_some_and(|name| name == "reqbook.md" || name == "mad.md" || name == "README.md")
    {
        Ok(())
    } else if path
        .components()
        .any(|component| matches!(component.as_os_str().to_str(), Some("flows" | "pipelines")))
    {
        let pipeline = parse_pipeline(&source, path)?;
        pipeline::validate_dependencies(&pipeline)?;
        Ok(())
    } else {
        parse_endpoint(&source, path)?;
        Ok(())
    };
    result.map_err(Into::into)
}

pub(crate) fn markdown_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(path).map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(markdown_files(&path)?);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
