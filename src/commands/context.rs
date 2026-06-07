//! `rqb context` command.

use anyhow::Result;
use reqbook::agent_context::{self, AgentContextOptions, ContextMode, ContextSections};
use serde_json::json;

use crate::{ContextArgs, ContextOutputFormat};

pub(crate) struct ContextRenderOptions {
    pub(crate) root: std::path::PathBuf,
    pub(crate) targets: Vec<String>,
    pub(crate) changed_from: Option<String>,
    pub(crate) token_budget: usize,
    pub(crate) mode: ContextMode,
    pub(crate) intent: Option<String>,
    pub(crate) max_fields: usize,
    pub(crate) sections: ContextSections,
    pub(crate) verbose: bool,
    pub(crate) env: String,
}

pub(crate) fn run(args: ContextArgs) -> Result<()> {
    let targets = normalize_targets(&args.target)?;
    if args.changed_from.is_some() && !targets.is_empty() {
        anyhow::bail!("use either explicit targets or --changed-from, not both");
    }

    let root = args.root.clone();
    let env = args.env.clone();
    let changed_from = args.changed_from.clone();
    let token_budget = args.token_budget;
    let mode = ContextMode::parse(&args.mode)?;
    let sections = ContextSections::parse(args.include.as_deref(), args.brief, args.no_guidance)?;
    let verbose = args.verbose;
    let rendered = render_markdown(ContextRenderOptions {
        root: root.clone(),
        targets: targets.clone(),
        changed_from: changed_from.clone(),
        token_budget,
        mode,
        intent: args.intent.clone(),
        max_fields: args.max_fields,
        sections,
        verbose,
        env: env.clone(),
    })?;
    match args.output {
        ContextOutputFormat::Markdown => println!("{rendered}"),
        ContextOutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "root": root,
                "env": env,
                "targets": targets,
                "changed_from": changed_from,
                "token_budget": token_budget,
                "mode": mode.as_str(),
                "intent": args.intent,
                "brief": args.brief,
                "max_fields": args.max_fields,
                "sections": sections.names(),
                "verbose": verbose,
                "content": rendered,
            }))?
        ),
    }
    Ok(())
}

pub(crate) fn render_markdown(options: ContextRenderOptions) -> Result<String> {
    let ContextRenderOptions {
        root,
        targets,
        changed_from,
        token_budget,
        mode,
        intent,
        max_fields,
        sections,
        verbose,
        env,
    } = options;

    if changed_from.is_some() && !targets.is_empty() {
        anyhow::bail!("use either explicit targets or --changed-from, not both");
    }
    if targets.len() <= 1 {
        agent_context::render(AgentContextOptions {
            root,
            target: targets.first().cloned(),
            changed_from,
            token_budget,
            verbose,
            env,
            mode,
            intent,
            max_fields,
            sections,
        })
    } else {
        let per_target_budget = (token_budget / targets.len()).max(128);
        let mut parts = Vec::new();
        for target in targets {
            parts.push(agent_context::render(AgentContextOptions {
                root: root.clone(),
                target: Some(target),
                changed_from: None,
                token_budget: per_target_budget,
                verbose,
                env: env.clone(),
                mode,
                intent: intent.clone(),
                max_fields,
                sections,
            })?);
        }
        Ok(parts.join("\n\n---\n\n"))
    }
}

pub(crate) fn normalize_targets(raw: &[String]) -> Result<Vec<String>> {
    match raw {
        [] => Ok(Vec::new()),
        [kind, value] if matches!(kind.as_str(), "flow" | "endpoint") => Ok(vec![value.clone()]),
        [kind] if matches!(kind.as_str(), "flow" | "endpoint") => {
            anyhow::bail!("rqb context {kind} requires a target name")
        }
        values
            if values
                .iter()
                .any(|value| matches!(value.as_str(), "flow" | "endpoint")) =>
        {
            anyhow::bail!("use `rqb context flow <name>`, `rqb context endpoint <id>`, or pass plain targets without kind markers")
        }
        values => Ok(values.to_vec()),
    }
}
