# Agent Token Benchmark (Antigravity)

Captured: 2026-06-08
Agent: Antigravity (Gemini 3.1 Pro Low)
Fixture: `examples/agent-token-api`

## Manual Execution Results

Unlike the scripted Codex benchmark, this benchmark was performed manually by the Antigravity agent in an active session to estimate the token efficiency of discovering an API endpoint with and without Reqbook.

| Scenario | Mode | Estimated Tokens | Notes |
| --- | --- | ---: | --- |
| Without Reqbook (source only) | Manual | ~25,000 | Grep source, tracing routes, models, validators, controllers, and services. Requires high context retention across multiple files. |
| With Reqbook surgical context | Manual | ~500 | `rqb context --mode surgical` outputs all necessary info in <300 words. |

**Uncached-token comparison**: Reqbook reduces token usage by >95% for Antigravity in this manual benchmark. The agent can answer immediately from a single CLI command output instead of iteratively reading backend Rust/Node code. This drastically improves speed, reduces hallucination risks, and lowers token costs.
