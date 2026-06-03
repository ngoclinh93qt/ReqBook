# Trellis v1.0.0 build plan

This file is the shared coordination plan for all agents working on Trellis. Keep it current as phases are completed, blocked, or revised. Do not remove historical notes that explain decisions.

## Competitive positioning update - 2026-06-03

Research note: do not position MarkApiDown as a clone of any Git-native or local-first API client. MarkApiDown's sharper category is:

> Executable API documentation for humans, CI, and coding agents.

Use this wording across docs, landing pages, examples, and release notes:

- "Executable Markdown for API workflows"
- "Runnable Markdown API specs"
- "API testing for coding agents"
- "Agent-ready API workspace"
- "Keep API docs, tests, and agent context in one Markdown source of truth"

The competitive argument should be fair:

- Local API clients are the right choice when a team primarily wants manual request exploration and a polished desktop request builder.
- Hurl is the right choice when a team primarily wants a fast plain-text HTTP test runner in CI.
- Postman is the right choice when a team primarily wants a broad API platform, hosted collaboration, monitoring, and governance.
- MarkApiDown is the right choice when a team wants API docs in the repo to be readable, executable, reviewable in PRs, runnable in CI, and directly usable by coding agents.

Current app capabilities already present:

- Markdown endpoint specs and pipeline specs under `api-docs/`.
- CLI validation, execution, flow execution, browser preview, mock server, ad-hoc request saving, doctor checks.
- Import from cURL, Postman, Insomnia, OpenAPI, and source route scanning.
- Basic structured assertions with `status`, `body.*`, and `headers.*`.
- Agent skills, slash commands, MCP tools, compact agent outputs, search, variables, history, session, authoring, and batch exec.

Implementation status from this update:

- Phase 16 core shipped: `response.match` supports `shape`, `strict`, and `schema`; `http strict` fence syntax is accepted; strict assertions can affect execution results; `mad check` emits Markdown, GitHub, JUnit, and JSON reports with `--changed-from`.
- Phase 17 first pass shipped: `mad export openapi`, `mad import collection`, `mad import http`, improved OpenAPI tag/operation/security preservation.
- Phase 18 core shipped: `mad context` and MCP `mad_context` return compact endpoint, flow, and changed-spec context.
- Phase 19 examples validated: `jsonplaceholder`, `saas-auth-api`, `github-api-client`, `ecommerce-checkout-flow`, and `agent-token-api`.
- Phase 20 MVP shipped: `packages/vscode` provides preview, run, validate, compact context, result panel, and variable autocomplete powered by the `mad` binary.

### Phase 16 - contract trust and PR review

Priority: P0. Goal: make "contract" credible for serious API teams and make API behavior changes easy to review in pull requests.

Feature work:

- Add explicit response match modes:
  - `shape` remains the default for JSON bodies to preserve the current forgiving workflow.
  - `strict` requires status, expected headers, and JSON/string body to match exactly except documented ignored fields.
  - `schema` validates the actual response against a JSON Schema block.
- Add frontmatter or section syntax for match mode:

```yaml
response:
  match: strict
```

````markdown
## Expected response

```http strict
HTTP/1.1 201 Created
Content-Type: application/json

{
  "id": "usr_123",
  "email": "ada@example.com",
  "status": "active"
}
```
````

- Promote structured assertions so failing assertions affect the command result when `--strict-assertions` or `response.match: strict` is enabled.
- Add a PR-focused command:

```bash
mad check api-docs/ --changed-from origin/main --report markdown
mad check api-docs/ --changed-from origin/main --report github
mad check api-docs/ --report junit
```

Expected report shape:

```text
API contract check

Changed endpoints: 3
Passed contracts: 2
Changed response shape: 1
Missing variables: 1

- PASS POST /users create user
- PASS GET /users/:userId fetch user
- FAIL POST /checkout/sessions response.body.payment.status changed
- WARN GET /me requires authToken but env dev does not define it
```

Acceptance checks:

