//! `mad context` command.

use anyhow::Result;
use mark_api_down::agent_context::{self, AgentContextOptions};

use crate::ContextArgs;

pub(crate) fn run(args: ContextArgs) -> Result<()> {
    let target = match args.target.as_slice() {
        [] => None,
        [single] => Some(single.clone()),
        [kind, value] if matches!(kind.as_str(), "flow" | "endpoint") => Some(value.clone()),
        _ => anyhow::bail!("mad context accepts <target> or flow <name>"),
    };
    let rendered = agent_context::render(AgentContextOptions {
        root: args.root,
        target,
        changed_from: args.changed_from,
        token_budget: args.token_budget,
        verbose: args.verbose,
        env: args.env,
    })?;
    println!("{rendered}");
    Ok(())
}
