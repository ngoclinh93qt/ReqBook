# Benchmarks

Benchmarks are captured before release candidates and after performance-sensitive changes. The local harness measures the release binary because that is what users install.

## Current Local Command Results

Captured: 2026-06-01  
Machine: Darwin 25.3.0 arm64  
Binary: `target/release/mad`  
Command: `cargo build --release --locked && node scripts/benchmark.mjs`

| Metric | Target | Measured | Notes |
| --- | ---: | ---: | --- |
| Default binary size | < 10 MiB | 3.83 MiB | release binary |
| Cold start, `mad --help` mean | < 20 ms | 2.43 ms | 30 runs; p95 2.73 ms |
| `mad validate <file>` mean | < 25 ms | 12.9 ms | 30 runs; p95 14.0 ms |
| `mad validate <collection>` mean | < 100 ms | 15.0 ms | 9 markdown files; p95 16.3 ms |
| Parser benchmark, 50-line endpoint | < 100 us | 15.301 us | `cargo bench --bench parse_endpoint -- --sample-size 10` |
| Web first response | < 100 ms | 36.2 ms | localhost server, first HTTP response |

## Agent Token Benchmark

This benchmark runs Codex as the agent against the same local fixture in two modes:

- **Without MarkApiDown:** the agent can inspect only implementation source under `examples/agent-token-api/src/`.
- **With MarkApiDown:** the agent uses `examples/agent-token-api/api-docs/` and may validate it with `mad`.

The fixture is intentionally local and does not require a network service. The harness stores raw JSONL, prompts, and summaries under `target/token-benchmarks/codex/<timestamp>/`.

Captured: 2026-06-01T15:06:43.918Z  
Machine: Darwin 25.3.0 arm64  
Codex: `codex-cli 0.135.0`  
Fixture: `examples/agent-token-api`  
Command: `node scripts/codex-token-benchmark.mjs`

| Scenario | Usage runs | Mean total tokens | Mean uncached tokens | Notes |
| --- | ---: | ---: | ---: | --- |
| Without MarkApiDown (source only) | 1/1 | 72,527 | 10,703 | Source files only |
| With MarkApiDown specs | 1/1 | 56,025 | 14,041 | API docs plus validation |

Total-token comparison: MarkApiDown used **22.8% fewer total tokens** in this run (`1.29x` without / with MarkApiDown). Codex also reports cached input tokens, so uncached-token accounting is recorded separately; in this run MarkApiDown used **31.2% more uncached tokens**.

## Test Status

| Check | Status | Notes |
| --- | --- | --- |
| `cargo check --locked` | Pass | Full feature set |
| `cargo test --locked --lib --bins` | Pass | 130 unit tests |
| `cargo test --locked --no-default-features --features minimal --lib --bins` | Pass | 103 unit tests |
| `cd web && npm run build` | Pass | Vite app build |
| `cd ../landingpage && npm run build` | Pass | Requires Node >= 22.12; Cloudflare adapter binds a local inspection port |
| `cargo test --locked` | Blocked by external service | `tests/httpbin.rs` timed out against `https://httpbin.org/get` from this environment |

## Commands

```bash
# Build the web UI and release binary, then run local timing benchmarks.
make bench

# Parser micro-benchmark.
cargo bench --bench parse_endpoint -- --sample-size 10

# Agent token benchmark. Requires a working Codex CLI login.
make bench-agent-token

# Non-network tests.
cargo test --locked --lib --bins
cargo test --locked --no-default-features --features minimal --lib --bins
```

## Release Notes Checklist

Record these with every release benchmark pass:

- machine model and OS,
- Rust version,
- Node version,
- Git commit,
- build features,
- benchmark command,
- any external-service failures or variance.
