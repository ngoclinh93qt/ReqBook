# Release checklist

Use this checklist for every public Trellis release.

## Choose the release type

Recommended first public release:

- `v0.1.0` for a first beta with real users.
- `v1.0.0-rc1` only if the markdown schema and CLI surface are considered stable.

Avoid tagging `v1.0.0` until at least a few external users have installed Trellis and run `trellis init`, `trellis serve`, and a basic flow.

## Preflight

```bash
make release-check
cargo bench --bench parse_endpoint
cargo build --release --locked
cargo dist build
```

Update:

- `CHANGELOG.md`
- `BENCHMARKS.md`
- README screenshots or demo GIF links
- Docs pages affected by CLI/schema changes

## Smoke test

Run from a clean directory:

```bash
trellis version
trellis init --name=demo --dev-url=https://jsonplaceholder.typicode.com --yes
trellis validate api-docs/
trellis serve
trellis skills install --agent=claude-code
```

Also test:

- macOS Apple Silicon
- Linux x86_64
- Windows 11
- `cargo install trellis`
- npm wrapper install
- Docker image

## Cut release

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow runs:

- release checks
- cargo-dist artifact build
- optional crates.io publish when `PUBLISH_CRATES=true`
- optional npm publish when `PUBLISH_NPM=true`
- Docker image publish
- docs deploy

## Post-release

- Verify GitHub release artifacts and checksums.
- Verify shell installer against the new tag.
- Verify Docker pull.
- Verify crates.io and npm pages if publishing was enabled.
- Create a short release discussion.
- Open follow-up issues for deferred work instead of hiding it in notes.

## Launch notes

The public message should be narrow:

> Trellis is a local-first, markdown-native API spec and workflow tool built for AI coding agents.

Lead with:

- Markdown specs in PRs
- Local Rust binary
- Browser execute/edit preview
- Flow canvas saved as markdown
- Agent skills across major coding assistants
