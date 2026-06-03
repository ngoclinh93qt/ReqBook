//! `mad import` command.

use std::{io::Read, path::Path};

use anyhow::{Context as AnyhowContext, Result};

use super::{read_text, regenerate_index};
use crate::ImportCommand;

pub(crate) async fn run(command: ImportCommand, collection: &Path) -> Result<()> {
    match command {
        ImportCommand::Postman { file } => run_file_import(
            &file,
            "Postman collection",
            mark_api_down::importer::postman::import,
            collection,
        ),
        ImportCommand::Insomnia { file } => run_file_import(
            &file,
            "Insomnia export",
            mark_api_down::importer::insomnia::import,
            collection,
        ),
        ImportCommand::Openapi { file } => run_file_import(
            &file,
            "OpenAPI spec",
            mark_api_down::importer::openapi::import,
            collection,
        ),
        ImportCommand::Collection { path } => {
            let (name, endpoints) = mark_api_down::importer::collection::import_dir(&path)
                .with_context(|| format!("{}: invalid API client collection", path.display()))?;
            write_imported(&name, &path, &endpoints, collection)
        }
        ImportCommand::Http { file } => run_file_import(
            &file,
            ".http request file",
            mark_api_down::importer::http_file::import,
            collection,
        ),
        ImportCommand::Curl { file } => {
            let source = match file {
                Some(ref path) => read_text(path, "reading curl command")?,
                None => {
                    let mut buf = String::new();
                    std::io::stdin()
                        .read_to_string(&mut buf)
                        .context("reading curl command from stdin")?;
                    buf
                }
            };
            let (name, endpoints) =
                mark_api_down::importer::curl::import(&source).context("invalid curl command")?;
            if endpoints.is_empty() {
                println!("no endpoints parsed");
                return Ok(());
            }
            let parent = collection.parent().unwrap_or(Path::new("."));
            let written = mark_api_down::importer::write_endpoints(parent, &endpoints)?;
            println!("imported from {name}");
            for path in &written {
                println!("  created {}", path.display());
            }
            println!("{} endpoint(s) written", written.len());
            if !written.is_empty() {
                regenerate_index(collection)?;
            }
            Ok(())
        }
        ImportCommand::Project { path, port, url } => {
            use mark_api_down::importer::project::{smart_import, ImportSource};
            use owo_colors::OwoColorize;

            let root = path.unwrap_or_else(|| Path::new(".").to_path_buf());
            let started = std::time::Instant::now();

            println!("Importing from {} …", root.display());

            let (name, endpoints, source) = smart_import(&root, port, url.as_deref())
                .await
                .with_context(|| format!("importing from {}", root.display()))?;

            match &source {
                ImportSource::StaticFile(p) => {
                    println!(
                        "{} Found OpenAPI spec: {} (full params/body/responses)",
                        "✓".green(),
                        p.display()
                    );
                }
                ImportSource::RunningServer(u) => {
                    println!(
                        "{} Fetched live spec from {} (full params/body/responses)",
                        "✓".green(),
                        u
                    );
                }
                ImportSource::StaticScan(fw) => {
                    println!(
                        "{} No OpenAPI spec found   used static code scan (method + path only)",
                        "⚠".yellow()
                    );
                    if let Some(fw) = fw {
                        println!("  Detected framework: {}", fw.name);
                        if !fw.export_cmd.is_empty() {
                            println!();
                            println!("  Tip: export a full spec without starting a server:");
                            println!("    {}", fw.export_cmd);
                            println!("  Then: mad import openapi openapi.json");
                            println!();
                        } else {
                            println!();
                            println!("  Tip: start your dev server and re-run:");
                            println!("    mad import project --port <PORT>");
                            println!();
                        }
                    }
                }
            }

            if endpoints.is_empty() {
                println!("no routes found ({}ms)", started.elapsed().as_millis());
                println!("Tip: run from a directory containing source code, or use --url.");
                return Ok(());
            }

            let parent = collection.parent().unwrap_or(Path::new("."));
            let written = mark_api_down::importer::write_endpoints(parent, &endpoints)?;
            println!(
                "imported {}   {} route(s) found ({}ms)",
                name,
                endpoints.len(),
                started.elapsed().as_millis()
            );
            for p in &written {
                println!("  created {}", p.display());
            }
            let skipped = endpoints.len() - written.len();
            if skipped > 0 {
                println!("  {} spec(s) already existed and were skipped", skipped);
            }
            println!("{} spec(s) written", written.len());
            if !written.is_empty() {
                regenerate_index(collection)?;
                let env_path = collection.join("_shared/env.md");
                println!(
                    "Next: set baseUrl in {}, \
                     then run `mad validate {}`",
                    env_path.display(),
                    collection.display()
                );
            }
            Ok(())
        }
    }
}

fn run_file_import(
    file: &Path,
    kind: &str,
    parse: impl Fn(&str) -> anyhow::Result<(String, Vec<mark_api_down::importer::ImportedEndpoint>)>,
    collection: &Path,
) -> Result<()> {
    let source = read_text(file, &format!("reading {kind}"))?;
    let (name, endpoints) =
        parse(&source).with_context(|| format!("{}: invalid {kind}", file.display()))?;
    write_imported(&name, file, &endpoints, collection)
}

fn write_imported(
    name: &str,
    source_path: &Path,
    endpoints: &[mark_api_down::importer::ImportedEndpoint],
    collection: &Path,
) -> Result<()> {
    if endpoints.is_empty() {
        println!("no endpoints found in {}", source_path.display());
        return Ok(());
    }
    let parent = collection.parent().unwrap_or(Path::new("."));
    let written = mark_api_down::importer::write_endpoints(parent, endpoints)?;
    println!("imported from {} ({})", name, source_path.display());
    for path in &written {
        println!("  created {}", path.display());
    }
    println!("{} endpoint(s) written", written.len());
    if !written.is_empty() {
        regenerate_index(collection)?;
    }
    Ok(())
}
