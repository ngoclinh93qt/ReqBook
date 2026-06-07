mod commands;

use std::{
    io::{self},
    path::PathBuf,
};

use anyhow::Result;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use reqbook::workspace;

use commands::{
    agent, check, context, diagnose, doctor, exec, export, import, init, install, regenerate_index,
    request, serve, validate,
};

// ─── CLI types ────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
#[command(
    name = "rqb",
    version,
    about = "API workspace   design specs, send requests, validate contracts"
)]
struct Cli {
    /// Path to api-docs/reqbook.md.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Disable colored output.
    #[arg(long, global = true)]
    no_color: bool,
    /// Enable verbose diagnostics.
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Accept safety confirmations and non-interactive defaults.
    #[arg(long, global = true)]
    yes: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scaffold api-docs/.
    Init(InitArgs),
    /// Validate a file or directory.
    Validate { path: PathBuf },
    /// Execute one endpoint.
    Exec(ExecArgs),
    /// Run one endpoint and return an agent-facing failure diagnosis.
    Diagnose(DiagnoseArgs),
    /// Execute a pipeline.
    Flow(FlowArgs),
    /// Run contract checks for endpoint and flow specs.
    Check(CheckArgs),
    /// Print bounded executable API context for coding agents.
    Context(ContextArgs),
    /// Create agent-ready API context packs.
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Regenerate api-docs/README.md.
    Index,
    /// Import specs from another API tool.
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
    /// Export Reqbook specs to another API format.
    Export {
        #[command(subcommand)]
        command: ExportCommand,
    },
    /// Install skills, slash commands, or the MCP server for AI agent integration.
    Install {
        #[command(subcommand)]
        command: InstallCommand,
    },
    /// Install, list, or remove AI agent skills.
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    /// Launch web preview.
    Serve(ServeArgs),
    /// Start mock HTTP server from recorded spec responses.
    Mock(MockArgs),
    /// Start MCP server (stdio transport) for AI agent tool integration.
    Mcp,
    /// Diagnose project setup.
    Doctor(DoctorArgs),
    /// Generate shell completion.
    Completion { shell: Shell },
    /// Send an ad-hoc HTTP request without a spec file.
    Request(RequestArgs),
    /// Print version information.
    Version,
}

#[derive(Debug, Args)]
pub(crate) struct RequestArgs {
    /// HTTP method (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS).
    pub(crate) method: String,
    /// Target URL (may contain {{variable}} references).
    pub(crate) url: String,
    /// Add a request header as `Name: Value`. Repeatable.
    #[arg(short = 'H', long = "header")]
    pub(crate) headers: Vec<String>,
    /// Request body string or @file to read from a file.
    #[arg(short = 'd', long = "data")]
    pub(crate) data: Option<String>,
    /// Environment for variable resolution.
    #[arg(long, default_value = "dev")]
    pub(crate) env: String,
    /// Inject a variable as key=value. Repeatable.
    #[arg(long = "var")]
    pub(crate) vars: Vec<String>,
    /// Print resolved request without sending.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Timeout override in milliseconds.
    #[arg(long)]
    pub(crate) timeout: Option<u64>,
    /// Save as a spec file. Omit path to auto-save to current collection.
    #[arg(long)]
    pub(crate) save: Option<Option<PathBuf>>,
    /// Full diff output (default: compact status + body).
    #[arg(long)]
    pub(crate) verbose: bool,
    /// Output format (used with --verbose).
    #[arg(long, default_value = "console")]
    pub(crate) output: OutputFormat,
}

#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    /// Project name.
    #[arg(long)]
    pub(crate) name: Option<String>,
    /// Development base URL.
    #[arg(long)]
    pub(crate) dev_url: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ExecArgs {
    /// Endpoint markdown file.
    pub(crate) file: PathBuf,
    /// Environment.
    #[arg(long, default_value = "dev")]
    pub(crate) env: String,
    /// Output format.
    #[arg(long, default_value = "console")]
    pub(crate) output: OutputFormat,
    /// Inject a variable as key=value. Repeatable.
    #[arg(long = "var")]
    pub(crate) vars: Vec<String>,
    /// Print resolved request without sending it.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Timeout override in milliseconds.
    #[arg(long)]
    pub(crate) timeout: Option<u64>,
    /// Treat failing `## Assertions` rules as execution failures.
    #[arg(long)]
    pub(crate) strict_assertions: bool,
}

