# Benchmarks

Benchmarks are captured before release candidates and after performance-sensitive changes. The local harness measures the release binary because that is what users install.

## Current Local Command Results

Captured: 2026-06-01  
Machine: Darwin 25.3.0 arm64  
Binary: `target/release/rqb`  
Command: `cargo build --release --locked && node scripts/benchmark.mjs`

| Metric | Target | Measured | Notes |
| --- | ---: | ---: | --- |
| Default binary size | < 10 MiB | 3.83 MiB | release binary |
| Cold start, `rqb --help` mean | < 20 ms | 2.43 ms | 30 runs; p95 2.73 ms |
| `rqb validate <file>` mean | < 25 ms | 12.9 ms | 30 runs; p95 14.0 ms |
| `rqb validate <collection>` mean | < 100 ms | 15.0 ms | 9 markdown files; p95 16.3 ms |
| Parser benchmark, 50-line endpoint | < 100 us | 15.301 us | `cargo bench --bench parse_endpoint -- --sample-size 10` |
| Web first response | < 100 ms | 36.2 ms | localhost server, first HTTP response |

## Agent Token Benchmark

This benchmark runs Codex as the agent against the same local fixture in two modes:

- **Without Reqbook:** the agent can inspect only implementation source under `examples/agent-token-api/src/`.
- **With Reqbook:** the agent locates one concrete endpoint spec, then uses `rqb context --mode surgical --brief --max-fields 12 --include variables,request,response,errors,rules,verify --no-guidance --token-budget 800` against `examples/agent-token-api/api-docs/` and validates it with `rqb`.

The fixture is intentionally local and does not require a network service. The harness stores raw JSONL, prompts, and summaries under `target/token-benchmarks/codex/<timestamp>/`.

Latest rerun after adding rules/constraints, literal error codes, and a quality-focused prompt: 2026-06-07T23:24:42.105Z
Machine: Darwin 25.3.0 arm64
Codex: `codex-cli 0.135.0`
Fixture: `examples/agent-token-api`
Command: `node scripts/codex-token-benchmark.mjs`
Artifact: `target/token-benchmarks/codex/2026-06-07T23-24-42-104Z/summary.md`

| Scenario | Usage runs | Mean total tokens | Mean uncached tokens | Notes |
| --- | ---: | ---: | ---: | --- |
| Without Reqbook (source only) | 1/1 | 96,599 | 33,751 | Source files only |
| With Reqbook surgical context | 1/1 | 53,857 | 17,889 | Surgical context plus validation |

Uncached-token comparison: Reqbook used **47.0% fewer uncached tokens** in this run (`1.89x` without / with Reqbook). Total-token comparison: Reqbook used **44.2% fewer total tokens** in this one-run sample (`1.79x` without / with Reqbook). The Reqbook final answer covered the same endpoint method/path, required fields, validation ranges/enums, business rules, success fields, and documented error cases while inspecting only the Reqbook spec plus validation output. Treat this as a promising fixture result, not a broad public claim, until it is repeated across more tasks and stacks.

Previous rerun attempt after adding `--brief --max-fields 6 --no-guidance`: 2026-06-07T07:38:20.952Z. Result: blocked by Codex usage limit for both scenarios. Artifact: `target/token-benchmarks/codex/2026-06-07T07-38-20-951Z/summary.md`.

### Antigravity Benchmark (Manual)

Captured: 2026-06-08
Agent: Antigravity (Gemini 3.1 Pro Low)
Fixture: `examples/agent-token-api`
Artifact: `benchmarks/token/antigravity/summary.md`

| Scenario | Mode | Estimated Tokens | Notes |
| --- | --- | ---: | --- |
| Without Reqbook (source only) | Manual | ~25,000 | Grep source, tracing routes, models, validators, controllers, and services. |
| With Reqbook surgical context | Manual | ~500 | `rqb context --mode surgical` outputs all necessary info in <300 words. |

Uncached-token comparison: Reqbook reduces token usage by **>95%** for Antigravity in this manual benchmark. The agent can answer immediately from a single CLI command output instead of iteratively reading backend Rust/Node code. This drastically improves speed, reduces hallucination risks, and lowers token costs.

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
