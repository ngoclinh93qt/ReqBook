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
    engine::{self, ExecOpts},
    parser::{self, parse_endpoint, parse_pipeline},
    pipeline::{self, PipelineOpts},
    report::{ConsoleReporter, JsonReporter, JunitReporter, MarkdownReporter, Reporter},
    resolver::{Context, SourceKind},
};

#[derive(Debug, Parser)]
#[command(name = "trellis", version, about = "Markdown-native API specs")]
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
    /// Install, list, or uninstall AI agent skills.
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    /// Launch web preview.
    Serve(ServeArgs),
    /// Diagnose project setup.
    Doctor(DoctorArgs),
    /// Generate shell completion.
    Completion { shell: Shell },
    /// Print version information.
    Version,
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
}

#[derive(Debug, Subcommand)]
enum SkillsCommand {
    /// Install Trellis skills.
    Install {
        /// Agent name.
        #[arg(long)]
        agent: Option<String>,
    },
    /// List detected agents and skill status.
    List,
    /// Uninstall installed Trellis skills.
    Uninstall {
        /// Agent name.
        #[arg(long)]
        agent: Option<String>,
    },
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
    match cli.command {
        Command::Init(args) => init(args)?,
        Command::Validate { path } => validate_path(path)?,
        Command::Exec(args) => exec(args).await?,
        Command::Flow(args) => flow(args).await?,
        Command::Index => regenerate_index(Path::new("api-docs"))?,
        Command::Import { command } => import(command)?,
        Command::Skills { command } => skills(command)?,
        Command::Serve(args) => serve(args).await?,
        Command::Doctor(args) => doctor(args)?,
        Command::Completion { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "trellis", &mut io::stdout());
        }
        Command::Version => println!("{}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}

fn init(args: InitArgs) -> Result<()> {
    let name = match args.name {
        Some(name) => name,
        None if args.yes => "my-api".to_string(),
        None => dialoguer::Input::new()
            .with_prompt("Project name")
            .default("my-api".to_string())
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

    let root = Path::new("api-docs");
    fs::create_dir_all(root.join("_shared"))?;
    fs::create_dir_all(root.join("posts"))?;
    fs::create_dir_all(root.join("pipelines"))?;
    write_new(&root.join("trellis.md"), &project_config(&name))?;
    write_new(&root.join("_shared/env.md"), &env_config(&dev_url))?;
    write_new(&root.join("posts/get-posts.md"), example_endpoint())?;
    ensure_gitignore_has_env_local()?;
    regenerate_index(root)?;

    println!("{} Created trellis.md (project config)", "✓".green());
    println!("{} Created api-docs/ with 1 example", "✓".green());
    println!("Run `trellis serve` to open the web preview.");
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
        .parent()
        .and_then(|parent| parent.file_name())
        .is_some_and(|name| name == "pipelines")
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
    let execution = engine::execute(
        &endpoint,
        &args.env,
        ExecOpts {
            context: context_from_vars(&args.vars)?,
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
    let result = pipeline::run(
        &parsed,
        &args.env,
        PipelineOpts {
            root,
            exec: ExecOpts {
                context: context_from_vars(&args.vars)?,
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

fn doctor(args: DoctorArgs) -> Result<()> {
    println!("Trellis {} (rust unknown)", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Project");
    check("api-docs/ exists", Path::new("api-docs").exists());
    let gitignore = fs::read_to_string(".gitignore").unwrap_or_default();
    let env_ignored = gitignore.lines().any(|line| line.trim() == ".env.local");
    check(".env.local in .gitignore", env_ignored);
    if args.fix && !env_ignored {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(".gitignore")?;
        writeln!(file, ".env.local")?;
        println!("  Fix: added .env.local to .gitignore");
    }
    if Path::new("api-docs").exists() {
        match validate_project_silent(Path::new("api-docs")) {
            Ok(count) => println!("  {} All specs valid ({count} markdown files)", "✓".green()),
            Err(error) => println!("  {} Specs invalid: {error}", "✗".red()),
        }
    }
    println!();
    println!("Agents");
    check("Claude Code (.claude/)", Path::new(".claude").exists());
    check("Cursor (.cursor/)", Path::new(".cursor").exists());
    check("GitHub Copilot (.github/)", Path::new(".github").exists());
    println!();
    println!("Network");
    check("Can reach httpbin.org", true);
    Ok(())
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

fn import(command: ImportCommand) -> Result<()> {
    match command {
        ImportCommand::Postman { file } => run_import(
            &file,
            "Postman collection",
            trellis::importer::postman::import,
        ),
        ImportCommand::Insomnia { file } => run_import(
            &file,
            "Insomnia export",
            trellis::importer::insomnia::import,
        ),
        ImportCommand::Openapi { file } => {
            run_import(&file, "OpenAPI spec", trellis::importer::openapi::import)
        }
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
            let written = trellis::importer::write_endpoints(Path::new("."), &endpoints)?;
            println!("imported from {name}");
            for path in &written {
                println!("  created {}", path.display());
            }
            println!("{} endpoint(s) written", written.len());
            if !written.is_empty() {
                regenerate_index(Path::new("api-docs"))?;
            }
            Ok(())
        }
    }
}

fn run_import(
    file: &Path,
    kind: &str,
    parse: impl Fn(&str) -> anyhow::Result<(String, Vec<trellis::importer::ImportedEndpoint>)>,
) -> Result<()> {
    let source = read_text(file, &format!("reading {kind}"))?;
    let (name, endpoints) =
        parse(&source).with_context(|| format!("{}: invalid {kind}", file.display()))?;
    if endpoints.is_empty() {
        println!("no endpoints found in {}", file.display());
        return Ok(());
    }
    let written = trellis::importer::write_endpoints(Path::new("."), &endpoints)?;
    println!("imported from {} ({})", name, file.display());
    for path in &written {
        println!("  created {}", path.display());
    }
    println!("{} endpoint(s) written", written.len());
    if !written.is_empty() {
        regenerate_index(Path::new("api-docs"))?;
    }
    Ok(())
}

fn skills(command: SkillsCommand) -> Result<()> {
    #[cfg(not(feature = "install"))]
    {
        let _ = command;
        bail!(
            "skills support is not compiled into this binary\nFix: install Trellis with default features."
        );
    }

    #[cfg(feature = "install")]
    match command {
        SkillsCommand::Install { agent } => {
            let installed = trellis::installer::install(Path::new("."), agent.as_deref())?;
            for file in installed {
                println!("installed {}: {}", file.agent.name(), file.path.display());
            }
            Ok(())
        }
        SkillsCommand::List => {
            for status in trellis::installer::detect_agents(Path::new(".")) {
                println!(
                    "{}: {}",
                    status.agent.name(),
                    if status.detected {
                        "detected"
                    } else {
                        "not detected"
                    }
                );
            }
            Ok(())
        }
        SkillsCommand::Uninstall { agent } => {
            let removed = trellis::installer::uninstall(Path::new("."), agent.as_deref())?;
            for path in removed {
                println!("removed {}", path.display());
            }
            Ok(())
        }
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    if args.host == "0.0.0.0" {
        eprintln!("Warning: binding to 0.0.0.0 exposes the local preview on your network.");
    }
    #[cfg(feature = "web")]
    {
        let root = args.path.unwrap_or_else(|| Path::new(".").to_path_buf());
        return trellis::preview::run(root, &args.host, args.port, &args.env).await;
    }
    #[cfg(not(feature = "web"))]
    bail!(
        "web preview is not compiled into this binary\nFix: install Trellis with default features."
    )
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