#[derive(Debug, Args)]
pub(crate) struct DiagnoseArgs {
    /// Endpoint markdown file.
    pub(crate) file: PathBuf,
    /// Environment.
    #[arg(long, default_value = "dev")]
    pub(crate) env: String,
    /// Output format.
    #[arg(long, default_value = "console")]
    pub(crate) output: DiagnoseOutputFormat,
    /// Inject a variable as key=value. Repeatable.
    #[arg(long = "var")]
    pub(crate) vars: Vec<String>,
    /// Timeout override in milliseconds.
    #[arg(long)]
    pub(crate) timeout: Option<u64>,
    /// Treat failing `## Assertions` rules as execution failures.
    #[arg(long)]
    pub(crate) strict_assertions: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FlowArgs {
    /// Pipeline markdown file.
    pub(crate) pipeline: PathBuf,
    /// Environment.
    #[arg(long, default_value = "dev")]
    pub(crate) env: String,
    /// Output format.
    #[arg(long, default_value = "console")]
    pub(crate) output: OutputFormat,
    /// Inject a variable as key=value. Repeatable.
    #[arg(long = "var")]
    pub(crate) vars: Vec<String>,
    /// Force parallel execution.
    #[arg(long)]
    pub(crate) parallel: bool,
    /// Force sequential execution.
    #[arg(long)]
    pub(crate) no_parallel: bool,
    /// Timeout override in milliseconds.
    #[arg(long)]
    pub(crate) timeout: Option<u64>,
    /// Print resolved step requests without sending them.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Treat failing `## Assertions` rules as execution failures.
    #[arg(long)]
    pub(crate) strict_assertions: bool,
}

#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// File or api-docs/ directory to check.
    #[arg(default_value = "api-docs")]
    pub(crate) path: PathBuf,
    /// Only check files changed since this git ref.
    #[arg(long)]
    pub(crate) changed_from: Option<String>,
    /// Report format.
    #[arg(long, default_value = "markdown")]
    pub(crate) report: CheckReportFormat,
    /// Environment.
    #[arg(long, default_value = "dev")]
    pub(crate) env: String,
    /// Inject a variable as key=value. Repeatable.
    #[arg(long = "var")]
    pub(crate) vars: Vec<String>,
    /// Timeout override in milliseconds.
    #[arg(long)]
    pub(crate) timeout: Option<u64>,
    /// Treat failing `## Assertions` rules as contract failures.
    #[arg(long)]
    pub(crate) strict_assertions: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ContextArgs {
    /// Endpoint/flow ids or file paths, e.g. `users.create posts.create` or `flow checkout`.
    pub(crate) target: Vec<String>,
    /// Only summarize files changed since this git ref.
    #[arg(long)]
    pub(crate) changed_from: Option<String>,
    /// api-docs/ root directory.
    #[arg(long, default_value = "api-docs")]
    pub(crate) root: PathBuf,
    /// Approximate output token budget.
    #[arg(long, default_value_t = 800)]
    pub(crate) token_budget: usize,
    /// Context output mode: surgical, compact, or schema.
    #[arg(long, default_value = "surgical")]
    pub(crate) mode: String,
    /// Agent task intent: implement, debug, test, review, or document.
    #[arg(long)]
    pub(crate) intent: Option<String>,
    /// Token-optimized output: omit title/guidance and keep only executable contract sections.
    #[arg(long)]
    pub(crate) brief: bool,
    /// Maximum request/response fields to include per section.
    #[arg(long, default_value_t = 8)]
    pub(crate) max_fields: usize,
    /// Comma-separated sections: title,variables,request,response,errors,assertions,rules,verify,guidance,all.
    #[arg(long)]
    pub(crate) include: Option<String>,
    /// Omit agent workflow guidance text.
    #[arg(long)]
    pub(crate) no_guidance: bool,
    /// Include full request and expected response blocks.
    #[arg(long)]
    pub(crate) verbose: bool,
    /// Environment used in suggested next commands.
    #[arg(long, default_value = "dev")]
    pub(crate) env: String,
    /// Output format.
    #[arg(long, default_value = "markdown")]
    pub(crate) output: ContextOutputFormat,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AgentCommand {
    /// Write an executable context pack for coding agents.
    Pack(AgentPackArgs),
}

#[derive(Debug, Args)]
pub(crate) struct AgentPackArgs {
    /// Endpoint/flow ids or file paths, e.g. `users.create posts.create` or `flow checkout`.
    pub(crate) target: Vec<String>,
    /// Only summarize files changed since this git ref.
    #[arg(long)]
    pub(crate) changed_from: Option<String>,
    /// api-docs/ root directory.
    #[arg(long, default_value = "api-docs")]
    pub(crate) root: PathBuf,
    /// Output markdown file.
    #[arg(long, default_value = ".reqbook/agent-context.md")]
    pub(crate) out: PathBuf,
    /// Approximate output token budget.
    #[arg(long, default_value_t = 1600)]
    pub(crate) token_budget: usize,
    /// Context output mode: surgical, compact, or schema.
    #[arg(long, default_value = "surgical")]
    pub(crate) mode: String,
    /// Agent task intent: implement, debug, test, review, or document.
    #[arg(long)]
    pub(crate) intent: Option<String>,
    /// Token-optimized output: omit title/guidance and keep only executable contract sections.
    #[arg(long)]
    pub(crate) brief: bool,
    /// Maximum request/response fields to include per section.
    #[arg(long, default_value_t = 8)]
    pub(crate) max_fields: usize,
    /// Comma-separated sections: title,variables,request,response,errors,assertions,rules,verify,guidance,all.
    #[arg(long)]
    pub(crate) include: Option<String>,
    /// Omit agent workflow guidance text.
    #[arg(long)]
    pub(crate) no_guidance: bool,
    /// Include full request, expected response, agent-task, and notes blocks.
    #[arg(long)]
    pub(crate) verbose: bool,
    /// Environment used in suggested next commands.
    #[arg(long, default_value = "dev")]
    pub(crate) env: String,
}

#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    /// Project path.
    pub(crate) path: Option<PathBuf>,
    /// Port.
    #[arg(long, default_value_t = 8080)]
    pub(crate) port: u16,
    /// Host.
    #[arg(long, default_value = "127.0.0.1")]
    pub(crate) host: String,
    /// Environment.
    #[arg(long, default_value = "dev")]
    pub(crate) env: String,
    /// Start in mock mode: serve recorded responses instead of real HTTP requests.
    #[arg(long, default_value_t = false)]
    pub(crate) mock: bool,
}

#[derive(Debug, Args)]
pub(crate) struct MockArgs {
    /// api-docs/ root directory containing the spec files.
    #[arg(default_value = "api-docs")]
    pub(crate) dir: PathBuf,
    /// TCP port to listen on.
    #[arg(long, default_value_t = 4001)]
    pub(crate) port: u16,
    /// Artificial response delay in milliseconds.
    #[arg(long)]
    pub(crate) latency: Option<u64>,
}

#[derive(Debug, Args)]
pub(crate) struct DoctorArgs {
    /// Automatically fix supported issues.
    #[arg(long)]
    pub(crate) fix: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ImportCommand {
    /// Import Postman Collection v2.1 JSON.
    Postman { file: PathBuf },
    /// Import Insomnia v4 JSON.
    Insomnia { file: PathBuf },
    /// Import OpenAPI 3.x YAML or JSON.
    Openapi { file: PathBuf },
    /// Import a local API client collection directory.
    Collection { path: PathBuf },
    /// Import a `.http` / REST Client request file.
    Http { file: PathBuf },
    /// Import a raw curl command (paste from browser DevTools).
    Curl {
        /// File containing the curl command (omit to read from stdin).
        file: Option<PathBuf>,
    },
    /// Scan project source code for route definitions and import them.
    Project {
        /// Root directory to scan (default: current directory).
        path: Option<PathBuf>,
        /// Port of a running development server.
        #[arg(long)]
        port: Option<u16>,
        /// Explicit OpenAPI/Swagger spec URL.
        #[arg(long)]
        url: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ExportCommand {
    /// Export endpoint specs as OpenAPI 3.x YAML or JSON.
    Openapi {
        /// api-docs/ root directory.
        path: PathBuf,
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Emit JSON instead of YAML.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum InstallCommand {
    /// Install AI agent skills (all or by name).
    Skills {
        name: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    /// Install slash commands for Claude Code and Codex CLI (all or by name).
    Slashcmd {
        name: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    /// Register the Reqbook MCP server with an AI agent.
    Mcp {
        /// Agent name. Installs for all detected agents if omitted.
        #[arg(long)]
        agent: Option<String>,
    },
    /// List detected agents and installation status.
    List,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SkillsCommand {
    /// Install AI agent skills (all or by name).
    Install {
        name: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    /// List detected agents and installation status.
    List,
    /// Remove installed Reqbook skill and slash-command files.
    Uninstall {
        name: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
    Console,
    Junit,
    Json,
    Markdown,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ContextOutputFormat {
    Markdown,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum DiagnoseOutputFormat {
    Console,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CheckReportFormat {
    Markdown,
    Github,
    Junit,
    Json,
}

// ─── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.no_color {
        std::env::set_var("NO_COLOR", "1");
    }
    let yes = cli.yes;
    let collection = workspace::collection_root(cli.config.as_deref());
    match cli.command {
        Command::Init(args) => {
            init::run(args, &collection, yes)?;
            println!("\nScanning for existing API routes...");
            import::run(
                ImportCommand::Project {
                    path: None,
                    port: None,
                    url: None,
                },
                &collection,
            )
            .await?;
            println!("\nRun `rqb serve` to open the web preview.");
        }
        Command::Validate { path } => validate::run(path)?,
        Command::Exec(args) => exec::exec(args, yes).await?,
        Command::Diagnose(args) => diagnose::run(args, yes).await?,
        Command::Flow(args) => exec::flow(args, yes).await?,
        Command::Check(args) => check::run(args, yes).await?,
        Command::Context(args) => context::run(args)?,
        Command::Agent { command } => match command {
            AgentCommand::Pack(args) => agent::pack(args)?,
        },
        Command::Index => regenerate_index(&collection)?,
        Command::Import { command } => import::run(command, &collection).await?,
        Command::Export { command } => export::run(command)?,
        Command::Install { command } => install::install(command).await?,
        Command::Skills { command } => install::skills(command).await?,
        Command::Serve(args) => serve::serve(args, &collection).await?,
        Command::Mock(args) => serve::mock(args).await?,
        Command::Request(args) => request::run(args, &collection, yes).await?,
        Command::Mcp => reqbook::mcp::run_mcp_server().await?,
        Command::Doctor(args) => doctor::run(args, &collection)?,
        Command::Completion { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "rqb", &mut io::stdout());
        }
        Command::Version => println!("{}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
