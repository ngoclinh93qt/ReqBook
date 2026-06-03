//! `mad export` command.

use anyhow::{Context as AnyhowContext, Result};

use crate::ExportCommand;

pub(crate) fn run(command: ExportCommand) -> Result<()> {
    match command {
        ExportCommand::Openapi { path, out, json } => {
            let rendered = mark_api_down::exporter::openapi::export_string(&path, json)
                .with_context(|| format!("exporting OpenAPI from {}", path.display()))?;
            if let Some(out) = out {
                if let Some(parent) = out.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("creating {}", parent.display()))?;
                    }
                }
                std::fs::write(&out, rendered)
                    .with_context(|| format!("writing {}", out.display()))?;
                println!("exported OpenAPI to {}", out.display());
            } else {
                println!("{rendered}");
            }
            Ok(())
        }
    }
}
