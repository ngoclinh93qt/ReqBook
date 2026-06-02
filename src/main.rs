mod commands;

use std::{
    io::{self},
    path::PathBuf,
};

use anyhow::Result;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use mark_api_down::workspace;

use commands::{doctor, exec, import, init, install, regenerate_index, request, serve, validate};

// ─── CLI types ────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
#[command(
    name = "mad",
    version,
    about = "API workspace   design specs, send requests, validate contracts"
)]
struct Cli {
    /// Path to api-docs/mad.md.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Disable colored output.
    #[arg(long, global = true)]
    no_color: bool,
    /// Enable verbose diagnostics.
    #[arg(short, long, global = true)]
    verbose: bool,
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
    /// Execute a pipeline.
    Flow(FlowArgs),
    /// Regenerate api-docs/README.md.
    Index,
    /// Import specs from another API tool.
    Import {
        #[command(subcommand)]
        command: ImportCommand,
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
    /// Accept defaults and do not prompt.
    #[arg(long)]
    pub(crate) yes: bool,
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
    /// Register the MarkApiDown MCP server with Claude Code.
    Mcp,
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
    /// Remove installed MarkApiDown skill and slash-command files.
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

// ─── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.no_color {
        std::env::set_var("NO_COLOR", "1");
    }
    let collection = workspace::collection_root(cli.config.as_deref());
    match cli.command {
        Command::Init(args) => {
            init::run(args, &collection)?;
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
            println!("\nRun `mad serve` to open the web preview.");
        }
        Command::Validate { path } => validate::run(path)?,
        Command::Exec(args) => exec::exec(args).await?,
        Command::Flow(args) => exec::flow(args).await?,
        Command::Index => regenerate_index(&collection)?,
        Command::Import { command } => import::run(command, &collection).await?,
        Command::Install { command } => install::install(command).await?,
        Command::Skills { command } => install::skills(command).await?,
        Command::Serve(args) => serve::serve(args, &collection).await?,
        Command::Mock(args) => serve::mock(args).await?,
        Command::Request(args) => request::run(args, &collection).await?,
        Command::Mcp => mark_api_down::mcp::run_mcp_server().await?,
        Command::Doctor(args) => doctor::run(args, &collection)?,
        Command::Completion { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "mad", &mut io::stdout());
        }
        Command::Version => println!("{}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
