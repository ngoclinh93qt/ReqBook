# Trellis v1.0.0 build plan

This file is the shared coordination plan for all agents working on Trellis. Keep it current as phases are completed, blocked, or revised. Do not remove historical notes that explain decisions.

## Current status

- Current phase: Phase 5 - distribution: acceptance passed; commit pending
- Repository status at start: empty workspace, no git repository
- Active instruction: work phases in order, run each phase acceptance check, commit before moving on

## Phase checklist

1. Phase 1 - markdown convention: completed and committed (`efaad0c docs: define markdown convention`)
2. Phase 2 - engine core: completed and committed (`31c0f1c feat: implement engine core`)
3. Phase 3 - CLI surface: completed and committed (`cd6f3e7 feat: add cli surface`)
4. Phase 4 - cross-agent skill compatibility: completed and committed (`c35a312 feat: add cross-agent skills`)
5. Phase 5 - distribution: acceptance passed, commit pending
6. Phase 6 - migration tools: pending
7. Phase 7 - web preview: pending
8. Phase 8 - documentation site: pending
9. Phase 9 - examples: pending
10. Phase 10 - CI/CD: pending
11. Phase 11 - acceptance, polish, launch readiness: pending

## Coordination rules

- Follow the phase order exactly.
- Before starting a phase, verify that the previous phase has passed its acceptance checks and has been committed.
- If a later phase exposes a defect in an earlier phase, fix the earlier phase first and commit the fix.
- Preserve markdown-native configuration. Do not introduce TOML, JSON, or YAML project config files beyond Rust/package tooling files that require them.
- Keep production code free of stubs, unchecked secrets, and untracked TODO/FIXME comments.

## Phase 1 acceptance tracking

- `docs/spec/convention.md` standalone-readable: passed
- Edge cases documented: passed
- "Migrating from Postman" concept mapping table included: passed

## Phase 1 validation notes

- The convention document defines project layout, endpoint frontmatter, endpoint sections, variable resolution, pipeline format, markdown-native project config, security rules, reports, exit codes, and a complete minimal example.
- Edge cases covered include missing frontmatter, missing required fields, missing sections, multiple `http` blocks, invalid request blocks, unsupported protocols, environment mismatches, missing variables, nested variables, and secret detection.
- Postman migration includes a concept mapping table and migration workflow.

## Phase 2 acceptance tracking

- `cargo test`: passed
- `cargo clippy -- -D warnings`: passed
- `cargo fmt --check`: passed
- Real GET to `https://httpbin.org/get` integration test: passed
- Secret detection catches token in `env.md` and exits 5: passed
- Benchmark parse 50-line endpoint under 1 ms: passed at approximately 18 microseconds
- Coverage on `parser`, `resolver`, `engine`, `pipeline`: passed

Coverage measured with `cargo llvm-cov --summary-only`:

- `parser`: 85.54% line coverage
- `resolver`: 85.05% line coverage
- `engine`: 87.27% line coverage
- `pipeline`: 86.39% line coverage
- Total crate: 82.56% line coverage

Phase 2 final command results:

- `cargo test`: 27 unit tests, 1 httpbin integration test, and doc tests passed
- `cargo clippy -- -D warnings`: passed
- `cargo fmt --check`: passed
- `cargo bench --bench parse_endpoint -- --sample-size 10`: `parse_50_line_endpoint` measured approximately 15.8 microseconds
- Secret detection CLI check: `trellis validate api-docs/_shared/env.md` exits 5 for `sk_` token

## Phase 3 acceptance tracking

- `trellis --help` shows clean output and lists all required commands: passed
- `trellis init` in an empty directory creates a working `api-docs/` project: passed at approximately 0.06 seconds for init plus validate
- `trellis completion bash > /tmp/c && source /tmp/c` enables a bash completion registration: passed
- `trellis doctor` runs in under 500 ms with project, agent, and network diagnostics: passed at approximately 0.04 seconds in the initialized project check
- Error messages include path and suggested fix where applicable: passed for invalid specs, secret detection, and missing files

Phase 3 implementation notes:

- Full clap command surface is present: `init`, `validate`, `exec`, `flow`, `index`, `import`, `skills`, `serve`, `doctor`, `completion`, and `version`.
- `import`, `skills install/uninstall`, and `serve` are wired into the CLI with actionable errors until their implementation phases complete.
- `trellis init` creates `.gitignore` with `.env.local` to satisfy the security convention and doctor check.

## Phase 4 acceptance tracking

- `trellis skills install --agent=claude-code` creates `.claude/skills/trellis-{author,exec,flow}/SKILL.md`: passed
- `trellis skills install --agent=cursor` creates `.cursor/rules/trellis-{author,exec,flow}.mdc`: passed
- `trellis skills install --agent=copilot` creates `.github/instructions/trellis-*.instructions.md`: passed
- `trellis skills install` without `--agent` auto-detects at least one agent: passed using `.opencode/`
- Generated YAML frontmatter validates for Claude Code, Cursor, and Copilot formats: passed
- `trellis skills uninstall` removes installed workspace skill files: passed
- Real Claude Code non-interactive smoke check with trigger phrase: passed; Claude returned `trellis-author`

## Phase 5 acceptance tracking

- `cargo build --release --no-default-features --features minimal` (Dockerfile minimal build): passed at approximately 17 seconds
- `cargo test`: passed (all 28 tests including httpbin integration)
- `cargo clippy -- -D warnings`: passed
- `cargo fmt --check`: passed
- `packages/npm/package.json` is valid JSON with correct `bin` and `files` fields: passed
- `scripts/install.sh` syntax check (`bash -n`): passed
- All distribution artifacts present: Dockerfile, `.dockerignore`, `.github/workflows/release.yml`, `CHANGELOG.md`, `README.md`, `packages/npm/`, `scripts/`, `wix/main.wxs`: passed
- `Cargo.toml` metadata complete (authors, homepage, documentation, readme, keywords, categories): passed
- `[workspace.metadata.dist]` targets 5 platforms (linux-musl x86/arm, macOS x86/arm, Windows msvc): passed
- `[profile.dist]` with release-grade settings present: passed
- `[package.metadata.wix]` with upgrade/path GUIDs present: passed

Phase 5 implementation notes:

- `[workspace.metadata.dist]` configures cargo-dist 0.31.0 with shell, powershell, homebrew, npm, and msi installers.
- `scripts/install.sh` supports Linux/macOS with SHA256 verification, automatic PATH selection, and `--version` override.
- `scripts/install.ps1` supports Windows x86_64/arm64 with SHA256 verification.
- `packages/npm/trellis.js` is a Node.js binary wrapper that downloads and caches the correct platform binary on first run.
- Dockerfile uses a two-stage Alpine build with the `minimal` feature set for a minimal image size.
- `[profile.dist]` is pinned to `opt-level = "z"` with `strip = true` for smallest binary size.
- clap and clap_complete versions are pinned exactly (`=4.5.23`, `=4.5.40`) to ensure reproducible dist builds.
- `src/lib.rs` adds `#[cfg(feature = "install")]` gate on the installer module so the minimal build compiles without it.

## Phase 4 implementation notes:

- Canonical skill sources live in `skills/trellis-author/SKILL.md`, `skills/trellis-exec/SKILL.md`, and `skills/trellis-flow/SKILL.md`.
- Cursor and Copilot formats are generated from the same canonical SKILL.md frontmatter and body.
- Auto-detection follows the configured agent matrix. During local validation, temporary `HOME` was used for CLI checks that should not touch the developer's real global skill directories.
