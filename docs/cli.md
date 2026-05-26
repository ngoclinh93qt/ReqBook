# CLI reference

This page documents every `trellis` subcommand, its flags, and its behavior. For installation see [Getting started](getting-started.md). For project and environment configuration see [Configuration reference](configuration.md).

## Global flags

These flags are accepted by every subcommand.

| Flag | Type | Description |
| --- | --- | --- |
| `--config <path>` | path | Path to `api-docs/trellis.md`. Overrides the default discovery. |
| `--no-color` | bool | Disable ANSI color in output. Also respected if `NO_COLOR` is set in the environment. |
| `-v`, `--verbose` | bool | Enable verbose diagnostic output. |

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Passed. |
| 1 | Test failed. |
| 2 | Invalid spec. |
| 3 | Engine error. |
| 4 | Network error. |
| 5 | Secret detected. |

Exit code 5 is returned before any network request is made. See [Secret detection](configuration.md#secret-detection) for which patterns trigger it.

---

## trellis init

Scaffold a new `api-docs/` project in the current directory.

```
trellis init [--name=<name>] [--dev-url=<url>] [--yes]
```

| Flag | Type | Default | Description |
| --- | --- | --- | --- |
| `--name` | string | interactive | Project name written into `trellis.md`. |
| `--dev-url` | string | interactive | Base URL written into `_shared/env.md` for the `dev` environment. |
| `--yes` | bool | false | Accept all defaults without interactive prompts. |

Without `--yes`, `trellis init` prompts for any missing values. `--yes` uses `my-api` and `http://localhost:8080` as defaults.

`trellis init` will not overwrite existing files. If a file already exists, it exits with an error naming the conflicting file.

`trellis init` appends `.env.local` to `.gitignore` if it is not already present.

**Example**

```bash
trellis init --name=payments-api --dev-url=http://localhost:3000
```

**Exit codes**: 0 on success, 3 on filesystem error.

---

## trellis validate

Validate one endpoint file, one pipeline file, or all markdown files under a directory.

```
trellis validate <path>
```

| Argument | Description |
| --- | --- |
| `<path>` | File or directory to validate. |

Trellis classifies each file by location and name:

- Files named `env.md` are validated as environment config.
- Files under `pipelines/` are validated as pipeline files.
- `trellis.md` and `README.md` are checked for frontmatter only.
- All other `.md` files are validated as endpoint files.

Each error message includes the file path, line number when known, and a suggested fix.

**Example**

```bash
trellis validate api-docs/
trellis validate api-docs/users/get-user-by-id.md
```

**Exit codes**: 0 if all files are valid, 2 if any spec is invalid, 5 if a secret is detected.

---

## trellis exec

Execute one endpoint file and compare the actual response against the expected response.

```
trellis exec <file> [--env=<env>] [--output=<format>] [--var key=val]... [--dry-run] [--timeout=<ms>]
```

| Flag | Type | Default | Description |
| --- | --- | --- | --- |
| `<file>` | path | required | Path to an endpoint markdown file. |
| `--env` | string | `dev` | Environment name. Must match a heading in `_shared/env.md`. |
| `--output` | enum | `console` | Output format: `console`, `junit`, `json`, or `markdown`. |
| `--var` | string | — | Inject a variable as `key=value`. Repeatable. CLI variables override all other sources. |
| `--dry-run` | bool | false | Print the resolved request without sending it. Does not make a network connection. |
| `--timeout` | integer | — | Override request timeout in milliseconds. Takes precedence over endpoint and project defaults. |

When `--env=prod` is used in an interactive terminal, Trellis prompts for confirmation before sending. Pass `--yes` (global flag) to skip the prompt in CI.

If a referenced variable is not resolved from any source, Trellis exits with code 2 and prints a suggested fix naming the variable and where to define it.

**Examples**

```bash
# Basic execution
trellis exec api-docs/users/get-user-by-id.md

# With environment and variable override
trellis exec api-docs/users/get-user-by-id.md --env=staging --var userId=42

# CI: JUnit output for test reporters
trellis exec api-docs/users/get-user-by-id.md --output=junit > results.xml

# Dry run to inspect the resolved request
trellis exec api-docs/users/create-user.md --dry-run --var email=test@example.com

# Timeout override
trellis exec api-docs/users/get-user-by-id.md --timeout=10000
```

**Exit codes**: 0 if the response matches the expected response, 1 if any assertion fails, 2 if the spec is invalid, 3 on engine error, 4 on network error, 5 if a secret is detected.

---

## trellis flow

Execute a pipeline file. Steps run sequentially by default, with optional parallelism.

```
trellis flow <file> [--env=<env>] [--output=<format>] [--var key=val]... [--parallel] [--no-parallel] [--timeout=<ms>]
```

| Flag | Type | Default | Description |
| --- | --- | --- | --- |
| `<file>` | path | required | Path to a pipeline markdown file. |
| `--env` | string | `dev` | Environment name. |
| `--output` | enum | `console` | Output format: `console`, `junit`, `json`, or `markdown`. |
| `--var` | string | — | Inject a variable as `key=value`. Repeatable. |
| `--parallel` | bool | false | Force parallel execution, overriding the pipeline file's `parallel` setting. |
| `--no-parallel` | bool | false | Force sequential execution, overriding the pipeline file's `parallel` setting. |
| `--timeout` | integer | — | Override timeout in milliseconds for every step in the pipeline. |

`--parallel` and `--no-parallel` are mutually exclusive. Steps that depend on a captured value from a previous step always wait for that step even in parallel mode.

**Examples**

```bash
# Run a pipeline
trellis flow api-docs/pipelines/user-onboarding.md --env=staging

# Force sequential execution
trellis flow api-docs/pipelines/user-onboarding.md --no-parallel

# JSON output for programmatic consumption
trellis flow api-docs/pipelines/user-onboarding.md --output=json
```

**Exit codes**: 0 if all steps pass (or `continue-on-error` is true), 1 if any step fails, 2 if the pipeline spec is invalid, 3 on engine error, 4 on network error.

---

## trellis index

Regenerate `api-docs/README.md` from the current set of markdown files under `api-docs/`.

```
trellis index
```

No flags. Reads the current directory's `api-docs/` folder. The generated file contains a linked list of all markdown files. Do not edit `api-docs/README.md` by hand — it will be overwritten on the next `trellis index` run.

`trellis index` is run automatically by `trellis init` and by the import commands.

**Example**

```bash
trellis index
```

**Exit codes**: 0 on success, 3 on filesystem error.

---

## trellis import

Convert an existing API spec file into Trellis markdown endpoint files.

### trellis import postman

Import a Postman Collection v2.1 JSON export.

```
trellis import postman <file>
```

| Argument | Description |
| --- | --- |
| `<file>` | Path to a Postman Collection v2.1 JSON file. |

**Example**

```bash
trellis import postman my-collection.json
```

### trellis import insomnia

Import an Insomnia v4 JSON export.

```
trellis import insomnia <file>
```

| Argument | Description |
| --- | --- |
| `<file>` | Path to an Insomnia v4 JSON export file. |

**Example**

```bash
trellis import insomnia insomnia_export.json
```

### trellis import openapi

Import an OpenAPI 3.x spec in YAML or JSON format.

```
trellis import openapi <file>
```

| Argument | Description |
| --- | --- |
| `<file>` | Path to an OpenAPI 3.x YAML or JSON file. |

**Example**

```bash
trellis import openapi openapi.yaml
trellis import openapi openapi.json
```

### Import behavior

All three import commands write endpoint markdown files under `api-docs/` and then run `trellis index` to regenerate `api-docs/README.md`. Pre-request scripts, dynamic variables, and complex assertions are imported as `agent-task` blocks for manual review rather than as executable logic.

After importing, run `trellis validate api-docs/` and move any secrets to `.env.local`. See the [Migration guide](guides/migration.md) for per-tool details and post-import checklists.

**Exit codes**: 0 on success, 2 if the input file is not a valid spec for that tool, 3 on filesystem error.

---

## trellis skills

Install, list, or remove Trellis skill files for AI coding agents.

### trellis skills install

Install Trellis skill files into detected AI agent config directories.

```
trellis skills install [--agent=<name>]
```

| Flag | Type | Description |
| --- | --- | --- |
| `--agent` | string | Install skills only for a specific agent (e.g. `claude`, `cursor`, `copilot`). Installs for all detected agents if omitted. |

Skill files teach agents how to author, validate, and execute Trellis specs. Trellis detects agents by checking for their config directories (`.claude/`, `.cursor/`, `.github/`).

**Example**

```bash
# Install for all detected agents
trellis skills install

# Install only for Claude Code
trellis skills install --agent=claude
```

### trellis skills list

List detected AI agents and whether Trellis skills are installed for each.

```
trellis skills list
```

No flags.

**Example**

```bash
trellis skills list
```

Example output:

```text
claude: detected
cursor: not detected
copilot: not detected
```

### trellis skills uninstall

Remove installed Trellis skill files.

```
trellis skills uninstall [--agent=<name>]
```

| Flag | Type | Description |
| --- | --- | --- |
| `--agent` | string | Uninstall skills only for a specific agent. Uninstalls for all agents if omitted. |

**Example**

```bash
trellis skills uninstall --agent=claude
```

**Exit codes (all skills subcommands)**: 0 on success, 3 on filesystem error. Skills commands require the binary to be built with the `install` feature; the default distribution includes it.

---

## trellis serve

Start the local web preview server.

```
trellis serve [<path>] [--port=8080] [--host=127.0.0.1] [--env=<env>]
```

| Flag / Argument | Type | Default | Description |
| --- | --- | --- | --- |
| `<path>` | path | `.` (current directory) | Root directory of the Trellis project. |
| `--port` | integer | `8080` | TCP port to listen on. |
| `--host` | string | `127.0.0.1` | Host to bind to. Use `0.0.0.0` to expose on the local network (a warning is printed). |
| `--env` | string | `dev` | Environment used when executing endpoints from the preview UI. |

The web preview reads the same markdown files as the CLI. No build step is required. The server watches for file changes and refreshes connected browsers automatically.

**Example**

```bash
trellis serve
trellis serve --port=9000 --env=staging
trellis serve /path/to/other-project
```

**Exit codes**: 0 on clean shutdown (Ctrl-C), 3 on startup error. The web preview requires the binary to be built with the `web` feature; the default distribution includes it.

---

## trellis doctor

Check the project environment for common setup problems.

```
trellis doctor [--fix]
```

| Flag | Type | Description |
| --- | --- | --- |
| `--fix` | bool | Automatically apply safe fixes (e.g. adding `.env.local` to `.gitignore`). |

`trellis doctor` checks:

- Whether `api-docs/` exists.
- Whether `.env.local` is listed in `.gitignore`.
- Whether all specs under `api-docs/` are valid.
- Which AI agent config directories are present.
- Whether an outbound network connection can be made.

Use `trellis doctor` as the first debugging step when `trellis exec` or `trellis validate` behaves unexpectedly. See [Getting started — diagnose issues](getting-started.md#diagnose-issues) for example output.

**Example**

```bash
trellis doctor
trellis doctor --fix
```

**Exit codes**: 0 if all checks pass, 1 if any check fails.

---

## trellis completion

Print shell completion script to stdout.

```
trellis completion <shell>
```

| Argument | Description |
| --- | --- |
| `<shell>` | One of `bash`, `zsh`, `fish`, `elvish`, or `powershell`. |

**Examples**

```bash
# Bash
trellis completion bash >> ~/.bash_completion

# Zsh
trellis completion zsh > ~/.zfunc/_trellis
echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc
autoload -Uz compinit && compinit

# Fish
trellis completion fish > ~/.config/fish/completions/trellis.fish
```

**Exit codes**: 0 on success.

---

## trellis version

Print the installed version of Trellis and exit.

```
trellis version
```

No flags.

**Example**

```bash
trellis version
# 1.0.0
```

**Exit codes**: 0 always.
