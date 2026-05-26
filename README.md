[![build](https://img.shields.io/github/actions/workflow/status/trellis-md/trellis/ci.yml?branch=main)](https://github.com/trellis-md/trellis/actions)
[![crates.io](https://img.shields.io/crates/v/trellis)](https://crates.io/crates/trellis)
[![npm](https://img.shields.io/npm/v/@trellis-md/trellis)](https://www.npmjs.com/package/@trellis-md/trellis)
[![license](https://img.shields.io/crates/l/trellis)](LICENSE)

# Trellis

Trellis is a markdown-native API spec system: endpoint specs, pipelines, and environment config live as ordinary markdown files that are executed by a Rust engine, rendered in a browser preview, validated in CI, and read by AI coding agents — no separate schema format, no GUI, no build step.

## Quick start

```bash
cargo install trellis
trellis init --name=my-api --dev-url=http://localhost:8080
trellis exec api-docs/posts/get-posts.md --env=dev
```

## Features

| Feature | Description |
| --- | --- |
| Executable markdown | Endpoint files are valid markdown and runnable HTTP specs. |
| Pipelines | Chain endpoints, capture response values, inject into later steps. |
| Variable system | CLI flags, env.md, .env.local, and TRELLIS_* env vars with clear priority. |
| Secret detection | Parser refuses tokens and keys in versioned files (exit code 5). |
| Cross-agent skills | Install skill files for Claude Code, Cursor, and GitHub Copilot. |
| Web preview | `trellis serve` opens a live browser view of your specs. |
| Import | Convert Postman, Insomnia, or OpenAPI specs with one command. |
| Stable exit codes | 0 pass, 1 fail, 2 invalid spec, 3 engine error, 4 network, 5 secret. |

## Installation

```bash
# Cargo
cargo install trellis

# Shell installer (macOS / Linux)
curl -fsSL https://trellis.dev/install.sh | sh

# npm
npm install -g @trellis-md/trellis

# Homebrew
brew install trellis-md/tap/trellis

# Docker
docker run --rm -v "$(pwd)":/work -w /work ghcr.io/trellis-md/trellis:latest validate api-docs/
```

## Documentation

- [Getting started](docs/getting-started.md) — installation, first project, and the web preview.
- [CLI reference](docs/cli.md) — every command, flag, and exit code.
- [Configuration reference](docs/configuration.md) — project config, environments, variables, secrets, and auth.
- [Spec convention](docs/spec/convention.md) — canonical format for endpoint files and pipelines.
- [Migration guide](docs/guides/migration.md) — importing from Postman, Insomnia, and OpenAPI.

## License

Apache-2.0. See [LICENSE](LICENSE).