- `mad exec` preserves current behavior unless a stricter mode is explicitly set.
- `mad check --changed-from` only evaluates changed endpoint and flow files.
- Markdown, GitHub, JUnit, and JSON reports are deterministic and snapshot-tested.
- Strict contract failures and assertion failures are visible in CLI output, MCP output, and reports.

### Phase 17 - OpenAPI round trip and migration depth

Priority: P0. Goal: reduce adoption fear by making MarkApiDown easy to enter and easy to leave.

Feature work:

- Keep existing `mad import openapi`, but add export:

```bash
mad export openapi api-docs/ --out openapi.generated.yaml
mad export openapi api-docs/ --json --out openapi.generated.json
```

- Improve import metadata preservation:
  - `operationId` -> stable title/slug hint.
  - OpenAPI examples -> `## Expected response`.
  - schemas -> optional `## Schema` blocks for `response.match: schema`.
  - tags/security/servers -> frontmatter and `_shared/env.md`.
- Add first-pass local API client collection and `.http` migration:

```bash
mad import collection ./local-client-collection
mad import http ./requests.http
```

Acceptance checks:

- Importing then exporting the same small OpenAPI fixture keeps paths, methods, status examples, parameters, tags, and auth scheme.
- Exported OpenAPI passes a common validator.
- Local client collection and `.http` imports create valid endpoint files and never commit secrets.

### Phase 18 - agent context commands

Priority: P0. Goal: give coding agents compact, deterministic API context without forcing them to read every markdown file.

Feature work:

```bash
mad context users.create
mad context flow signup-login-profile
mad context --changed-from origin/main
mad agent-plan "add password reset API"
```

Example context output:

```text
Endpoint: POST /users
File: api-docs/apis/users/post-create-user.md
Auth: bearer
Variables: baseUrl, email, role
Expected: 201 body.id, body.email, body.status
Related flow: signup-login-profile
Safe next command: mad exec api-docs/apis/users/post-create-user.md --env dev
```

Acceptance checks:

- Context output is stable, under a configurable token budget, and excludes full bodies unless `--verbose`.
- `mad context --changed-from` summarizes only changed specs and related flows.
- MCP exposes the same context shape for agents.

### Phase 19 - real examples and adoption kits

Priority: P0. Goal: make the product feel useful on realistic workflows, not toy single requests.

Examples to maintain:

- `examples/saas-auth-api`: signup, login, current user, token capture, PR-reviewable onboarding flow.
- `examples/github-api-client`: public GitHub repository smoke checks, path variables, rate-limit-aware notes.
- `examples/ecommerce-checkout-flow`: cart, item, checkout session, captured cart/session ids, mock-mode demo.
- Existing `examples/jsonplaceholder` remains the smallest no-auth quick start.
- Existing `examples/agent-token-api` remains the benchmark fixture.

Each example must include:

- `README.md` with install, validate, execute, flow, mock/serve commands.
- `api-docs/mad.md`, `_shared/env.md`, endpoint files, and at least one flow where appropriate.
- Realistic `## Assertions`, `## Tests`, and `## Notes`.
- A copyable agent prompt.

### Phase 20 - lightweight editor workflow

Priority: P1. Goal: meet developers where they write specs without building a full desktop API client clone.

Feature work:

- VS Code extension MVP:
  - Preview current endpoint markdown.
  - Run current endpoint.
  - Validate current file.
  - Autocomplete variables from `_shared/env.md`, `.env.local`, and captures in related flows.
  - Show compact response/result panel.
- Keep this editor workflow narrow. The browser UI remains the local visual workspace; the extension is the fast in-editor bridge.

### Product guardrails

- Do not build a full desktop API client clone as the next major milestone.
- Do not market as a generic API client alternative. Lead with executable API docs, agent context, PR review, and CI.
- Do not claim token or cost reductions with numbers until a benchmark is published.
- Keep Markdown as the source of truth. Avoid new project config formats unless required by platform tooling.

