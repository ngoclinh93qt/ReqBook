[![build](https://img.shields.io/github/actions/workflow/status/ngoclinh93qt/ReqBook/ci.yml?branch=main)](https://github.com/ngoclinh93qt/ReqBook/actions)
[![scorecard](https://api.scorecard.dev/projects/github.com/ngoclinh93qt/ReqBook/badge)](https://scorecard.dev/viewer/?uri=github.com/ngoclinh93qt/ReqBook)
[![crates.io](https://img.shields.io/crates/v/reqbook)](https://crates.io/crates/reqbook)
[![npm](https://img.shields.io/npm/v/reqbook)](https://www.npmjs.com/package/reqbook)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

# Reqbook

**Reqbook is executable API documentation for humans, CI, and coding agents.** Keep API docs, tests, and agent context in one Markdown source of truth.

```bash
cargo install reqbook
rqb init --name=my-api --dev-url=http://localhost:8080 --yes
rqb serve                          # opens rqb-ui
rqb request GET https://httpbin.org/get  # ad-hoc request (rqb-cli)
```

## Surfaces

| Interface | Launch | Use for |
| --- | --- | --- |
| **rqb-cli** | `rqb <command>` | Scripts, CI, agents, ad-hoc requests |
| **rqb-ui** | `rqb serve` | Interactive design, debugging, review |
| **VS Code extension** | `packages/vscode` | In-editor preview, run, validate, context, and variable autocomplete |

## Why Reqbook

| Capability | What it means |
| --- | --- |
| Collections | `api-docs/` is a collection — auto-located from your git repo root. |
| Ad-hoc requests | `rqb request GET <url>` or "New Request" in the browser — no spec file needed. |
| API design | Write specs in markdown, validate contracts, iterate on design. |
| Markdown-native | Specs live in reviewable `.md` files alongside your code. |
| Local Rust binary | Fast CLI and browser preview without a hosted workspace. |
| Flow canvas | Connect endpoints, capture values, inject downstream — save as markdown. |
| Agent-native | Give Claude Code, Cursor, Copilot, and others runnable API contracts they can read, write, and validate. |
| Contract checks | Run `rqb check` in CI with Markdown, GitHub, JUnit, or JSON reports. |
| Import, export, scan | Import cURL/Postman/Insomnia/OpenAPI/local client collections/`.http`, export OpenAPI, or scan a project for missing specs. |

## Project layout

```text
api-docs/
├── reqbook.md
├── _shared/
│   └── env.md
├── apis/
│   └── users/
│       └── get-user-by-id.md
└── flows/
    └── user-onboarding.md
```

## Quick start

Create a project:

```bash
rqb init --name=my-api --dev-url=https://jsonplaceholder.typicode.com --yes
```

Run an endpoint:

```bash
rqb exec api-docs/apis/posts/get-posts.md --env=dev
```

Open the web preview:

```bash
rqb serve
```

Install AI agent skills:

```bash
rqb skills install --agent=claude-code
rqb skills install --agent=cursor
rqb skills install --agent=copilot
```

Scan an existing project for routes:

```bash
rqb import project .
```

## Installation

```bash
# Cargo
cargo install reqbook

# Shell installer (macOS / Linux)
curl -fsSL https://markapidown.net/install.sh | sh

# npm wrapper
npm install -g reqbook

# Homebrew
brew install reqbook/tap/rqb

# Docker
docker run --rm -v "$(pwd)":/work -w /work ghcr.io/ngoclinh93qt/rqb:latest validate api-docs/
```

## CLI

```bash
rqb init
rqb validate api-docs/
rqb exec api-docs/apis/users/get-user-by-id.md --env=dev --var userId=42
rqb flow api-docs/flows/user-onboarding.md --env=dev
rqb check api-docs/ --changed-from origin/main --report github
rqb context users.create
rqb export openapi api-docs/ --out openapi.generated.yaml
rqb import curl
rqb import collection ./local-client-collection
rqb import http ./requests.http
rqb import project .
rqb skills install
rqb serve
rqb doctor
```

## Documentation

- [Getting started](https://docs.markapidown.net/quickstart)
- [CLI reference](https://docs.markapidown.net/cli/overview)
- [Configuration reference](https://docs.markapidown.net/reference/config)
- [Spec convention](docs/spec/convention.md)
- [Benchmarks](BENCHMARKS.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

Apache-2.0. See [LICENSE](LICENSE).
