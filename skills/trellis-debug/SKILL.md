---
name: trellis-debug
description: Use this skill to trace, test, and debug API calls using Trellis specs. Triggers on phrases like "test this endpoint", "why is GET /users returning 404", "debug the onboarding flow", "response doesn't match", "run this spec", "verify the API", or when an API call is failing and a Trellis spec exists for it.
---

# Trellis debug

Use this skill to trace and debug API calls through Trellis specs. Covers single endpoint
execution, pipeline tracing, spec validation, and response diff analysis.

## Decision tree

```
Single endpoint issue?  → Step 1 (exec)
Pipeline/flow failure?  → Step 2 (flow trace)
Spec invalid?           → Step 3 (validate)
```

---

## Step 1 — Debug a single endpoint

### Locate the spec

```bash
trellis_list_specs   # via MCP
# or
rg -n "^method:\|^path:" api-docs/apis/ | grep -i "GET\|/users"
rg --files api-docs/apis/ | grep -i "users\|get-user"
```

Match by method + path, filename, or title. If no spec exists, use `trellis-sync` first.

### Validate the spec before running

```bash
trellis validate api-docs/apis/<resource>/<file>.md
```

Exit 2 = invalid spec structure. Fix markdown, frontmatter, or the http block before continuing.

### Dry-run to inspect what's being sent

```bash
trellis exec <file> --env=dev --dry-run
```

Check: correct baseUrl, auth header present, path params substituted, body shape correct.

### Execute

```bash
trellis exec <file> --env=dev --var id=usr_123
```

Via MCP:
```json
{ "tool": "trellis_exec", "arguments": { "spec_path": "<file>", "env": "dev", "vars": { "id": "usr_123" } } }
```

### Interpret the result

| Exit code | Meaning | Check |
|---|---|---|
| 0 | Passed | — |
| 2 | Invalid spec | Fix frontmatter, http block structure |
| 3 | Engine error | Check protocol, request block, expected response block |
| 4 | Network error | Check baseUrl, DNS, VPN, service running |
| 5 | Secret detected | Move value to `.env.local` or `TRELLIS_*` env var |

**Response mismatch:** report `expected vs actual` status code, headers, and body diff. Check whether the spec's `## Expected response` is outdated — offer to update it if the API is intentionally different.

**Missing variables:** check `api-docs/_shared/env.md` for `baseUrl`. Check `.env.local` for auth tokens. Run `--dry-run` to see which placeholders weren't resolved.

**Auth failures (401/403):** check `authToken` is set in env, auth header matches the spec's `auth:` frontmatter field.

---

## Step 2 — Trace a pipeline failure

### Locate the pipeline

```bash
rg --files api-docs/flows/
rg -n "^name:" api-docs/flows/
```

### Read the pipeline before running

Check: step order, `Capture` expressions, `Inject` variable names, `Assert` conditions. Verify all referenced endpoint files exist.

### Execute

```bash
trellis flow api-docs/flows/<pipeline>.md --env=dev
```

Via MCP:
```json
{ "tool": "trellis_flow", "arguments": { "pipeline_path": "<file>", "env": "dev" } }
```

### Trace the failure

Report for each step:
- Endpoint file and status code
- Captured values (mask secrets: `authToken=****`)
- Whether the step matched its Assert

**First failing step:** identify it, then debug it as a single endpoint (Step 1) with the captured inputs from previous steps injected as `--var`.

**Capture mismatch:** if a capture expression doesn't resolve (e.g. `response.body.id` but actual response has `response.body.userId`), update the capture expression to match the real response shape.

**Inject not resolved:** the variable wasn't captured by a previous step — check the capture source step succeeded and the capture name matches the inject name exactly.

---

## Step 3 — Validate specs

Single file:
```bash
trellis validate api-docs/apis/<resource>/<file>.md
```

Full project:
```bash
trellis validate api-docs/
```

Via MCP:
```json
{ "tool": "trellis_validate", "arguments": { "path": "api-docs/" } }
```

Common errors and fixes:

| Error | Fix |
|---|---|
| Missing `method:` in frontmatter | Add `method: GET` (or POST, etc.) |
| No `## Request` section | Add the section with an `http` code block |
| No http block in `## Request` | Wrap request in ` ```http ``` ` fence |
| Invalid auth value | Use `none`, `bearer`, `basic`, or `custom` |
| Secret detected | Move value to `.env.local`, use `{{variable}}` placeholder |

---

## Operating rules

- Never print raw auth tokens. Trellis masks them; do not re-print captured values.
- Confirm before executing against `prod`. Use `--env=staging` or `--env=dev` by default.
- When a response doesn't match, check whether the spec is outdated before assuming the API is broken.
- Prefer `--dry-run` when variables are missing or the endpoint is destructive (DELETE, payment).