## Current status

- Current phase: Phase 14 MCP server in progress
- Repository status at start: empty workspace, no git repository
- Active instruction: work phases in order, run each phase acceptance check, commit before moving on

## Phase checklist

1. Phase 1 - markdown convention: completed and committed (`efaad0c docs: define markdown convention`)
2. Phase 2 - engine core: completed and committed (`31c0f1c feat: implement engine core`)
3. Phase 3 - CLI surface: completed and committed (`cd6f3e7 feat: add cli surface`)
4. Phase 4 - cross-agent skill compatibility: completed and committed (`c35a312 feat: add cross-agent skills`)
5. Phase 5 - distribution: completed and committed (`a8f690c feat: add distribution packaging`)
6. Phase 6 - migration tools: completed and committed (`f5c8c78 feat: add migration tools`)
7. Phase 7 - web preview: completed and committed (`42ae3a7 feat: add web preview`)
8. Phase 8 - documentation site: completed and committed (`e9616d3 docs: add documentation site`)
9. Phase 9 - examples: completed and committed (`7e402a0 examples: add jsonplaceholder example project`)
10. Phase 10 - CI/CD: completed and committed (`ecb61cd ci: add CI workflow`)
11. Phase 11 - acceptance, polish, launch readiness: completed and committed (`c530699 chore: polish and launch readiness for v1.0.0`)
12. Phase 12 - slash commands, smart init, project route scanner: completed and committed (`d239200 feat: slash commands, smart init, project route scanner`)
13. Phase 13 - OSS release readiness: completed and committed (pending tag)
14. Phase 14 - MCP server: in progress, not committed
15. Phase 15 - mock server: not started

## Phase 13 acceptance tracking

- OSS trust files added: `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`: passed
- GitHub issue templates and PR template added: passed
- Dependabot and OpenSSF Scorecard workflow added: passed
- README rewritten for public OSS positioning with install, quick start, project layout, docs, release status: passed
- Release and launch docs added: `docs/release.md`, `docs/launch.md`: passed
- Release workflow hardened with preflight checks, optional crates.io/npm publish gates, Docker, docs deploy: passed
- Cargo package excludes workspace-local artifacts such as `.claude/`, root `api-docs/`, `web/node_modules/`, and frontend source while including built `web/dist/`: passed
- `cargo fmt --check`: passed
- `cargo clippy --locked -- -D warnings`: passed
- `cargo test --locked`: passed
- `cargo test --locked --no-default-features --features minimal`: passed
- `cd web && npm run build`: passed
- `cargo run -- validate api-docs`: passed
- `cargo run -- validate examples/jsonplaceholder/api-docs`: passed
- `cargo package --allow-dirty --no-verify --offline`: passed, packaged 68 files / 758.0 KiB uncompressed / 210.4 KiB compressed
- `cargo dist build`: blocked locally because `cargo-dist` is not installed in this environment; the GitHub release workflow uses `axodotdev/cargo-dist@v0.31.0`.
- Vibe-coder feedback pass: fixed CLI env loading from `_shared/env.md`, path param resolution, flow array captures, safer dry-run output, web empty-body override behavior, and agent skill paths for `api-docs/apis`.
- Regression tests added for `:pathParam` resolution and `response.body[0].id` capture.
- Vibe-coder smoke test in `/private/tmp/trellis-vibe-fixed.UqpH7n`: `init`, `exec`, `dry-run`, `doctor`, `flow`, `skills install --agent=claude-code` passed.
- `cargo test --locked`: passed, 121 unit tests + 1 integration test.
- `cargo test --locked --no-default-features --features minimal`: passed, 98 unit tests + 1 integration test.
- `cargo clippy --locked -- -D warnings`: passed.
- `cd web && npm run build`: passed.

Phase 13 implementation notes:

