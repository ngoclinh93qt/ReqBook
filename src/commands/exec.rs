//! `rqb exec` and `rqb flow` commands.

use std::path::Path;

use anyhow::Result;
use reqbook::{
    engine::{self, ExecOpts},
    parser::{parse_endpoint, parse_pipeline},
    pipeline::{self, PipelineOpts},
};

use super::{confirm_production_env, execution_context, print_report, read_text};
use crate::{ExecArgs, FlowArgs, OutputFormat};

pub(crate) async fn exec(args: ExecArgs, yes: bool) -> Result<()> {
    let source = read_text(&args.file, "executing endpoint")?;
    let endpoint = parse_endpoint(&source, &args.file)?;
    engine::ensure_environment_allowed(&endpoint, &args.env)?;
    if !args.dry_run {
        confirm_production_env(&args.env, yes, "execute endpoint")?;
    }
    let context = execution_context(&args.file, &args.env, &args.vars)?;
    let execution = engine::execute(
        &endpoint,
        &args.env,
        ExecOpts {
            context,
            timeout_ms: args.timeout,
            dry_run: args.dry_run,
            strict_assertions: args.strict_assertions,
        },
    )
    .await?;
    print_report(args.output, &execution)
}

pub(crate) async fn flow(args: FlowArgs, yes: bool) -> Result<()> {
    let source = read_text(&args.pipeline, "executing pipeline")?;
    let mut parsed = parse_pipeline(&source, &args.pipeline)?;
    confirm_production_env(&args.env, yes, "run pipeline")?;
    if args.parallel {
        parsed.schema.parallel = true;
    }
    if args.no_parallel {
        parsed.schema.parallel = false;
    }
    let root = args
        .pipeline
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("api-docs"))
        .to_path_buf();
    let context = execution_context(&args.pipeline, &args.env, &args.vars)?;
    let result = pipeline::run(
        &parsed,
        &args.env,
        PipelineOpts {
            root,
            exec: ExecOpts {
                context,
                timeout_ms: args.timeout,
                dry_run: false,
                strict_assertions: args.strict_assertions,
            },
        },
    )
    .await?;
    match args.output {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
        _ => println!(
            "pipeline {}: {} step(s), passed={}",
            parsed.schema.name,
            result.steps.len(),
            result.passed
        ),
    }
    Ok(())
}
