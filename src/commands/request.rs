//! `mad request` command — send ad-hoc HTTP requests without a spec file.

use std::path::Path;

use anyhow::Result;
use mark_api_down::{
    adhoc::{self, AdHocParams},
    engine::{self, ExecOpts},
};

use super::{execution_context, print_report};
use crate::RequestArgs;

pub(crate) async fn run(args: RequestArgs, collection: &Path) -> Result<()> {
    let body = match &args.data {
        Some(d) if d.starts_with('@') => {
            let path = Path::new(&d[1..]);
            Some(super::read_text(path, "reading request body file")?)
        }
        Some(d) => Some(d.clone()),
        None => None,
    };

    let mut headers = std::collections::BTreeMap::new();
    for h in &args.headers {
        if let Some((name, value)) = h.split_once(':') {
            headers.insert(name.trim().to_string(), value.trim().to_string());
        }
    }

    let params = AdHocParams {
        method: args.method.clone(),
        url: args.url.clone(),
        headers,
        body,
        env: args.env.clone(),
    };

    let endpoint = adhoc::build_endpoint(&params)?;
    let context = {
        let dummy = collection.join("mad.md");
        execution_context(&dummy, &args.env, &args.vars)?
    };
    let execution = engine::execute(
        &endpoint,
        &args.env,
        ExecOpts {
            context,
            timeout_ms: args.timeout,
            dry_run: args.dry_run,
        },
    )
    .await?;

    if args.verbose {
        print_report(args.output, &execution)?;
    } else {
        let status = execution
            .response
            .as_ref()
            .map(|r| format!("{}  {}ms", r.status, execution.duration_ms))
            .unwrap_or_else(|| format!("(no response)  {}ms", execution.duration_ms));
        println!("{status}");
        if let Some(r) = &execution.response {
            if !r.body.is_empty() {
                println!("{}", r.body);
            }
        }
    }

    if let Some(save_path) = &args.save {
        let response_block = execution
            .response
            .as_ref()
            .map(|r| {
                let mut block = format!("HTTP/1.1 {}\n", r.status);
                for (k, v) in &r.headers {
                    block.push_str(&format!("{k}: {v}\n"));
                }
                if !r.body.is_empty() {
                    block.push('\n');
                    block.push_str(&r.body);
                }
                block
            })
            .unwrap_or_default();

        match save_path {
            Some(path) => {
                adhoc::save_to_path(path, &params, &response_block)?;
                println!("saved: {}", path.display());
            }
            None => {
                let filename = adhoc::scratch_filename(&args.method, &args.url);
                let dest = collection.join("apis/scratch").join(&filename);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                adhoc::save_to_path(&dest, &params, &response_block)?;
                println!("saved: {}", dest.display());
            }
        }
    } else if !args.dry_run {
        match adhoc::save_to_scratch(&params, "") {
            Ok(path) => eprintln!("scratch: {}", path.display()),
            Err(e) => eprintln!("scratch save skipped: {e}"),
        }
    }

    Ok(())
}
