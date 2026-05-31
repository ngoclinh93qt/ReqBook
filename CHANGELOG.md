# Changelog

## 0.1.0

Initial release of MarkApiDown — markdown-native API specs with execution, pipelines, and cross-agent workflows.

### Engine

- HTTP endpoint execution (`mad exec`) with variable resolution, secret detection, and response diffing
- Pipeline execution (`mad flow`) with step captures, inject, and assert
- Variable resolution priority: pipeline → CLI → endpoint frontmatter → env.md → .env.local → `MAD_*` OS vars
- Secret detection in versioned markdown — exits 5 on `sk_`, `pk_live_`, `Bearer eyJ`, or long hex strings
- Auth header masking in CLI output, reports, and web preview
- Retry policy with fixed and exponential backoff
- Output formats: `console`, `junit`, `json`, `markdown`

### CLI

- `mad init` — scaffold `api-docs/` with example endpoint, env config, and `.gitignore`
- `mad validate` — validate one file or a directory tree; exit codes 0/2/5
- `mad exec` — execute one endpoint with `--env`, `--var`, `--dry-run`, `--timeout`, `--output`
- `mad flow` — execute a pipeline with `--parallel`/`--no-parallel`
- `mad index` — regenerate `api-docs/README.md`
- `mad import postman` — import Postman Collection v2.1 JSON
- `mad import insomnia` — import Insomnia v4 JSON
- `mad import openapi` — import OpenAPI 3.x YAML or JSON
- `mad skills install/list/uninstall` — install cross-agent skills for Claude Code, Cursor, Copilot, and more
- `mad serve` — web preview at `http://127.0.0.1:8080` with spec browser and live Run button
- `mad doctor [--fix]` — diagnose project setup (api-docs, .env.local, agents, network)
- `mad completion` — shell completion for bash, zsh, fish, PowerShell

### Cross-agent skills

- `mad-author` — skill for authoring endpoint specs in correct MarkApiDown format
- `mad-exec` — skill for running specs and interpreting results
- `mad-flow` — skill for building and running pipelines
- Formats: Claude Code (SKILL.md), Cursor (.mdc), GitHub Copilot (.instructions.md), Codex CLI, Antigravity, OpenCode

### Web preview

- Axum HTTP server with askama HTML templates
- Endpoint browser grouped by resource with method badges
- Individual spec pages with request/response blocks
- Live Run button that executes specs and displays JSON results
- Loads `_shared/env.md` variables for the selected environment

### Distribution

- `cargo install mark-api-down`
- Shell installer: `curl -fsSL https://markapidown.net/install.sh | sh`
- PowerShell installer for Windows
- npm: `npm install -g mark-api-down`
- Homebrew tap: `brew install mark-api-down/tap/mad`
- Docker: `ghcr.io/ngoclinh93qt/markapidown:latest`
- Targets: `x86_64`/`aarch64` Linux (musl), macOS, and Windows (MSVC)

### Spec convention

- Markdown-native endpoint files with YAML frontmatter and `## Request`, `## Expected response`, `## Tests`, `## Notes` sections
- Pipeline files with ordered steps, `Capture`, `Inject`, `Assert` directives
- `_shared/env.md` for non-secret environment variables
- `api-docs/mad.md` for project configuration
- Full spec documented in `docs/spec/convention.md`
