//! `mad check` command.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as AnyhowContext, Result};
use mark_api_down::{
    engine::{self, EngineError, ExecOpts},
    parser::{parse_endpoint, parse_pipeline},
    pipeline::{self, PipelineOpts},
    resolver::ResolveError,
};
use serde::Serialize;

use super::{execution_context, find_api_docs_root, read_text};
use crate::{CheckArgs, CheckReportFormat};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum CheckStatus {
    Pass,
    Fail,
    Warn,
}

#[derive(Debug, Clone, Serialize)]
struct CheckItem {
    status: CheckStatus,
    kind: &'static str,
    file: String,
    method: Option<String>,
    path: Option<String>,
    title: String,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CheckSummary {
    changed_only: bool,
    checked_files: usize,
    changed_endpoints: usize,
    checked_flows: usize,
    passed_contracts: usize,
    changed_response_shape: usize,
    missing_variables: usize,
    items: Vec<CheckItem>,
}

pub(crate) async fn run(args: CheckArgs) -> Result<()> {
    let files = spec_files(&args.path, args.changed_from.as_deref())?;
    let changed_only = args.changed_from.is_some();
    let mut items = Vec::new();

    for file in files {
        if is_flow_file(&file) {
            items.push(check_flow(&file, &args).await);
        } else {
            items.push(check_endpoint(&file, &args).await);
        }
    }

    items.sort_by(|a, b| a.file.cmp(&b.file));
    let summary = summarize(items, changed_only);
    let output = render_report(args.report, &summary)?;
    println!("{output}");

    if summary
        .items
        .iter()
        .any(|item| item.status == CheckStatus::Fail)
    {
        std::process::exit(2);
    }
    Ok(())
}

fn spec_files(path: &Path, changed_from: Option<&str>) -> Result<Vec<PathBuf>> {
    let files = if let Some(base) = changed_from {
        changed_files(base, path)?
    } else {
        super::validate::markdown_files(path)?
    };
    Ok(files
        .into_iter()
        .filter(|path| is_checkable_markdown(path))
        .collect())
}

fn changed_files(base: &str, path: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", base, "--"])
        .arg(path)
        .output()
        .with_context(|| format!("running git diff against {base}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut files: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .collect();
    files.sort();
    Ok(files)
}

async fn check_endpoint(file: &Path, args: &CheckArgs) -> CheckItem {
    let file_display = file.display().to_string();
    let source = match read_text(file, "checking endpoint") {
        Ok(source) => source,
        Err(error) => return fail_item("endpoint", &file_display, None, None, "", error),
    };
    let endpoint = match parse_endpoint(&source, file) {
        Ok(endpoint) => endpoint,
        Err(error) => return fail_item("endpoint", &file_display, None, None, "", error),
    };
    let method = Some(endpoint.schema.method.as_str().to_string());
    let path = Some(endpoint.schema.path.clone());
    let title = endpoint.title.clone();
    let context = match execution_context(file, &args.env, &args.vars) {
        Ok(context) => context,
        Err(error) => return fail_item("endpoint", &file_display, method, path, &title, error),
    };

    match engine::execute(
        &endpoint,
        &args.env,
        ExecOpts {
            context,
            timeout_ms: args.timeout,
            dry_run: false,
            strict_assertions: args.strict_assertions,
        },
    )
    .await
    {
        Ok(execution) if execution.diff.passed => CheckItem {
            status: CheckStatus::Pass,
            kind: "endpoint",
            file: file_display,
            method,
            path,
            title,
            message: None,
        },
        Ok(execution) => CheckItem {
            status: CheckStatus::Fail,
            kind: "endpoint",
            file: file_display,
            method,
            path,
            title,
            message: Some(diff_message(&execution.diff)),
        },
        Err(error) if is_missing_variable(&error) => CheckItem {
            status: CheckStatus::Warn,
            kind: "endpoint",
            file: file_display,
            method,
            path,
            title,
            message: Some(error.to_string()),
        },
        Err(error) => fail_item("endpoint", &file_display, method, path, &title, error),
    }
}

async fn check_flow(file: &Path, args: &CheckArgs) -> CheckItem {
    let file_display = file.display().to_string();
    let source = match read_text(file, "checking flow") {
        Ok(source) => source,
        Err(error) => return fail_item("flow", &file_display, None, None, "", error),
    };
    let flow = match parse_pipeline(&source, file) {
        Ok(flow) => flow,
        Err(error) => return fail_item("flow", &file_display, None, None, "", error),
    };
    let title = flow.title.clone();
    let root = find_api_docs_root(file).unwrap_or_else(|| {
        file.ancestors()
            .nth(2)
            .unwrap_or_else(|| Path::new("api-docs"))
            .to_path_buf()
    });
    let context = match execution_context(file, &args.env, &args.vars) {
        Ok(context) => context,
        Err(error) => return fail_item("flow", &file_display, None, None, &title, error),
    };
    match pipeline::run(
        &flow,
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
    .await
    {
        Ok(result) if result.passed => CheckItem {
            status: CheckStatus::Pass,
            kind: "flow",
            file: file_display,
            method: None,
            path: None,
            title,
            message: None,
        },
        Ok(result) => CheckItem {
            status: CheckStatus::Fail,
            kind: "flow",
            file: file_display,
            method: None,
            path: None,
            title,
            message: Some(format!(
                "{} step(s) failed",
                result
                    .steps
                    .iter()
                    .filter(|step| {
                        step.error.is_some()
                            || step
                                .execution
                                .as_ref()
                                .map(|execution| !execution.diff.passed)
                                .unwrap_or(false)
                    })
                    .count()
            )),
        },
        Err(error) if error.to_string().contains("unresolved variable") => CheckItem {
            status: CheckStatus::Warn,
            kind: "flow",
            file: file_display,
            method: None,
            path: None,
            title,
            message: Some(error.to_string()),
        },
        Err(error) => fail_item("flow", &file_display, None, None, &title, error),
    }
}

fn fail_item(
    kind: &'static str,
    file: &str,
    method: Option<String>,
    path: Option<String>,
    title: &str,
    error: impl std::fmt::Display,
) -> CheckItem {
    CheckItem {
        status: CheckStatus::Fail,
        kind,
        file: file.to_string(),
        method,
        path,
        title: title.to_string(),
        message: Some(error.to_string()),
    }
}

fn summarize(items: Vec<CheckItem>, changed_only: bool) -> CheckSummary {
    let changed_endpoints = items.iter().filter(|item| item.kind == "endpoint").count();
    let checked_flows = items.iter().filter(|item| item.kind == "flow").count();
    let passed_contracts = items
        .iter()
        .filter(|item| item.status == CheckStatus::Pass)
        .count();
    let changed_response_shape = items
        .iter()
        .filter(|item| item.status == CheckStatus::Fail)
        .count();
    let missing_variables = items
        .iter()
        .filter(|item| item.status == CheckStatus::Warn)
        .count();

    CheckSummary {
        changed_only,
        checked_files: items.len(),
        changed_endpoints,
        checked_flows,
        passed_contracts,
        changed_response_shape,
        missing_variables,
        items,
    }
}

fn render_report(format: CheckReportFormat, summary: &CheckSummary) -> Result<String> {
    match format {
        CheckReportFormat::Markdown => Ok(render_markdown(summary)),
        CheckReportFormat::Github => Ok(render_github(summary)),
        CheckReportFormat::Junit => Ok(render_junit(summary)),
        CheckReportFormat::Json => Ok(serde_json::to_string_pretty(summary)?),
    }
}

fn render_markdown(summary: &CheckSummary) -> String {
    let first_label = if summary.changed_only {
        "Changed endpoints"
    } else {
        "Checked endpoints"
    };
    let mut out = format!(
        "API contract check\n\n{first_label}: {}\nChecked flows: {}\nPassed contracts: {}\nChanged response shape: {}\nMissing variables: {}\n",
        summary.changed_endpoints,
        summary.checked_flows,
        summary.passed_contracts,
        summary.changed_response_shape,
        summary.missing_variables
    );
    for item in &summary.items {
        out.push('\n');
        out.push_str("- ");
        out.push_str(status_label(item.status));
        out.push(' ');
        if let (Some(method), Some(path)) = (&item.method, &item.path) {
            out.push_str(method);
            out.push(' ');
            out.push_str(path);
            out.push(' ');
        }
        out.push_str(&item.title);
        if let Some(message) = &item.message {
            out.push_str(" - ");
            out.push_str(one_line(message));
        }
    }
    out
}

fn render_github(summary: &CheckSummary) -> String {
    let mut out = String::new();
    for item in &summary.items {
        match item.status {
            CheckStatus::Fail => out.push_str(&format!(
                "::error file={}::{}\n",
                item.file,
                github_escape(item.message.as_deref().unwrap_or("contract failed"))
            )),
            CheckStatus::Warn => out.push_str(&format!(
                "::warning file={}::{}\n",
                item.file,
                github_escape(item.message.as_deref().unwrap_or("check warning"))
            )),
            CheckStatus::Pass => {}
        }
    }
    out.push_str(&render_markdown(summary));
    out
}

fn render_junit(summary: &CheckSummary) -> String {
    let failures = summary
        .items
        .iter()
        .filter(|item| item.status == CheckStatus::Fail)
        .count();
    let mut out = format!(
        r#"<testsuite name="mad-check" tests="{}" failures="{}">"#,
        summary.items.len(),
        failures
    );
    for item in &summary.items {
        let name = format!(
            "{} {}",
            item.method.as_deref().unwrap_or(item.kind),
            item.path.as_deref().unwrap_or(&item.title)
        );
        out.push_str(&format!(
            r#"<testcase classname="{}" name="{}">"#,
            xml_escape(&item.file),
            xml_escape(&name)
        ));
        if item.status == CheckStatus::Fail {
            out.push_str(&format!(
                "<failure>{}</failure>",
                xml_escape(item.message.as_deref().unwrap_or("contract failed"))
            ));
        }
        out.push_str("</testcase>");
    }
    out.push_str("</testsuite>");
    out
}

fn diff_message(diff: &engine::ResponseDiff) -> String {
    let mut parts = Vec::new();
    if let Some(status) = &diff.status {
        parts.push(format!("status {status}"));
    }
    parts.extend(diff.headers.iter().map(|header| format!("header {header}")));
    if let Some(body) = &diff.body {
        parts.push(body.clone());
    }
    parts.extend(
        diff.assertions
            .iter()
            .map(|assertion| format!("assertion {assertion}")),
    );
    if parts.is_empty() {
        "contract failed".to_string()
    } else {
        parts.join("; ")
    }
}

fn is_missing_variable(error: &EngineError) -> bool {
    matches!(
        error,
        EngineError::Resolve {
            source: ResolveError::MissingVariable { .. },
            ..
        }
    )
}

fn is_checkable_markdown(path: &Path) -> bool {
    if !path.extension().is_some_and(|ext| ext == "md") {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    !matches!(name, "README.md" | "mad.md" | "env.md")
        && !path
            .components()
            .any(|component| matches!(component.as_os_str().to_str(), Some("_shared")))
}

fn is_flow_file(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component.as_os_str().to_str(), Some("flows" | "pipelines")))
}

fn status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "PASS",
        CheckStatus::Fail => "FAIL",
        CheckStatus::Warn => "WARN",
    }
}

fn one_line(input: &str) -> &str {
    input.lines().next().unwrap_or(input)
}

fn github_escape(input: &str) -> String {
    one_line(input)
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_report_is_deterministic() {
        let summary = summarize(
            vec![CheckItem {
                status: CheckStatus::Pass,
                kind: "endpoint",
                file: "api-docs/apis/users/get-user.md".to_string(),
                method: Some("GET".to_string()),
                path: Some("/users/:userId".to_string()),
                title: "Get user".to_string(),
                message: None,
            }],
            true,
        );
        let report = render_markdown(&summary);
        assert!(report.contains("Changed endpoints: 1"));
        assert!(report.contains("- PASS GET /users/:userId Get user"));
    }

    #[test]
    fn junit_report_escapes_failure_messages() {
        let summary = summarize(
            vec![CheckItem {
                status: CheckStatus::Fail,
                kind: "endpoint",
                file: "a.md".to_string(),
                method: Some("POST".to_string()),
                path: Some("/users".to_string()),
                title: "Create user".to_string(),
                message: Some("expected < got >".to_string()),
            }],
            false,
        );
        let report = render_junit(&summary);
        assert!(report.contains("&lt;"));
        assert!(report.contains("failures=\"1\""));
    }
}
