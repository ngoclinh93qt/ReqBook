[![build](https://img.shields.io/github/actions/workflow/status/trellis-md/trellis/ci.yml?branch=main)](https://github.com/trellis-md/trellis/actions)
[![scorecard](https://api.scorecard.dev/projects/github.com/trellis-md/trellis/badge)](https://scorecard.dev/viewer/?uri=github.com/trellis-md/trellis)
[![crates.io](https://img.shields.io/crates/v/trellis)](https://crates.io/crates/trellis)
[![npm](https://img.shields.io/npm/v/trellis-md)](https://www.npmjs.com/package/trellis-md)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

# Trellis

Trellis is a local-first, markdown-native API spec and workflow tool built for AI coding agents.

Endpoint specs, environments, and flows are ordinary markdown files. The Rust engine validates and executes them, the browser preview lets you edit and run them, and agent skills make the same files usable from Claude Code, Codex CLI, Cursor, GitHub Copilot, Antigravity, and OpenCode.

```bash
cargo install trellis
trellis init --name=my-api --dev-url=http://localhost:8080 --yes
trellis serve
```

## Why Trellis

| Capability | What it means |
| --- | --- |
| Markdown-native | API specs, config, env values, and flows live in reviewable `.md` files. |
| Local Rust binary | Fast CLI and browser preview without a hosted workspace. |
| Browser execute + edit | Run endpoints, tweak variables, edit markdown, and save back to disk. |
| Flow canvas | Connect endpoint blocks, capture response values, inject them downstream, and save as markdown. |
| Agent-native | Install one canonical skill set across major AI coding agents. |
| Import and scan | Import cURL/Postman/OpenAPI or scan a project for missing API specs. |

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

- [Getting started](docs/getting-started.md)
- [CLI reference](docs/cli.md)
- [Configuration reference](docs/configuration.md)
- [Spec convention](docs/spec/convention.md)
- [Migration guide](docs/guides/migration.md)
- [Benchmarks](BENCHMARKS.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## Release status

Trellis is preparing its first public release. The recommended first public tag is `v0.1.0` or `v1.0.0-rc1`, followed by a short dogfood period before a stable `v1.0.0`.

Before cutting a release:

```bash
make release-check
cargo dist build
```

## License

Apache-2.0. See [LICENSE](LICENSE).