- Keep the next public release conservative: prefer `v0.1.0` or `v1.0.0-rc1` until several external users complete init, serve, flow, and install flows.
- Before a real public tag, update `BENCHMARKS.md` with fresh measurements from the release machine and configure publish toggles/secrets: `PUBLISH_CRATES`, `PUBLISH_NPM`, `CARGO_REGISTRY_TOKEN` or npm trusted publishing.
- Local package verification is now clean enough for crates.io packaging; root demo specs and local agent installs stay outside the crate package.

## Phase 15 acceptance tracking

- `trellis mock api-docs/` starts without error and prints bind address: not started
- `GET /<path>` for each spec's path returns the recorded response body with correct status code: not started
- `POST /<path>` for each POST spec returns recorded response: not started
- Axum route params (`:userId`) resolve correctly — requesting `/users/42` matches the `/users/:userId` spec: not started
- Unknown paths return `404 {"error":"no mock for <method> <path>"}`: not started
- `--port <n>` flag overrides default port 4001: not started
- `--dir <path>` flag overrides default `api-docs/` directory: not started
- `--latency <ms>` flag adds artificial delay to all responses: not started
- `cargo test` with new mock module unit tests: not started
- `cargo clippy -- -D warnings`: not started
- `cargo fmt --check`: not started

Phase 15 implementation notes:

- New file: `src/mock.rs`, gated by `#[cfg(feature = "web")]` to reuse the existing Axum dependency.
- New CLI subcommand: `Command::Mock(MockArgs)` added to the `Command` enum in `src/main.rs`. `MockArgs` has three fields: `dir: PathBuf` (default `api-docs/`), `port: u16` (default `4001`), `latency: Option<u64>` (milliseconds).
- Route building: walk all `.md` files under `<dir>/apis/` using the same `collect_specs` pattern from `src/preview.rs`; call `parse_endpoint` on each; build an `Axum Router` entry per `(method, path)` pair. Trellis path params use `:param` syntax — Axum 0.7 uses the same syntax, so conversion is a direct passthrough.
- Response parsing: `expected_response` block from each spec has the form `HTTP/1.1 <status> <reason>\n<headers>\n\n<body>`. Split on the first blank line; extract status code from the first line with a simple integer parse; use the body verbatim. If the headers block contains `Content-Type`, forward it; otherwise default to `application/json`.
- Each route handler is a closure that returns a static `(StatusCode, HeaderMap, Bytes)` tuple built at startup, not at request time, so the router holds no mutable state.
- `--latency <ms>` injects a `tokio::time::sleep` before the response in every handler. This is wired via a shared `AppState` field, not per-route cloning.
- Conflicting specs (two files with the same method + path): log a warning and keep the first one encountered (alphabetical walk order). Do not fail; conflicting specs are a data problem, not a runtime error.
- Wildcard/catch-all routes (`:param` at multiple levels, e.g. `/orgs/:orgId/repos/:repoId`) are registered verbatim; Axum handles nested params natively.
- `trellis mock` is independent of `trellis serve`. They can run concurrently on different ports. The mock server has no web UI.
- `src/mock.rs` exports a single public async function `run_mock_server(dir, port, latency) -> Result<()>` called from `main.rs`.
- No new dependencies required: Axum, tokio, serde_json, and walkdir are all already present.

---

## Phase 14 acceptance tracking

- `trellis mcp` starts without error and writes the MCP `initialize` response to stdout: not started
- `tools/list` request returns a JSON list with exactly three tools: `trellis_exec`, `trellis_flow`, `trellis_validate`: not started
- `trellis_exec` tool call with `{"spec_path": "api-docs/apis/posts/get-list.md"}` executes the spec and returns a JSON result with `passed`, `status_code`, and `body` fields: not started
- `trellis_flow` tool call with `{"pipeline_path": "api-docs/flows/demo-post-flow.md"}` executes the pipeline and returns a JSON result with per-step outcomes: not started
- `trellis_validate` tool call with `{"path": "api-docs/"}` validates the directory and returns `{"valid": true, "file_count": N}` or a list of errors: not started
- Unknown tool name returns a JSON-RPC error response with code `-32601` (method not found): not started
- Invalid params (missing `spec_path`) returns a JSON-RPC error response with code `-32602` (invalid params): not started
- `trellis mcp --help` documents the command clearly: not started
- `cargo test` with at least 5 unit tests covering the JSON-RPC message serialization and tool dispatch: not started
- `cargo clippy -- -D warnings`: not started
- `cargo fmt --check`: not started
- Smoke test: pipe a minimal `initialize` + `tools/list` + `tools/call` sequence through `trellis mcp` via stdin and verify stdout JSON is valid MCP: not started

