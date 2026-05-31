[![build](https://img.shields.io/github/actions/workflow/status/ngoclinh93qt/MarkApiDown/ci.yml?branch=main)](https://github.com/ngoclinh93qt/MarkApiDown/actions)
[![scorecard](https://api.scorecard.dev/projects/github.com/ngoclinh93qt/MarkApiDown/badge)](https://scorecard.dev/viewer/?uri=github.com/ngoclinh93qt/MarkApiDown)
[![crates.io](https://img.shields.io/crates/v/mark-api-down)](https://crates.io/crates/mark-api-down)
[![npm](https://img.shields.io/npm/v/mark-api-down)](https://www.npmjs.com/package/mark-api-down)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

# MarkApiDown

**API workspace** — design specs, send requests, validate contracts, from CLI and browser.

MarkApiDown is a local-first, markdown-native API workspace. Endpoint specs, environments, and flows are ordinary markdown files. The Rust engine validates and executes them. The browser lets you edit, run, and design APIs visually. Agent skills make the same files usable from Claude Code, Cursor, GitHub Copilot, and others.

```bash
cargo install mark-api-down
mad init --name=my-api --dev-url=http://localhost:8080 --yes
mad serve                          # opens mad-ui
mad request GET https://httpbin.org/get  # ad-hoc request (mad-cli)
```

## Two interfaces, one binary

| Interface | Launch | Use for |
| --- | --- | --- |
| **mad-cli** | `mad <command>` | Scripts, CI, agents, ad-hoc requests |
| **mad-ui** | `mad serve` | Interactive design, debugging, review |

## Why MarkApiDown

| Capability | What it means |
| --- | --- |
| Collections | `api-docs/` is a collection — auto-located from your git repo root. |
| Ad-hoc requests | `mad request GET <url>` or "New Request" in the browser — no spec file needed. |
| API design | Write specs in markdown, validate contracts, iterate on design. |
| Markdown-native | Specs live in reviewable `.md` files alongside your code. |
| Local Rust binary | Fast CLI and browser preview without a hosted workspace. |
| Flow canvas | Connect endpoints, capture values, inject downstream — save as markdown. |
| Agent-native | Install skills and MCP tools for Claude Code, Cursor, Copilot, and others. |
| Import and scan | Import cURL/Postman/OpenAPI or scan a project for missing specs. |

## Project layout

```text
api-docs/
├── mad.md
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
mad init --name=my-api --dev-url=https://jsonplaceholder.typicode.com --yes
```

Run an endpoint:

```bash
mad exec api-docs/apis/posts/get-posts.md --env=dev
```

Open the web preview:

```bash
mad serve
```

Install AI agent skills:

```bash
mad skills install --agent=claude-code
mad skills install --agent=cursor
mad skills install --agent=copilot
```

Scan an existing project for routes:

```bash
mad import project .
```

## Installation

```bash
# Cargo
cargo install mark-api-down

# Shell installer (macOS / Linux)
curl -fsSL https://markapidown.net/install.sh | sh

# npm wrapper
npm install -g mark-api-down

# Homebrew
brew install mark-api-down/tap/mad

# Docker
docker run --rm -v "$(pwd)":/work -w /work ghcr.io/ngoclinh93qt/markapidown:latest validate api-docs/
```

## CLI

```bash
mad init
mad validate api-docs/
mad exec api-docs/apis/users/get-user-by-id.md --env=dev --var userId=42
mad flow api-docs/flows/user-onboarding.md --env=dev
mad import curl
mad import project .
mad skills install
mad serve
mad doctor
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
