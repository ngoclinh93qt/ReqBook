---
description: Create specs, enrich expected responses, or build flows. Usage: /mad [scan|enrich|flow] [target]
---

Route based on $ARGUMENTS:

| $ARGUMENTS | Mode |
|---|---|
| empty, "scan", or a directory | Scan routes and create missing specs |
| "enrich", a .md file, or a directory | Enrich expected responses and tests |
| "flow" or a flow description | Build or run a pipeline |

---

## Scan — create specs from routes

**Step 1 — Auto-import first:**
```bash
mad import project ${ARGUMENTS:-.}
```
If `✓ Found OpenAPI spec` → skip to post-scan. If stubs created → continue.

If the CLI suggests an export command (FastAPI, Spring Boot, etc.), run it then `mad import openapi <file>`.

**Step 2 — Read routes from source:**
```bash
rg -l "@GetMapping\|@RestController" src/       # Spring Boot
rg -l "@app\.get\|@router\." .                  # FastAPI / Flask
rg -l "router\.get\|app\.post" src/             # Express / NestJS
rg -l "r\.GET\|r\.POST" .                       # Gin / Echo
```
For each route: read the handler and type definitions. Use real field names and realistic example values — never `"string"` or `1`.

**Step 3 — Author with `mad_author`:**

Every spec must have a realistic `## Request` body, `## Expected response` with real field values, and `## Tests` covering at least a happy path and one error case.

**Post-scan:** `mad validate api-docs/` then `mad index`.

---

## Enrich — fill in expected responses and tests

For each target spec:
1. Read the spec and its route handler.
2. Run `mad_exec` with `infer_expected: true` if the server is up — use the live response.
3. Update `## Expected response` with real field values (not stubs).
4. Add `## Assertions`: at minimum `status` and one `body.<key>: exists`.
5. Add `## Tests`: happy path + auth failure + one validation error.

If the response shape is unknown and no server is running, ask the user for a concrete example instead of guessing.

---

## Flow — build or run a pipeline

**To build:** reason about data dependencies before writing.
```
POST /login         → capture: authToken
POST /orders        → inject: authToken  →  capture: orderId
GET  /orders/:id    → inject: authToken, orderId  →  assert: status 200, items non-empty
```
Write to `api-docs/flows/<name>.md`. Then: `mad validate api-docs/flows/<name>.md` and `mad index`.

**To run:**
```json
{ "tool": "mad_flow", "pipeline_path": "api-docs/flows/<name>.md", "env": "dev" }
```
Report per step: status, captured values (mask secrets with `****`), assertion result, and first failure with diagnosis.