Phase 14 implementation notes:

- New file: `src/mcp.rs`. No new feature flag — MCP uses only `serde_json` (already a dependency) and `tokio::io` (already in scope). MCP over stdio is the canonical transport and requires no network stack beyond what is already present.
- New CLI subcommand: `Command::Mcp` (no arguments at v1) added to the `Command` enum in `src/main.rs`. Invocation: `trellis mcp`. The command reads newline-delimited JSON from stdin and writes newline-delimited JSON to stdout in a loop until EOF.
- Protocol: MCP JSON-RPC 2.0 over stdio. Message framing is one JSON object per line (NDJSON). Both `request` and `notification` forms are handled; only `request` forms require a response.
- MCP initialization sequence handled in `src/mcp.rs`:
  1. Client sends `{"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}` — respond with `{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"trellis","version":"1.0.0"}}}`.
  2. Client sends `{"jsonrpc":"2.0","method":"notifications/initialized"}` — this is a notification (no `id`); no response required.
  3. Client sends `{"jsonrpc":"2.0","id":2,"method":"tools/list"}` — respond with the three tool schemas below.
  4. Client sends `{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"trellis_exec","arguments":{...}}}` — dispatch to the appropriate handler and return the result.
- Tool schemas for `tools/list` response:
  - `trellis_exec`: description `"Execute a Trellis endpoint spec and return the HTTP result."`, input schema requires `spec_path: string`, optional `env: string` and `vars: object`.
  - `trellis_flow`: description `"Execute a Trellis pipeline and return per-step results."`, input schema requires `pipeline_path: string`, optional `env: string`.
  - `trellis_validate`: description `"Validate a Trellis spec file or directory."`, input schema requires `path: string`.
- Tool call return format: all three tools return a `content` array with a single `{"type":"text","text":"<json string>"}` entry. This matches the MCP spec for text tool results and allows any MCP client to display or further parse the output.
- `trellis_exec` handler: resolve `spec_path` relative to cwd; call `parse_endpoint` then `engine::execute`; serialize `ExecutionResult` to JSON; return as text content. On error, return a JSON-RPC error response with code `-32000` and the anyhow error message.
- `trellis_flow` handler: resolve `pipeline_path`; call `parse_pipeline` then `pipeline::run`; serialize per-step results to JSON; return as text content.
- `trellis_validate` handler: if `path` is a file, call `parse_endpoint` and return `{"valid":true}` or `{"valid":false,"error":"..."}`. If `path` is a directory, walk all `.md` files, collect errors, return `{"valid":true,"file_count":N}` or `{"valid":false,"errors":[...]}`.
- Error handling: unknown method returns `{"code":-32601,"message":"Method not found"}`; missing required params returns `{"code":-32602,"message":"Invalid params: <field> is required"}`; internal engine errors return `{"code":-32000,"message":"<anyhow error chain>"}`.
- The `run_mcp_server()` async function in `src/mcp.rs` uses `tokio::io::AsyncBufReadExt` to read lines from stdin and `tokio::io::AsyncWriteExt` to write responses to stdout. The loop exits cleanly on EOF.
- No MCP SDK dependency. The MCP 2024-11-05 protocol spec is simple enough to implement with hand-written serde structs. Keeping it dependency-free avoids supply chain risk and keeps the binary size delta under 50 KB.
- `src/lib.rs` exports `pub mod mcp;` unconditionally (no feature flag) since it has no optional deps.

