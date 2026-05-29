[![build](https://img.shields.io/github/actions/workflow/status/trellis-md/trellis/ci.yml?branch=main)](https://github.com/trellis-md/trellis/actions)
[![scorecard](https://api.scorecard.dev/projects/github.com/trellis-md/trellis/badge)](https://scorecard.dev/viewer/?uri=github.com/trellis-md/trellis)
[![crates.io](https://img.shields.io/crates/v/trellis)](https://crates.io/crates/trellis)
[![npm](https://img.shields.io/npm/v/trellis-md)](https://www.npmjs.com/package/trellis-md)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

# Trellis

**API workspace** — design specs, send requests, validate contracts, from CLI and browser.

Trellis is a local-first, markdown-native API workspace. Endpoint specs, environments, and flows are ordinary markdown files. The Rust engine validates and executes them. The browser lets you edit, run, and design APIs visually. Agent skills make the same files usable from Claude Code, Cursor, GitHub Copilot, and others.

```bash
cargo install trellis
trellis init --name=my-api --dev-url=http://localhost:8080 --yes
trellis serve                          # opens trellis-ui
trellis request GET https://httpbin.org/get  # ad-hoc request (trellis-cli)
```

## Two interfaces, one binary

| Interface | Launch | Use for |
| --- | --- | --- |
| **trellis-cli** | `trellis <command>` | Scripts, CI, agents, ad-hoc requests |
| **trellis-ui** | `trellis serve` | Interactive design, debugging, review |

## Why Trellis

| Capability | What it means |
| --- | --- |
| Collections | `api-docs/` is a collection — auto-located from your git repo root. |
| Ad-hoc requests | `trellis request GET <url>` or "New Request" in the browser — no spec file needed. |
| API design | Write specs in markdown, validate contracts, iterate on design. |
| Markdown-native | Specs live in reviewable `.md` files alongside your code. |
| Local Rust binary | Fast CLI and browser preview without a hosted workspace. |
| Flow canvas | Connect endpoints, capture values, inject downstream — save as markdown. |
| Agent-native | Install skills and MCP tools for Claude Code, Cursor, Copilot, and others. |
| Import and scan | Import cURL/Postman/OpenAPI or scan a project for missing specs. |

## Project layout

```text
api-docs/
├── trellis.md
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
trellis init --name=my-api --dev-url=https://jsonplaceholder.typicode.com --yes
```

Run an endpoint:

```bash
trellis exec api-docs/apis/posts/get-posts.md --env=dev
```

Open the web preview:

```bash
trellis serve
```

Install AI agent skills:

```bash
trellis skills install --agent=claude-code
trellis skills install --agent=cursor
trellis skills install --agent=copilot
```

Scan an existing project for routes:

```bash
trellis import project .
```

## Installation

```bash
# Cargo
cargo install trellis

# Shell installer (macOS / Linux)
curl -fsSL https://trellis.dev/install.sh | sh

# npm wrapper
npm install -g trellis-md

# Homebrew
brew install trellis-md/tap/trellis

# Docker
docker run --rm -v "$(pwd)":/work -w /work ghcr.io/trellis-md/trellis:latest validate api-docs/
```

## CLI

```bash
trellis init
trellis validate api-docs/
trellis exec api-docs/apis/users/get-user-by-id.md --env=dev --var userId=42
trellis flow api-docs/flows/user-onboarding.md --env=dev
trellis import curl
trellis import project .
trellis skills install
trellis serve
trellis doctor
```

## Documentation

- [Getting started](https://trellis.dev/docs/quickstart)
- [CLI reference](https://trellis.dev/docs/cli/overview)
- [Configuration reference](https://trellis.dev/docs/reference/config)
- [Spec convention](docs/spec/convention.md)
- [Benchmarks](BENCHMARKS.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

Apache-2.0. See [LICENSE](LICENSE).
