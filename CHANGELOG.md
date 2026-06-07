# Changelog

## 0.1.0

Initial release of Reqbook — markdown-native API specs with execution, pipelines, and cross-agent workflows.

### Positioning

- Updated product framing to executable API documentation for humans, CI, and coding agents
- Avoids positioning Reqbook as a desktop API client clone; the source of truth remains Markdown in the repo

### Engine

- HTTP endpoint execution (`rqb exec`) with variable resolution, secret detection, and response diffing
- Response match modes: default JSON shape matching, strict exact matching, and JSON Schema validation
- `response.match: strict` and `response.match: schema` frontmatter support, plus `http strict` fence syntax
- Structured assertion failures can be promoted to contract failures with `--strict-assertions` or strict response mode
- Pipeline execution (`rqb flow`) with step captures, inject, and assert
- Variable resolution priority: pipeline → CLI → endpoint frontmatter → env.md → .env.local → `RQB_*` OS vars
- Secret detection in versioned markdown — exits 5 on `sk_`, `pk_live_`, `Bearer eyJ`, or long hex strings
- Auth header masking in CLI output, reports, and web preview
- Retry policy with fixed and exponential backoff
- Output formats: `console`, `junit`, `json`, `markdown`

### CLI

- `rqb init` — scaffold `api-docs/` with example endpoint, env config, and `.gitignore`
- `rqb validate` — validate one file or a directory tree; exit codes 0/2/5
- `rqb exec` — execute one endpoint with `--env`, `--var`, `--dry-run`, `--timeout`, `--output`
- `rqb flow` — execute a pipeline with `--parallel`/`--no-parallel`
- `rqb check` — PR-focused contract checks with Markdown, GitHub, JUnit, and JSON reports
- `rqb context` — bounded executable endpoint, flow, or changed-spec context for coding agents
- `rqb index` — regenerate `api-docs/README.md`
- `rqb import postman` — import Postman Collection v2.1 JSON
- `rqb import insomnia` — import Insomnia v4 JSON
- `rqb import openapi` — import OpenAPI 3.x YAML or JSON
- `rqb import collection` — import local API client collection directories
- `rqb import http` — import `.http` / REST Client files
- `rqb export openapi` — export Reqbook endpoint specs as OpenAPI 3.x YAML or JSON
- `rqb skills install/list/uninstall` — install cross-agent skills for Claude Code, Cursor, Copilot, and more
- `rqb serve` — web preview at `http://127.0.0.1:8080` with spec browser and live Run button
- `rqb doctor [--fix]` — diagnose project setup (api-docs, .env.local, agents, network)
- `rqb completion` — shell completion for bash, zsh, fish, PowerShell

### Editor workflow

- VS Code extension MVP in `packages/vscode`
- Commands for endpoint preview, endpoint execution, current-file validation, and compact agent context
- Variable autocomplete from `_shared/env.md`, `.env.local`, current spec path params, and flow captures
- Result panel for run, validate, and context output

### Cross-agent skills

- `rqb-author` — skill for authoring endpoint specs in correct Reqbook format
- `rqb-exec` — skill for running specs and interpreting results
- `rqb-flow` — skill for building and running pipelines
- Formats: Claude Code (SKILL.md), Cursor (.mdc), GitHub Copilot (.instructions.md), Codex CLI, Antigravity, OpenCode

### Web preview

- Axum HTTP server with askama HTML templates
- Endpoint browser grouped by resource with method badges
- Individual spec pages with request/response blocks
- Live Run button that executes specs and displays JSON results
- Loads `_shared/env.md` variables for the selected environment

### Distribution

- `cargo install reqbook`
- Shell installer: `curl -fsSL https://markapidown.net/install.sh | sh`
- PowerShell installer for Windows
- npm: `npm install -g reqbook`
- Homebrew tap: `brew install reqbook/tap/rqb`
- Docker: `ghcr.io/ngoclinh93qt/reqbook:latest`
- Targets: `x86_64`/`aarch64` Linux (musl), macOS, and Windows (MSVC)

### Spec convention

- Markdown-native endpoint files with YAML frontmatter and `## Request`, `## Expected response`, `## Tests`, `## Notes` sections
- Pipeline files with ordered steps, `Capture`, `Inject`, `Assert` directives
- `_shared/env.md` for non-secret environment variables
- `api-docs/reqbook.md` for project configuration
- Full spec documented in `docs/spec/convention.md`
