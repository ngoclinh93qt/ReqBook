//! `rqb diagnose` command.

use anyhow::Result;
use reqbook::diagnose::Diagnosis;

use super::{confirm_production_env, execution_context};
use crate::{DiagnoseArgs, DiagnoseOutputFormat};

pub(crate) async fn run(args: DiagnoseArgs, yes: bool) -> Result<()> {
    confirm_production_env(&args.env, yes, "diagnose endpoint")?;
    let context = execution_context(&args.file, &args.env, &args.vars)?;
    let diagnosis = reqbook::diagnose::diagnose_endpoint(
        &args.file,
        &args.env,
        context,
        args.timeout,
        args.strict_assertions,
    )
    .await?;

    match args.output {
        DiagnoseOutputFormat::Json => println!("{}", serde_json::to_string_pretty(&diagnosis)?),
        DiagnoseOutputFormat::Console => print_console(&diagnosis)?,
    }

    Ok(())
}

fn print_console(diagnosis: &Diagnosis) -> Result<()> {
    println!("passed: {}", diagnosis.passed);
    if let Some(status) = diagnosis.status {
        println!("status: {status}");
    }
    if let Some(error_type) = &diagnosis.error_type {
        println!("error_type: {error_type}");
    }
    println!("summary: {}", diagnosis.summary);
    println!("likely_cause: {}", diagnosis.likely_cause);
    println!("next_action: {}", diagnosis.next_action);

    if !diagnosis.inspect.is_empty() {
        println!("inspect:");
        for item in &diagnosis.inspect {
            println!("- {item}");
        }
    }

    if !diagnosis.verify.is_empty() {
        println!("verify:");
        for command in &diagnosis.verify {
            println!("- {command}");
        }
    }

    if !diagnosis.diff.is_null() {
        println!("diff: {}", serde_json::to_string(&diagnosis.diff)?);
    }

    Ok(())
}
