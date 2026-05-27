//! Report formatting.

use anyhow::Result;
use owo_colors::OwoColorize;
use serde_json::json;

use crate::engine::Execution;

/// Execution reporter.
pub trait Reporter {
    /// Render a report for one execution.
    fn report(&self, result: &Execution) -> Result<String>;
}

/// Human-readable console reporter.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn report(&self, result: &Execution) -> Result<String> {
        let status = result
            .response
            .as_ref()
            .map_or("DRY RUN".to_string(), |response| {
                response.status.to_string()
            });
        let status_text = if std::env::var_os("NO_COLOR").is_some() {
            status
        } else if result.diff.passed {
            status.green().to_string()
        } else {
            status.red().to_string()
        };
        let mut out = format!(
            "{} {}\nstatus: {}\nduration: {}ms",
            result.request.method, result.request.url, status_text, result.duration_ms
        );
        if result.response.is_none() {
            if !result.request.headers.is_empty() {
                out.push_str("\nheaders:");
                for (name, value) in &result.request.headers {
                    out.push_str(&format!("\n  {name}: {value}"));
                }
            }
            if !result.request.body.is_empty() {
                out.push_str("\nbody:\n");
                out.push_str(&result.request.body);
            }
        } else if !result.diff.passed {
            if let Some(status) = &result.diff.status {
                out.push_str(&format!("\ndiff.status: {status}"));
            }
            for header in &result.diff.headers {
                out.push_str(&format!("\ndiff.header: {header}"));
            }
            if let Some(body) = &result.diff.body {
                out.push_str(&format!("\ndiff.body: {body}"));
            }
        }
        Ok(out)
    }
}

/// JUnit XML reporter.
#[derive(Debug, Clone, Copy, Default)]
pub struct JunitReporter;

impl Reporter for JunitReporter {
    fn report(&self, result: &Execution) -> Result<String> {
        let failure = if result.diff.passed {
            String::new()
        } else {
            format!(
                "<failure>{}</failure>",
                xml_escape(&format!("{:?}", result.diff))
            )
        };
        Ok(format!(
            r#"<testsuite tests="1" failures="{}"><testcase name="{} {}">{}</testcase></testsuite>"#,
            u8::from(!result.diff.passed),
            xml_escape(&result.request.method),
            xml_escape(&result.request.url),
            failure
        ))
    }
}

/// JSON reporter.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonReporter;

impl Reporter for JsonReporter {
    fn report(&self, result: &Execution) -> Result<String> {
        Ok(serde_json::to_string_pretty(result)?)
    }
}

/// Markdown reporter.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownReporter;

impl Reporter for MarkdownReporter {
    fn report(&self, result: &Execution) -> Result<String> {
        let payload = json!({
            "request": result.request,
            "response": result.response,
            "diff": result.diff,
        });
        Ok(format!(
            "# Trellis execution\n\n- Method: `{}`\n- URL: `{}`\n- Duration: `{}` ms\n\n```json\n{}\n```",
            result.request.method,
            result.request.url,
            result.duration_ms,
            serde_json::to_string_pretty(&payload)?
        ))
    }
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
    use crate::engine::{CapturedRequest, Execution, ResponseDiff};

    use super::*;

    fn execution() -> Execution {
        Execution {
            request: CapturedRequest {
                method: "GET".to_string(),
                url: "https://example.com".to_string(),
                headers: Default::default(),
                body: String::new(),
            },
            response: None,
            duration_ms: 1,
            diff: ResponseDiff {
                passed: true,
                ..ResponseDiff::default()
            },
        }
    }

    #[test]
    fn renders_json_report() {
        let report = JsonReporter.report(&execution()).unwrap();
        assert!(report.contains("https://example.com"));
    }

    #[test]
    fn renders_junit_report() {
        let report = JunitReporter.report(&execution()).unwrap();
        assert!(report.contains("testsuite"));
    }
}
