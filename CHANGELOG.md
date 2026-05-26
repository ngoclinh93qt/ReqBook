# Changelog

## 1.0.0

Initial release of Trellis — markdown-native API specs with execution, pipelines, and cross-agent workflows.

### Engine

- HTTP endpoint execution (`trellis exec`) with variable resolution, secret detection, and response diffing
- Pipeline execution (`trellis flow`) with step captures, inject, and assert
- Variable resolution priority: pipeline → CLI → endpoint frontmatter → env.md → .env.local → `TRELLIS_*` OS vars
- Secret detection in versioned markdown — exits 5 on `sk_`, `pk_live_`, `Bearer eyJ`, or long hex strings
- Auth header masking in CLI output, reports, and web preview
- Retry policy with fixed and exponential backoff
- Output formats: `console`, `junit`, `json`, `markdown`

### CLI

- `trellis init` — scaffold `api-docs/` with example endpoint, env config, and `.gitignore`
- `trellis validate` — validate one file or a directory tree; exit codes 0/2/5
- `trellis exec` — execute one endpoint with `--env`, `--var`, `--dry-run`, `--timeout`, `--output`
- `trellis flow` — execute a pipeline with `--parallel`/`--no-parallel`
- `trellis index` — regenerate `api-docs/README.md`
- `trellis import postman` — import Postman Collection v2.1 JSON
- `trellis import insomnia` — import Insomnia v4 JSON
- `trellis import openapi` — import OpenAPI 3.x YAML or JSON
- `trellis skills install/list/uninstall` — install cross-agent skills for Claude Code, Cursor, Copilot, and more
- `trellis serve` — web preview at `http://127.0.0.1:8080` with spec browser and live Run button
- `trellis doctor [--fix]` — diagnose project setup (api-docs, .env.local, agents, network)
- `trellis completion` — shell completion for bash, zsh, fish, PowerShell

### Cross-agent skills

- `trellis-author` — skill for authoring endpoint specs in correct Trellis format
- `trellis-exec` — skill for running specs and interpreting results
- `trellis-flow` — skill for building and running pipelines
- Formats: Claude Code (SKILL.md), Cursor (.mdc), GitHub Copilot (.instructions.md), Codex CLI, Antigravity, OpenCode

### Web preview

- Axum HTTP server with askama HTML templates
- Endpoint browser grouped by resource with method badges
- Individual spec pages with request/response blocks
- Live Run button that executes specs and displays JSON results
- Loads `_shared/env.md` variables for the selected environment

### Distribution

- `cargo install trellis`
- Shell installer: `curl -fsSL https://trellis.dev/install.sh | sh`
- PowerShell installer for Windows
- npm: `npm install -g trellis-md`
- Homebrew tap: `brew install trellis-md/tap/trellis`
- Docker: `ghcr.io/trellis-md/trellis:latest`
- Targets: `x86_64`/`aarch64` Linux (musl), macOS, and Windows (MSVC)

### Spec convention

- Markdown-native endpoint files with YAML frontmatter and `## Request`, `## Expected response`, `## Tests`, `## Notes` sections
- Pipeline files with ordered steps, `Capture`, `Inject`, `Assert` directives
- `_shared/env.md` for non-secret environment variables
- `api-docs/trellis.md` for project configuration
- Full spec documented in `docs/spec/convention.md`