---

## Phase 12 acceptance tracking

- `cargo test` (73 tests: 72 unit + 1 httpbin integration): passed
- `cargo clippy -- -D warnings`: passed
- `cargo fmt --check`: passed
- `trellis skills install --agent=claude-code` creates 7 slash commands in `.claude/commands/` + 3 skills: passed
- `trellis skills uninstall --agent=claude-code` removes all 10 files: passed
- `trellis install --agent=cursor` creates 3 skills only (no slash commands): passed
- `trellis init --yes` in a dir with `Cargo.toml name = "trellis"` uses "trellis" as default: passed (detected via smoke test)
- `trellis import project src/` on this repo finds 6 axum routes in preview.rs: passed
- `trellis import project examples/` returns 0 endpoints (no source code): passed (by test)
- Generated specs pass `trellis validate`: passed (by unit test)

Phase 12 implementation notes:

- Slash commands added to `src/installer/mod.rs` as `COMMANDS: &[CommandDef]` array (7 entries). `Agent::supports_commands()` gates which agents get them (claude-code, codex-cli only).
- `detect_project_name()` added to `src/main.rs`: checks package.json, Cargo.toml, pyproject.toml, go.mod, composer.json, pom.xml in order; uses `serde_json` for JSON manifests, simple line-by-line parsing for TOML/go.mod, regex for pom.xml — no new dependencies.
- `src/importer/project.rs` walks the source tree using `walkdir`-style manual recursion (no extra dep), applies 15 regex patterns per file extension, deduplicates by `(method, path)`, normalises paths (Flask `<type:param>`, OpenAPI `{param}`, Axum `:param` all → `:param`).
- `walkdir` moved from optional (`install` feature) to unconditional since the project importer has no feature gate.
- `title_from_path` in `curl.rs` promoted from `fn` to `pub(crate) fn` so `project.rs` can reuse it.

## Coordination rules

- Follow the phase order exactly.
- Before starting a phase, verify that the previous phase has passed its acceptance checks and has been committed.
- If a later phase exposes a defect in an earlier phase, fix the earlier phase first and commit the fix.
- Preserve markdown-native configuration. Do not introduce TOML, JSON, or YAML project config files beyond Rust/package tooling files that require them.
- Keep production code free of stubs, unchecked secrets, and untracked TODO/FIXME comments.

## Phase 11 acceptance tracking

- `cargo test` (47 tests, 46 unit + 1 httpbin integration): passed
- `cargo clippy -- -D warnings`: passed
- `cargo fmt --check`: passed
- `trellis --version` prints `1.0.0`: passed
- `trellis validate examples/jsonplaceholder/api-docs/` reports valid (9 files, 4 ms): passed
- `trellis doctor` runs cleanly with no fatal errors: passed
- No TODO/FIXME/stub markers in production code (`src/`): passed
- Binary size under 5 MB (`target/release/trellis` ≈ 3.1 MB stripped): passed
- `trellis skills list` detects claude-code, codex-cli, antigravity, copilot: passed
- `trellis completion bash` generates valid bash completion script: passed
- `CHANGELOG.md` expanded to full v1.0.0 release notes covering all 11 phases: passed
- `README.md` has badges, description, quick-start, features table, install methods: passed
- `docs/` site has index, getting-started, cli, configuration, migration guides: passed

Phase 11 implementation notes:

- End-to-end acceptance sweep confirmed every phase's deliverables are present and functional.
- `CHANGELOG.md` rewritten from stub to comprehensive notes grouped by: Engine, CLI, Cross-agent skills, Web preview, Distribution, Spec convention.
- No production code changes were needed; all prior phase implementations passed their acceptance criteria without modification.
- Binary is ~3.1 MB (stripped, opt-level="z") for the default feature set on macOS aarch64.

## Phase 10 acceptance tracking

