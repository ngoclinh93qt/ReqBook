# Trellis documentation

Trellis is a markdown-native API spec system: you write endpoint specs, pipelines, and environment config as ordinary markdown files, and the same files are executed by the Rust engine, rendered in a browser preview, validated in CI, and read by AI coding agents. There is no separate YAML schema, no proprietary format, and no GUI required to author or run specs.

## Key features

- **Executable markdown** — endpoint files double as documentation and as runnable HTTP requests with no separate tooling.
- **Pipelines** — chain multiple endpoints, capture response values, and inject them into later steps, sequentially or in parallel.
- **Variable system** — resolve values from pipeline captures, CLI flags, endpoint frontmatter, `env.md`, `.env.local`, and `TRELLIS_*` environment variables with a clear priority order.
- **Secret detection** — the parser refuses tokens, keys, and credentials in versioned markdown files and exits with code 5 before any request is sent.
- **Cross-agent skills** — install Trellis skill files into Claude Code, Cursor, GitHub Copilot, and other AI agents so they can author and execute specs.
- **Web preview** — `trellis serve` starts a local browser UI on the same markdown files, with no build step.
- **Import** — convert existing Postman, Insomnia, or OpenAPI specs into Trellis markdown with a single command.
- **Stable exit codes** — every command returns a predictable exit code so CI pipelines can act on pass, failure, invalid spec, or secret detection independently.

## Quick start

```bash
# Install
cargo install trellis

# Create a project
trellis init --name=my-api --dev-url=http://localhost:8080

# Execute an endpoint
trellis exec api-docs/apis/posts/get-posts.md --env=dev
```

## Documentation pages

- [Getting started](getting-started.md) — installation, first project, validation, and the web preview.
- [CLI reference](cli.md) — every command, flag, and exit code.
- [Configuration reference](configuration.md) — project config, environment config, variables, secrets, and auth.
- [Spec convention](spec/convention.md) — the canonical specification for endpoint files, pipelines, and project layout.
- [Release checklist](release.md) — release gates and smoke tests.
- [Launch plan](launch.md) — open-source launch strategy.
- [Migration guide](guides/migration.md) — importing from Postman, Insomnia, and OpenAPI.
