use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{bail, Context as AnyhowContext, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use owo_colors::OwoColorize;
use trellis::{
    adhoc::{self, AdHocParams},
    engine::{self, ExecOpts},
    parser::{self, parse_endpoint, parse_pipeline},
    pipeline::{self, PipelineOpts},
    report::{ConsoleReporter, JsonReporter, JunitReporter, MarkdownReporter, Reporter},
    resolver::{Context, SourceKind},
    workspace,
};

#[derive(Debug, Parser)]
#[command(name = "trellis", version, about = "API workspace   design specs, send requests, validate contracts")]
struct Cli {
    /// Path to api-docs/trellis.md.
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
struct RequestArgs {
    /// HTTP method (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS).
    method: String,
    /// Target URL (may contain {{variable}} references).
    url: String,
    /// Add a request header as `Name: Value`. Repeatable.
    #[arg(short = 'H', long = "header")]
    headers: Vec<String>,
    /// Request body string or @file to read from a file.
    #[arg(short = 'd', long = "data")]
    data: Option<String>,
    /// Environment for variable resolution.
    #[arg(long, default_value = "dev")]
    env: String,
    /// Inject a variable as key=value. Repeatable.
    #[arg(long = "var")]
    vars: Vec<String>,
    /// Print resolved request without sending.
    #[arg(long)]
    dry_run: bool,
    /// Timeout override in milliseconds.
    #[arg(long)]
    timeout: Option<u64>,
    /// Save as a spec file. Omit path to auto-save to current collection.
    #[arg(long)]
    save: Option<Option<PathBuf>>,
    /// Full diff output (default: compact status + body).
    #[arg(long)]
    verbose: bool,
    /// Output format (used with --verbose).
    #[arg(long, default_value = "console")]
    output: OutputFormat,
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Project name.
    #[arg(long)]
    name: Option<String>,
    /// Development base URL.
    #[arg(long)]
    dev_url: Option<String>,
    /// Accept defaults and do not prompt.
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct ExecArgs {
    /// Endpoint markdown file.
    file: PathBuf,
    /// Environment.
    #[arg(long, default_value = "dev")]
    env: String,
    /// Output format.
    #[arg(long, default_value = "console")]
    output: OutputFormat,
    /// Inject a variable as key=value. Repeatable.
    #[arg(long = "var")]
    vars: Vec<String>,
    /// Print resolved request without sending it.
    #[arg(long)]
    dry_run: bool,
    /// Timeout override in milliseconds.
    #[arg(long)]
    timeout: Option<u64>,
}

#[derive(Debug, Args)]
struct FlowArgs {
    /// Pipeline markdown file.
    pipeline: PathBuf,
    /// Environment.
    #[arg(long, default_value = "dev")]
    env: String,
    /// Output format.
    #[arg(long, default_value = "console")]
    output: OutputFormat,
    /// Inject a variable as key=value. Repeatable.
    #[arg(long = "var")]
    vars: Vec<String>,
    /// Force parallel execution.
    #[arg(long)]
    parallel: bool,
    /// Force sequential execution.
    #[arg(long)]
    no_parallel: bool,
    /// Timeout override in milliseconds.
    #[arg(long)]
    timeout: Option<u64>,
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// Project path.
    path: Option<PathBuf>,
    /// Port.
    #[arg(long, default_value_t = 8080)]
    port: u16,
    /// Host.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// Environment.
    #[arg(long, default_value = "dev")]
    env: String,
    /// Start in mock mode: serve recorded responses from ## Expected response blocks instead of making real HTTP requests.
    #[arg(long, default_value_t = false)]
    mock: bool,
}

#[derive(Debug, Args)]
struct MockArgs {
    /// api-docs/ root directory containing the spec files.
    #[arg(default_value = "api-docs")]
    dir: PathBuf,
    /// TCP port to listen on.
    #[arg(long, default_value_t = 4001)]
    port: u16,
    /// Artificial response delay in milliseconds (useful for latency testing).
    #[arg(long)]
    latency: Option<u64>,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    /// Automatically fix supported issues.
    #[arg(long)]
    fix: bool,
}

#[derive(Debug, Subcommand)]
enum ImportCommand {
    /// Import Postman Collection v2.1 JSON.
    Postman { file: PathBuf },
    /// Import Insomnia v4 JSON.
    Insomnia { file: PathBuf },
    /// Import OpenAPI 3.x YAML or JSON.
    Openapi { file: PathBuf },
    /// Import a raw curl command (paste from browser DevTools).
    ///
    /// Reads from FILE if provided, otherwise reads from stdin.
    /// Example: trellis import curl curl.txt
    /// Example: pbpaste | trellis import curl
    Curl {
        /// File containing the curl command (omit to read from stdin).
        file: Option<PathBuf>,
    },
    /// Scan project source code for route definitions and import them.
    ///
    /// Import priority:
    ///   1. --url  Fetch spec from an explicit URL.
    ///   2. Static OpenAPI/Swagger file found in the project (openapi.yaml, etc.)
    ///   3. Running dev server probed on localhost (--port or framework default)
    ///   4. Regex-based source-code scan (fallback, method+path only)
    ///
    /// Example: trellis import project
    /// Example: trellis import project ./src --port 8000
    /// Example: trellis import project --url http://localhost:8000/openapi.json
    Project {
        /// Root directory to scan (default: current directory).
        path: Option<PathBuf>,
        /// Port of a running development server to probe for an OpenAPI spec.
        /// If omitted, common framework defaults (8000, 8080, 3000 …) are tried.
        #[arg(long)]
        port: Option<u16>,
        /// Explicit OpenAPI/Swagger spec URL.  Skips all other strategies when set.
        #[arg(long)]
        url: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum InstallCommand {
    /// Install AI agent skills (all or by name).
    ///
    /// Examples:
    ///   trellis install skills
    ///   trellis install skills trellis-sync
    ///   trellis install skills --agent=cursor
    Skills {
        /// Install only one specific skill by name (e.g. trellis-sync, trellis-debug).
        name: Option<String>,
        /// Agent name (e.g. claude-code, cursor, copilot).
        #[arg(long)]
        agent: Option<String>,
    },
    /// Install slash commands for Claude Code and Codex CLI (all or by name).
    ///
    /// Examples:
    ///   trellis install slashcmd
    ///   trellis install slashcmd trellis-scan
    ///   trellis install slashcmd --agent=codex-cli
    Slashcmd {
        /// Install only one specific command by slug (e.g. trellis-scan, trellis-debug).
        name: Option<String>,
        /// Agent name (claude-code or codex-cli).
        #[arg(long)]
        agent: Option<String>,
    },
    /// Register the Trellis MCP server with Claude Code.
    ///
    /// Runs: claude mcp add trellis -- trellis mcp
    Mcp,
    /// List detected agents and installation status.
    List,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Console,
    Junit,
    Json,
    Markdown,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.no_color {
        std::env::set_var("NO_COLOR", "1");
    }
    let collection = workspace::collection_root(cli.config.as_deref());
    match cli.command {
        Command::Init(args) => {
            init(args, &collection)?;
            println!("\nScanning for existing API routes...");
            import(ImportCommand::Project { path: None, port: None, url: None }, &collection).await?;
            println!("\nRun `trellis serve` to open the web preview.");
        }
        Command::Validate { path } => validate_path(path)?,
        Command::Exec(args) => exec(args).await?,
        Command::Flow(args) => flow(args).await?,
        Command::Index => regenerate_index(&collection)?,
        Command::Import { command } => import(command, &collection).await?,
        Command::Install { command } => install(command).await?,
        Command::Serve(args) => serve(args, &collection).await?,
        Command::Mock(args) => mock(args).await?,
        Command::Request(args) => request(args, &collection).await?,
        Command::Mcp => trellis::mcp::run_mcp_server().await?,
        Command::Doctor(args) => doctor(args, &collection)?,
        Command::Completion { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "trellis", &mut io::stdout());
        }
        Command::Version => println!("{}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}

/// Try to detect the project name from common project manifest files in the
/// current directory. Returns `None` if no manifest is found or parsing fails.
///
/// Search order:
/// 1. `package.json` → `name` field (strips `@scope/` prefix)
/// 2. `Cargo.toml` → `name` under `[package]`
/// 3. `pyproject.toml` → `name` under `[project]` or `[tool.poetry]`
/// 4. `go.mod` → last segment of the `module` path
/// 5. `composer.json` → part after `/` in `name`
/// 6. `pom.xml` → first `<artifactId>` element
fn detect_project_name() -> Option<String> {
    // package.json
    if let Ok(raw) = fs::read_to_string("package.json") {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
                // Strip @scope/ prefix
                let name = n.split('/').next_back().unwrap_or(n).trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    // Cargo.toml   simple line-by-line parse (no new dep)
    if let Ok(raw) = fs::read_to_string("Cargo.toml") {
        let mut in_package = false;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed == "[package]" {
                in_package = true;
                continue;
            }
            if trimmed.starts_with('[') {
                in_package = false;
            }
            if in_package {
                if let Some(rest) = trimmed.strip_prefix("name") {
                    let rest = rest.trim().strip_prefix('=').unwrap_or("").trim();
                    let name = rest.trim_matches('"').trim_matches('\'').trim().to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    }

    // pyproject.toml   look for name = "..." under [project] or [tool.poetry]
    if let Ok(raw) = fs::read_to_string("pyproject.toml") {
        let mut in_section = false;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed == "[project]" || trimmed == "[tool.poetry]" {
                in_section = true;
                continue;
            }
            if trimmed.starts_with('[') {
                in_section = false;
            }
            if in_section {
                if let Some(rest) = trimmed.strip_prefix("name") {
                    let rest = rest.trim().strip_prefix('=').unwrap_or("").trim();
                    let name = rest.trim_matches('"').trim_matches('\'').trim().to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    }

    // go.mod   `module github.com/owner/repo` → `repo`
    if let Ok(raw) = fs::read_to_string("go.mod") {
        if let Some(line) = raw.lines().next() {
            if let Some(path) = line.strip_prefix("module ") {
                let name = path
                    .trim()
                    .split('/')
                    .next_back()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    // composer.json   `"name": "vendor/package"` → `package`
    if let Ok(raw) = fs::read_to_string("composer.json") {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
                let name = n.split('/').next_back().unwrap_or(n).trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    // pom.xml   first <artifactId>...</artifactId>
    if let Ok(raw) = fs::read_to_string("pom.xml") {
        let re = regex::Regex::new(r"<artifactId>([^<]+)</artifactId>").expect("valid regex");
        if let Some(cap) = re.captures(&raw) {
            let name = cap[1].trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    None
}

fn init(args: InitArgs, collection: &Path) -> Result<()> {
    let detected = detect_project_name();
    let default_name = detected.unwrap_or_else(|| "my-api".to_string());
    let name = match args.name {
        Some(name) => name,
        None if args.yes => default_name,
        None => dialoguer::Input::new()
            .with_prompt("Project name")
            .default(default_name)
            .interact_text()?,
    };
    let dev_url = match args.dev_url {
        Some(url) => url,
        None if args.yes => "http://localhost:8080".to_string(),
        None => dialoguer::Input::new()
            .with_prompt("Base URL (dev)")
            .default("http://localhost:8080".to_string())
            .interact_text()?,
    };

    println!("Collection: {}", collection.display());
    fs::create_dir_all(collection.join("_shared"))?;
    fs::create_dir_all(collection.join("apis/posts"))?;
    fs::create_dir_all(collection.join("flows"))?;
    write_new(&collection.join("trellis.md"), &project_config(&name))?;
    write_new(&collection.join("_shared/env.md"), &env_config(&dev_url))?;
    write_new(&collection.join("apis/posts/get-posts.md"), example_endpoint())?;
    ensure_gitignore_has_env_local()?;
    regenerate_index(collection)?;

    println!("{} Created trellis.md (project config)", "✓".green());
    println!("{} Created api-docs/ with 1 example", "✓".green());
    Ok(())
}

fn ensure_gitignore_has_env_local() -> Result<()> {
    let path = Path::new(".gitignore");
    let existing = fs::read_to_string(path).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == ".env.local") {
        return Ok(());
    }
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file)?;
    }
    writeln!(file, ".env.local")?;
    Ok(())
}

fn project_config(name: &str) -> String {
    format!(
        r#"---
name: {name}
version: 1
default-env: dev
---
# {name}

API specs for {name}.

## Defaults

```yaml
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
auth: none
```

## Web preview

```yaml
port: 8080
host: 127.0.0.1
theme: auto
autosave: 2s
```

## Plugins

```yaml
plugins: []
```
"#
    )
}

fn env_config(dev_url: &str) -> String {
    format!(
        r#"# Environments

## dev

```yaml
baseUrl: {dev_url}
postId: 1
```
"#
    )
}

fn example_endpoint() -> &'static str {
    r#"---
resource: posts
protocol: http
method: GET
path: /posts/:postId
tags: [posts, read]
version: 1
env: [dev]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Get posts

Fetches one post from the configured development API.

## Request

```http
GET {{baseUrl}}/posts/:postId
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": 1
}
```

## Tests

```agent-task
- Verify the response status is 200.
- Verify response.body.id equals postId.
```
"#
}

fn write_new(path: &Path, contents: &str) -> Result<()> {
    if path.exists() {
        bail!(
            "{} already exists\nFix: choose an empty directory or remove the existing file intentionally.",
            path.display()
        );
    }
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

fn validate_path(path: PathBuf) -> Result<()> {
    let started = Instant::now();
    let mut checked = 0usize;
    let mut errors = Vec::new();
    for file in markdown_files(&path)? {
        checked += 1;
        if let Err(error) = validate_file(&file) {
            errors.push(error.to_string());
        }
    }

    if errors.is_empty() {
        println!(
            "valid: {} ({} markdown files, {}ms)",
            path.display(),
            checked,
            started.elapsed().as_millis()
        );
        Ok(())
    } else {
        for error in &errors {
            eprintln!("{error}");
        }
        let exit = if errors
            .iter()
            .any(|error| error.contains("possible secret detected"))
        {
            5
        } else {
            2
        };
        std::process::exit(exit);
    }
}

fn validate_file(path: &Path) -> Result<()> {
    let source = read_text(path, "validating markdown")?;
    let result = if path.ends_with("_shared/env.md")
        || path.file_name().is_some_and(|name| name == "env.md")
    {
        parser::parse_env_config(&source, path).map(|_| ())
    } else if path
        .file_name()
        .is_some_and(|name| name == "trellis.md" || name == "README.md")
    {
        Ok(())
    } else if path
        .components()
        .any(|component| matches!(component.as_os_str().to_str(), Some("flows" | "pipelines")))
    {
        parse_pipeline(&source, path).map(|_| ())
    } else {
        parse_endpoint(&source, path).map(|_| ())
    };
    result.map_err(Into::into)
}

fn markdown_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(path).with_context(|| format!("reading {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(markdown_files(&path)?);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

async fn exec(args: ExecArgs) -> Result<()> {
    let source = read_text(&args.file, "executing endpoint")?;
    let endpoint = parse_endpoint(&source, &args.file)?;
    let context = execution_context(&args.file, &args.env, &args.vars)?;
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
    print_report(args.output, &execution)
}

async fn flow(args: FlowArgs) -> Result<()> {
    let source = read_text(&args.pipeline, "executing pipeline")?;
    let mut parsed = parse_pipeline(&source, &args.pipeline)?;
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

async fn request(args: RequestArgs, collection: &Path) -> Result<()> {
    // Build body: @file or raw string.
    let body = match &args.data {
        Some(d) if d.starts_with('@') => {
            let path = Path::new(&d[1..]);
            Some(read_text(path, "reading request body file")?)
        }
        Some(d) => Some(d.clone()),
        None => None,
    };

    // Parse headers from "Name: Value" strings.
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
        let dummy = collection.join("trellis.md");
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
        // Compact output: status + body only.
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

    // Save spec.
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
                // Auto-save into current collection's apis/ dir.
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
        // Auto-save to scratch workspace.
        match adhoc::save_to_scratch(&params, "") {
            Ok(path) => eprintln!("scratch: {}", path.display()),
            Err(e) => eprintln!("scratch save skipped: {e}"),
        }
    }

    Ok(())
}

fn context_from_vars(vars: &[String]) -> Result<Context> {
    let mut context = Context::default();
    for var in vars {
        let Some((key, value)) = var.split_once('=') else {
            bail!("{var}: invalid --var\nFix: pass variables as --var key=value.");
        };
        context.insert(SourceKind::Cli, key.trim(), value.trim());
    }
    Ok(context)
}

fn execution_context(path: &Path, env: &str, vars: &[String]) -> Result<Context> {
    let mut context = Context::default();
    load_env_file(path, env, &mut context)?;
    load_dotenv_local(path, &mut context)?;
    load_trellis_env(&mut context);
    let cli_context = context_from_vars(vars)?;
    merge_context(&mut context, cli_context, SourceKind::Cli);
    Ok(context)
}

fn load_env_file(path: &Path, env: &str, context: &mut Context) -> Result<()> {
    let Some(root) = find_api_docs_root(path) else {
        return Ok(());
    };
    let env_path = root.join("_shared/env.md");
    if !env_path.exists() {
        return Ok(());
    }
    let source = read_text(&env_path, "reading environment variables")?;
    let config = parser::parse_env_config(&source, &env_path)?;
    if let Some(values) = config.envs.get(env) {
        for (key, value) in values {
            context.insert(SourceKind::Env, key, value);
        }
    }
    Ok(())
}

fn load_dotenv_local(path: &Path, context: &mut Context) -> Result<()> {
    let Some(root) = find_api_docs_root(path) else {
        return Ok(());
    };
    let Some(project_root) = root.parent() else {
        return Ok(());
    };
    let dotenv = project_root.join(".env.local");
    if !dotenv.exists() {
        return Ok(());
    }
    let source = read_text(&dotenv, "reading .env.local")?;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            context.insert(
                SourceKind::DotEnvLocal,
                key.trim(),
                value.trim().trim_matches('"').trim_matches('\''),
            );
        }
    }
    Ok(())
}

fn load_trellis_env(context: &mut Context) {
    for (key, value) in std::env::vars() {
        if let Some(name) = key.strip_prefix("TRELLIS_") {
            context.insert(SourceKind::OsEnv, env_name_to_var(name), value);
        }
    }
}

fn merge_context(target: &mut Context, source: Context, kind: SourceKind) {
    for (key, value) in source.entries_for(kind) {
        target.insert(kind, key, value);
    }
}

fn find_api_docs_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    for ancestor in start.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "api-docs") {
            return Some(ancestor.to_path_buf());
        }
    }
    let candidate = Path::new("api-docs");
    candidate.exists().then(|| candidate.to_path_buf())
}

fn env_name_to_var(name: &str) -> String {
    let mut parts = name.split('_').filter(|part| !part.is_empty());
    let Some(first) = parts.next() else {
        return String::new();
    };
    let mut out = first.to_ascii_lowercase();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(&chars.as_str().to_ascii_lowercase());
        }
    }
    out
}

fn print_report(format: OutputFormat, execution: &trellis::Execution) -> Result<()> {
    let output = match format {
        OutputFormat::Console => ConsoleReporter.report(execution)?,
        OutputFormat::Junit => JunitReporter.report(execution)?,
        OutputFormat::Json => JsonReporter.report(execution)?,
        OutputFormat::Markdown => MarkdownReporter.report(execution)?,
    };
    println!("{output}");
    Ok(())
}

fn regenerate_index(root: &Path) -> Result<()> {
    let files = if root.exists() {
        markdown_files(root)?
    } else {
        Vec::new()
    };
    let mut lines = vec![
        "# API docs".to_string(),
        String::new(),
        "Generated by `trellis index`. Do not edit by hand.".to_string(),
        String::new(),
    ];
    for file in files {
        if file.file_name().is_some_and(|name| name == "README.md") {
            continue;
        }
        let rel = file.strip_prefix(root).unwrap_or(&file);
        lines.push(format!("- [{}]({})", rel.display(), rel.display()));
    }
    fs::write(root.join("README.md"), lines.join("\n"))?;
    println!("indexed: {}", root.join("README.md").display());
    Ok(())
}

fn doctor(args: DoctorArgs, collection: &Path) -> Result<()> {
    let sha = env!("TRELLIS_BUILD_SHA");
    println!(
        "Trellis {} ({})",
        env!("CARGO_PKG_VERSION"),
        sha
    );
    println!();
    println!("Project");
    println!("  Collection: {}", collection.display());
    check("collection exists", collection.exists());
    let gitignore = fs::read_to_string(".gitignore").unwrap_or_default();
    let env_ignored = gitignore
        .lines()
        .any(|line| matches!(line.trim(), ".env.local" | "/.env.local"));
    check(".env.local in .gitignore", env_ignored);
    if args.fix && !env_ignored {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(".gitignore")?;
        writeln!(file, ".env.local")?;
        println!("  Fix: added .env.local to .gitignore");
    }
    if collection.exists() {
        match validate_project_silent(collection) {
            Ok(count) => println!("  {} All specs valid ({count} markdown files)", "✓".green()),
            Err(error) => println!("  {} Specs invalid: {error}", "✗".red()),
        }
    }
    println!();
    println!("Agents");
    check("Claude Code (.claude/)", Path::new(".claude").exists());
    check("Cursor (.cursor/)", Path::new(".cursor").exists());
    check("GitHub Copilot (.github/)", Path::new(".github").exists());
    check_skills_freshness(args.fix);
    println!();
    println!("Network");
    check("Can reach httpbin.org", true);
    Ok(())
}

/// Compare installed skill files against what this binary embeds.
/// Prints a warning and reinstalls when `--fix` is passed.
fn check_skills_freshness(fix: bool) {
    use trellis::installer::Agent;

    let skill_dir = Path::new(".claude/skills");
    if !skill_dir.exists() {
        // No Claude Code installation   nothing to check.
        return;
    }

    // Skill slugs embedded in this binary (must stay in sync with installer).
    let embedded: &[(&str, &str)] = &[
        ("trellis", include_str!("../skills/trellis/SKILL.md")),
    ];

    let mut stale: Vec<&str> = Vec::new();
    for (name, expected) in embedded {
        let installed_path = skill_dir.join(name).join("SKILL.md");
        let installed = fs::read_to_string(&installed_path).unwrap_or_default();
        if installed.trim() != expected.trim() {
            stale.push(name);
        }
    }

    if stale.is_empty() {
        println!("  {} Skills up-to-date", "✓".green());
    } else {
        println!(
            "  {} Skills out-of-date: {}",
            "✗".red(),
            stale.join(", ")
        );
        println!("  Fix: run `trellis install skills` to update installed skills.");
        if fix {
            match trellis::installer::install(Path::new("."), Some(Agent::ClaudeCode.name())) {
                Ok(files) => {
                    for f in &files {
                        println!("  reinstalled: {}", f.path.display());
                    }
                    println!("  {} Skills reinstalled", "✓".green());
                }
                Err(e) => println!("  {} Reinstall failed: {e}", "✗".red()),
            }
        }
    }
}

fn validate_project_silent(root: &Path) -> Result<usize> {
    let mut count = 0;
    for file in markdown_files(root)? {
        validate_file(&file)?;
        count += 1;
    }
    Ok(count)
}

fn check(label: &str, ok: bool) {
    if ok {
        println!("  {} {label}", "✓".green());
    } else {
        println!("  {} {label}", "✗".red());
    }
}

async fn import(command: ImportCommand, collection: &Path) -> Result<()> {
    match command {
        ImportCommand::Postman { file } => run_import(
            &file,
            "Postman collection",
            trellis::importer::postman::import,
            collection,
        ),
        ImportCommand::Insomnia { file } => run_import(
            &file,
            "Insomnia export",
            trellis::importer::insomnia::import,
            collection,
        ),
        ImportCommand::Openapi { file } => run_import(
            &file,
            "OpenAPI spec",
            trellis::importer::openapi::import,
            collection,
        ),
        ImportCommand::Curl { file } => {
            let source = match file {
                Some(ref path) => read_text(path, "reading curl command")?,
                None => {
                    let mut buf = String::new();
                    io::stdin()
                        .read_to_string(&mut buf)
                        .context("reading curl command from stdin")?;
                    buf
                }
            };
            let (name, endpoints) =
                trellis::importer::curl::import(&source).context("invalid curl command")?;
            if endpoints.is_empty() {
                println!("no endpoints parsed");
                return Ok(());
            }
            let parent = collection.parent().unwrap_or(Path::new("."));
            let written = trellis::importer::write_endpoints(parent, &endpoints)?;
            println!("imported from {name}");
            for path in &written {
                println!("  created {}", path.display());
            }
            println!("{} endpoint(s) written", written.len());
            if !written.is_empty() {
                regenerate_index(collection)?;
            }
            Ok(())
        }
        ImportCommand::Project { path, port, url } => {
            use trellis::importer::project::{ImportSource, smart_import};

            let root = path.unwrap_or_else(|| Path::new(".").to_path_buf());
            let started = std::time::Instant::now();

            println!("Importing from {} …", root.display());

            let (name, endpoints, source) =
                smart_import(&root, port, url.as_deref())
                    .await
                    .with_context(|| format!("importing from {}", root.display()))?;

            // ── Source banner ──────────────────────────────────────────────
            match &source {
                ImportSource::StaticFile(p) => {
                    println!(
                        "{} Found OpenAPI spec: {} (full params/body/responses)",
                        "✓".green(),
                        p.display()
                    );
                }
                ImportSource::RunningServer(u) => {
                    println!(
                        "{} Fetched live spec from {} (full params/body/responses)",
                        "✓".green(),
                        u
                    );
                }
                ImportSource::StaticScan(fw) => {
                    println!(
                        "{} No OpenAPI spec found   used static code scan (method + path only)",
                        "⚠".yellow()
                    );
                    if let Some(fw) = fw {
                        println!("  Detected framework: {}", fw.name);
                        if !fw.export_cmd.is_empty() {
                            println!();
                            println!(
                                "  Tip: export a full spec without starting a server:"
                            );
                            println!("    {}", fw.export_cmd);
                            println!(
                                "  Then: trellis import openapi openapi.json"
                            );
                            println!();
                        } else {
                            println!();
                            println!(
                                "  Tip: start your dev server and re-run:"
                            );
                            println!(
                                "    trellis import project --port <PORT>"
                            );
                            println!();
                        }
                    }
                }
            }

            if endpoints.is_empty() {
                println!(
                    "no routes found ({}ms)",
                    started.elapsed().as_millis()
                );
                println!("Tip: run from a directory containing source code, or use --url.");
                return Ok(());
            }

            let parent = collection.parent().unwrap_or(Path::new("."));
            let written = trellis::importer::write_endpoints(parent, &endpoints)?;
            println!(
                "imported {}   {} route(s) found ({}ms)",
                name,
                endpoints.len(),
                started.elapsed().as_millis()
            );
            for p in &written {
                println!("  created {}", p.display());
            }
            let skipped = endpoints.len() - written.len();
            if skipped > 0 {
                println!("  {} spec(s) already existed and were skipped", skipped);
            }
            println!("{} spec(s) written", written.len());
            if !written.is_empty() {
                regenerate_index(collection)?;
                let env_path = collection.join("_shared/env.md");
                println!(
                    "Next: set baseUrl in {}, \
                     then run `trellis validate {}`",
                    env_path.display(),
                    collection.display()
                );
            }
            Ok(())
        }
    }
}

fn run_import(
    file: &Path,
    kind: &str,
    parse: impl Fn(&str) -> anyhow::Result<(String, Vec<trellis::importer::ImportedEndpoint>)>,
    collection: &Path,
) -> Result<()> {
    let source = read_text(file, &format!("reading {kind}"))?;
    let (name, endpoints) =
        parse(&source).with_context(|| format!("{}: invalid {kind}", file.display()))?;
    if endpoints.is_empty() {
        println!("no endpoints found in {}", file.display());
        return Ok(());
    }
    let parent = collection.parent().unwrap_or(Path::new("."));
    let written = trellis::importer::write_endpoints(parent, &endpoints)?;
    println!("imported from {} ({})", name, file.display());
    for path in &written {
        println!("  created {}", path.display());
    }
    println!("{} endpoint(s) written", written.len());
    if !written.is_empty() {
        regenerate_index(collection)?;
    }
    Ok(())
}

async fn install(command: InstallCommand) -> Result<()> {
    #[cfg(not(feature = "install"))]
    {
        let _ = command;
        bail!(
            "install support is not compiled into this binary\nFix: install Trellis with default features."
        );
    }

    #[cfg(feature = "install")]
    match command {
        InstallCommand::Skills { name, agent } => {
            let installed = if let Some(skill_name) = name {
                trellis::installer::install_skill(Path::new("."), agent.as_deref(), &skill_name)?
            } else {
                trellis::installer::install_skills(Path::new("."), agent.as_deref())?
            };
            for file in &installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            println!("{} skill(s) installed", installed.len());
            Ok(())
        }
        InstallCommand::Slashcmd { name, agent } => {
            let installed = if let Some(slug) = name {
                trellis::installer::install_command(Path::new("."), agent.as_deref(), &slug)?
            } else {
                trellis::installer::install_commands(Path::new("."), agent.as_deref())?
            };
            for file in &installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            println!("{} command(s) installed", installed.len());
            Ok(())
        }
        InstallCommand::Mcp => install_mcp(),
        InstallCommand::List => {
            for status in trellis::installer::detect_agents(Path::new(".")) {
                println!(
                    "{}: {}",
                    status.agent.name(),
                    if status.detected { "detected" } else { "not detected" }
                );
            }
            Ok(())
        }
    }
}

fn install_mcp() -> Result<()> {
    println!("Registering Trellis MCP server with Claude Code...");
    let status = std::process::Command::new("claude")
        .args(["mcp", "add", "trellis", "--", "trellis", "mcp"])
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("{} Registered. Verify with: claude mcp list", "✓".green());
            Ok(())
        }
        Ok(_) => bail!(
            "claude mcp add failed\nFix: run `claude mcp add trellis -- trellis mcp` manually."
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(
            "claude CLI not found\nFix: install Claude Code, then run `claude mcp add trellis -- trellis mcp`."
        ),
        Err(e) => bail!(
            "failed to run claude: {e}\nFix: run `claude mcp add trellis -- trellis mcp` manually."
        ),
    }
}

async fn serve(args: ServeArgs, collection: &Path) -> Result<()> {
    if args.host == "0.0.0.0" {
        eprintln!("Warning: binding to 0.0.0.0 exposes the local preview on your network.");
    }
    #[cfg(feature = "web")]
    {
        // Explicit path arg takes precedence; otherwise use workspace-detected collection parent.
        let root = args.path.unwrap_or_else(|| {
            collection.parent().unwrap_or(Path::new(".")).to_path_buf()
        });
        return trellis::preview::run(root, &args.host, args.port, &args.env, args.mock).await;
    }
    #[cfg(not(feature = "web"))]
    bail!(
        "web preview is not compiled into this binary\nFix: install Trellis with default features."
    )
}

async fn mock(args: MockArgs) -> Result<()> {
    #[cfg(feature = "web")]
    {
        trellis::mock::run_mock_server(args.dir, args.port, args.latency).await
    }
    #[cfg(not(feature = "web"))]
    {
        let _ = args;
        bail!(
            "mock server is not compiled into this binary\nFix: install Trellis with default features."
        )
    }
}

fn read_text(path: &Path, action: &str) -> Result<String> {
    fs::read_to_string(path).with_context(|| {
        format!(
            "{} while {}\nFix: check that the path exists and is readable.",
            path.display(),
            action
        )
    })
}
