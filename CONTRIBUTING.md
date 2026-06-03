# Contributing to Reqbook

Thanks for helping improve Reqbook. This project is still moving quickly, so small, focused changes are easiest to review.

## Development setup

Requirements:

- Rust 1.75 or newer
- Node.js 22 or newer
- npm

```bash
git clone https://github.com/ngoclinh93qt/ReqBook.git
cd rqb
cargo build
cd web && npm ci && npm run build
```

## Common commands

```bash
cargo fmt --check
cargo clippy --locked -- -D warnings
cargo test --locked
cargo test --locked --no-default-features --features minimal
cd web && npm run build
cargo run --bin rqb -- validate api-docs
```

Or run the aggregated local release check:

```bash
make release-check
```

## Pull requests

- Keep PRs focused on one behavior or topic.
- Add tests for parser, resolver, engine, installer, importer, or preview changes.
- Update docs when changing CLI behavior, file layout, or markdown schema.
- Do not include secrets, tokens, private URLs, or production credentials in examples.
- Do not commit generated local artifacts outside files intentionally produced by the build.

## Commit style

Use Conventional Commits when practical:

```text
feat: add flow validation endpoint
fix: preserve api-docs/apis paths in scanner
docs: document release checklist
```

## Release changes

Release-facing changes should include:

- Passing `make release-check`
- Updated `CHANGELOG.md`
- Updated docs or examples when user-facing behavior changes
- A note about any migration needed
