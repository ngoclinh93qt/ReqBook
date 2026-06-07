//! Agent workflow commands.

use std::{fs, path::Path};

use anyhow::{Context as AnyhowContext, Result};
use reqbook::agent_context::{ContextMode, ContextSections};

use crate::AgentPackArgs;

use super::context::{normalize_targets, render_markdown, ContextRenderOptions};

pub(crate) fn pack(args: AgentPackArgs) -> Result<()> {
    let targets = normalize_targets(&args.target)?;
    let mode = ContextMode::parse(&args.mode)?;
    let sections = ContextSections::parse(args.include.as_deref(), args.brief, args.no_guidance)?;
    let content = render_markdown(ContextRenderOptions {
        root: args.root.clone(),
        targets: targets.clone(),
        changed_from: args.changed_from.clone(),
        token_budget: args.token_budget,
        mode,
        intent: args.intent.clone(),
        max_fields: args.max_fields,
        sections,
        verbose: args.verbose,
        env: args.env.clone(),
    })?;
    let commands = safe_next_commands(&content);
    let pack = render_pack(&args, &targets, &content, &commands);
    write_pack(&args.out, &pack)?;
    println!("agent pack written: {}", args.out.display());
    Ok(())
}

fn render_pack(
    args: &AgentPackArgs,
    targets: &[String],
    content: &str,
    commands: &[String],
) -> String {
    let target_text = if targets.is_empty() {
        args.changed_from
            .as_deref()
            .map(|base| format!("changed-from {base}"))
            .unwrap_or_else(|| "unspecified".to_string())
    } else {
        targets.join(", ")
    };
    let command_text = if commands.is_empty() {
        "- `rqb validate api-docs`\n".to_string()
    } else {
        commands
            .iter()
            .map(|command| format!("- `{command}`"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };

    format!(
        r#"# Reqbook Agent Pack

Use this pack as executable API context for Codex, Claude, Cursor, or another coding agent.

## Scope

- Root: `{}`
- Env: `{}`
- Targets: `{}`
- Token budget: `{}`
- Mode: `{}`
- Intent: `{}`
- Brief: `{}`
- Max fields: `{}`
- Sections: `{}`
- Verbose: `{}`

## Guardrails

- Run `rqb validate {}` before changing or executing specs.
- Use `rqb flow <file> --dry-run --output json` before sending multi-step flows to real services.
- Do not pass `--yes` for production-like environments until the resolved requests have been reviewed.
- Treat any non-zero `rqb exec`, `rqb flow`, or `rqb check` exit as unfinished work.

## Suggested Verify Commands

{}
## Context

```text
{}
```
"#,
        args.root.display(),
        args.env,
        target_text,
        args.token_budget,
        args.mode,
        args.intent.as_deref().unwrap_or("implement"),
        args.brief,
        args.max_fields,
        ContextSections::parse(args.include.as_deref(), args.brief, args.no_guidance)
            .map(|sections| sections.names().join(", "))
            .unwrap_or_else(|_| "invalid".to_string()),
        args.verbose,
        args.root.display(),
        command_text,
        content.trim()
    )
}

fn safe_next_commands(content: &str) -> Vec<String> {
    let mut commands = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("Safe next command: ")
                .or_else(|| line.strip_prefix("- rqb "))
                .map(|command| {
                    if line.starts_with("- rqb ") {
                        format!("rqb {command}")
                    } else {
                        command.to_string()
                    }
                })
        })
        .map(|command| command.trim().to_string())
        .filter(|command| !command.is_empty())
        .collect::<Vec<_>>();
    commands.sort();
    commands.dedup();
    commands
}

fn write_pack(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn extracts_safe_next_commands() {
        let commands = safe_next_commands(
            "Endpoint: GET /users\nSafe next command: rqb exec api-docs/apis/users/get.md --env dev\n\nSafe next command: rqb flow api-docs/flows/demo.md --env dev\n",
        );
        assert_eq!(
            commands,
            vec![
                "rqb exec api-docs/apis/users/get.md --env dev",
                "rqb flow api-docs/flows/demo.md --env dev"
            ]
        );
    }

    #[test]
    fn render_pack_includes_guardrails() {
        let args = AgentPackArgs {
            target: vec!["users.get".to_string()],
            changed_from: None,
            root: PathBuf::from("api-docs"),
            out: PathBuf::from(".reqbook/agent-context.md"),
            token_budget: 800,
            mode: "surgical".to_string(),
            intent: Some("implement".to_string()),
            brief: false,
            max_fields: 8,
            include: None,
            no_guidance: false,
            verbose: true,
            env: "dev".to_string(),
        };
        let pack = render_pack(
            &args,
            &["users.get".to_string()],
            "Endpoint: GET /users",
            &["rqb exec api-docs/apis/users/get.md --env dev".to_string()],
        );
        assert!(pack.contains("## Guardrails"));
        assert!(pack.contains("rqb flow <file> --dry-run --output json"));
        assert!(pack.contains("Endpoint: GET /users"));
    }
}