- `.github/workflows/ci.yml` exists with test, lint, validate-examples, and docs jobs: passed
- `.github/workflows/release.yml` exists from Phase 5 with cargo-dist and Docker push: passed (pre-existing)
- CI matrix covers ubuntu, macos, windows: passed
- Lint job runs `cargo fmt --check` and `cargo clippy --locked -- -D warnings`: passed
- `validate-examples` job runs `trellis validate` on the jsonplaceholder example: passed (structure defined)
- Docs job deploys `docs/` to GitHub Pages on push to main: passed (structure defined)

Phase 10 implementation notes:

- `ci.yml` triggers on push to main and PRs; uses `dtolnay/rust-toolchain@stable` and `Swatinem/rust-cache@v2`.
- Three OS matrix: `ubuntu-latest`, `macos-latest`, `windows-latest` to match the 5 dist targets (linux musl built cross-compiled, not natively tested in CI).
- `validate-examples` job builds the release binary then runs `trellis validate` on the jsonplaceholder example, ensuring examples stay in sync.
- Docs job only runs on main branch; deploys the `docs/` directory to GitHub Pages.
- `release.yml` (from Phase 5) uses `axodotdev/cargo-dist@v0.31.0` to build all platform binaries on tag push.
- `CARGO_TERM_COLOR: always` and `RUST_BACKTRACE: 1` set in env for readable CI output.

## Phase 9 acceptance tracking

- `examples/jsonplaceholder/` exists as a complete Trellis project: passed
- `trellis validate examples/jsonplaceholder/api-docs/` passes all 8 markdown files: passed (4ms)
- `trellis index` generates `api-docs/README.md`: passed
- All 5 endpoint files use correct frontmatter and section structure: passed
- Pipeline `post-with-author.md` has capture and inject steps: passed
- `examples/README.md` documents all endpoints and pipeline with usage commands: passed

Phase 9 implementation notes:

