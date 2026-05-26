# Trellis v1.0.0 build plan

This file is the shared coordination plan for all agents working on Trellis. Keep it current as phases are completed, blocked, or revised. Do not remove historical notes that explain decisions.

## Current status

- Current phase: Phase 2 - engine core acceptance passed; commit pending
- Repository status at start: empty workspace, no git repository
- Active instruction: work phases in order, run each phase acceptance check, commit before moving on

## Phase checklist

1. Phase 1 - markdown convention: completed and committed (`efaad0c docs: define markdown convention`)
2. Phase 2 - engine core: acceptance passed, commit pending
3. Phase 3 - CLI surface: pending
4. Phase 4 - cross-agent skill compatibility: pending
5. Phase 5 - distribution: pending
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
