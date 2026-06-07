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
| **Reqbook desktop** | `cargo run -p rqb-desktop` | Native desktop shell around the local web preview |
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

Run the desktop app from source:

```bash
cd web && npm ci && npm run build
cd ..
cargo run -p rqb-desktop
```

Run the flow canvas E2E:

```bash
cd web
NPM_CONFIG_UPDATE_NOTIFIER=false npm ci
npm run build
npx playwright install chromium
cd ..
cargo build --locked
cd web
npm run e2e:flow
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

Until the first public release artifacts are published, install Reqbook from a local checkout:

```bash
git clone https://github.com/ngoclinh93qt/ReqBook.git
cd ReqBook
cargo install --path .
```

Package-manager channels such as crates.io, the shell installer, npm, Homebrew, Docker, and GitHub Release binaries should be documented here only after the corresponding public release artifacts are available.

## CLI

```bash
rqb init
rqb validate api-docs/
rqb exec api-docs/apis/users/get-user-by-id.md --env=dev --var userId=42
rqb diagnose api-docs/apis/users/get-user-by-id.md --env=dev --output=json
rqb flow api-docs/flows/user-onboarding.md --env=dev
rqb flow api-docs/flows/user-onboarding.md --dry-run --output json
rqb check api-docs/ --changed-from origin/main --report github
rqb context users.create --mode surgical --intent implement --brief --max-fields 12 --include variables,request,response,errors,rules,verify
rqb context flow user-onboarding --mode schema --output json
rqb agent pack flow user-onboarding --mode surgical --brief --out .reqbook/agent-context.md
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
- [Configuration reference](https://docs.markapidown.net/reference/configuration)
- [E2E testing](https://docs.markapidown.net/guides/e2e-testing)
- [Desktop smoke testing](https://docs.markapidown.net/guides/desktop-smoke)
- [Spec convention](docs/spec/convention.md)
- [Benchmarks](BENCHMARKS.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

Apache-2.0. See [LICENSE](LICENSE).