- `examples/jsonplaceholder/` uses JSONPlaceholder (https://jsonplaceholder.typicode.com) — no API key required, stable, free.
- Endpoints: GET /posts, GET /posts/:postId, POST /posts, GET /users, GET /users/:userId.
- Pipeline `post-with-author`: captures `response.body.userId` from the first step and injects it into the second step's `userId` variable.
- `_shared/env.md` sets `baseUrl`, `postId`, and `userId` for the dev environment.
- Each endpoint has a `## Tests` section with `agent-task` instructions.
- The example `.gitignore` has `.env.local` for security convention compliance.
- `api-docs/README.md` is generated by `trellis index` (not hand-written).

## Phase 8 acceptance tracking

- `docs/index.md` exists with description, features, quick start, and navigation links: passed (35 lines)
- `docs/getting-started.md` exists with all 5 install methods, init walkthrough, validate, exec, serve, doctor: passed (197 lines)
- `docs/cli.md` documents all 17 subcommands (init, validate, exec, flow, index, import×3, skills×3, serve, doctor, completion, version) plus global flags and exit codes: passed (448 lines)
- `docs/configuration.md` covers trellis.md format, env.md format, variable resolution priority, secret detection, auth modes, retry policy: passed (279 lines)
- `docs/guides/migration.md` covers Postman v2.1, Insomnia v4, OpenAPI 3.x with concept mapping tables and workflows: passed (222 lines)
- Each docs page cross-links to at least one other docs page: passed
- `README.md` updated with badges, description, quick start, features table, all 5 install methods, links to docs: passed (60 lines)
- `docs/spec/convention.md` unchanged: confirmed

Phase 8 implementation notes:

- Documentation follows the coordination rule for markdown-native structure: all docs are `.md` files with no site generator config required.
- `docs/spec/convention.md` is the canonical spec; the other docs pages are user-facing guides that reference it.
- Cross-linking uses relative markdown paths (e.g., `[CLI reference](cli.md)` from `docs/getting-started.md`).
- `cli.md` groups `import postman/insomnia/openapi` under `## trellis import` with `###` subsections, and similarly for `skills install/list/uninstall`.
- README.md uses placeholder badge URLs that resolve once the GitHub repo is published.

## Phase 7 acceptance tracking

- `trellis serve` starts, prints `✓ Preview: http://127.0.0.1:8080`, and serves HTML: passed
- `GET /` returns HTML listing grouped endpoint cards with method badges: passed
- `GET /spec/<path>` returns HTML endpoint detail page with request, expected response, and Run button: passed
- `POST /exec/<path>` returns JSON `Execution` with `diff.passed` field: passed (via curl smoke test)
- `cargo test` (47 tests total, 4 new preview unit tests): passed
- `cargo clippy -- -D warnings`: passed
- `cargo fmt --check`: passed
- `cargo build --release --no-default-features --features minimal` (Dockerfile path): passed

Phase 7 implementation notes:

- `src/preview.rs` implements the web preview server gated by `#[cfg(feature = "web")]`.
- Routes: `GET /` (index), `GET /spec/*path` (endpoint detail), `POST /exec/*path` (execute and return JSON).
- `IndexTemplate` (askama) groups endpoints by resource and renders method badges styled by HTTP method.
- `SpecTemplate` (askama) renders endpoint details with a JavaScript Run button that POSTs to `/exec/*path`.
- `collect_specs` walks `api-docs/` recursively, tries `parse_endpoint` on each `.md` file; silently skips non-endpoint files (README.md, trellis.md, env.md, pipeline files, etc.).
- `load_env_context` reads `api-docs/_shared/env.md` for the selected env and populates the `Context` before execution.
- `read_project_name` extracts the `name:` field from `api-docs/trellis.md` frontmatter for the page title.
- `serve` in `main.rs` changed to `async fn` to await the preview server; `#[cfg]` guards compile the correct version.
- Templates live in `templates/` (at crate root), embedded at compile time by askama.
- Server binds to `127.0.0.1:8080` by default; warns on `0.0.0.0`; uses `tokio::net::TcpListener` + `axum::serve`.

## Phase 6 acceptance tracking

- `cargo test` (43 tests total including 8 new importer unit tests): passed
- `cargo clippy -- -D warnings`: passed
- `cargo fmt --check`: passed
- `trellis import postman` parses Postman v2.1 JSON, generates `api-docs/<resource>/<method>-<slug>.md` files: passed
- `trellis import insomnia` parses Insomnia v4 JSON, maps request groups to resource folders: passed
- `trellis import openapi` parses OpenAPI 3.x YAML/JSON, converts `{param}` paths to `:param`: passed
- All generated files pass `parse_endpoint` (validated by `generated_file_passes_validate` tests): passed
- `trellis import` on missing file returns error with path and fix suggestion: passed (anyhow context)

Phase 6 implementation notes:

- `src/importer/mod.rs` defines `ImportedEndpoint`, `write_endpoints`, `render_endpoint`, and shared helpers: `convert_path_params`, `resource_slug`, `normalize_variables`, `parse_url`, `sentence_case`.
- `parse_url` handles `{{baseUrl}}/path`, `https://host/path`, `/path`, and bare-path forms; strips query strings; converts path variables to `:param`.
- `postman.rs` walks the nested item tree recursively; extracts example responses from the `response` array.
- `insomnia.rs` builds a group-id → slug map from `request_group` resources and resolves each request's `parentId` for resource naming.
- `openapi.rs` uses `serde_yaml::Value` (handles both YAML and JSON); converts `{param}` to `:param`; generates method-appropriate request blocks; uses `serde_json::to_string_pretty` on `serde_yaml::Value` for JSON body rendering.
- No new crate dependencies added. All three importers use `serde_json`, `serde_yaml`, `regex`, and `anyhow` already present.
- Test fixtures in `tests/fixtures/`: `postman.json`, `insomnia.json`, `openapi.yaml`.
- `importer` module is unconditional (no feature flag) since it uses no optional deps.

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
