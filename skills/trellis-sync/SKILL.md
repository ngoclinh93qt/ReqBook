---
name: trellis-sync
description: Use this skill to create or update api-docs/ specs for a project. Triggers on phrases like "sync trellis", "set up trellis", "document my routes", "my endpoint is not in trellis", "import this project", "enrich my specs", when specs are missing or stale, or when routes have changed. Covers first-time init, intelligent route scanning, spec enrichment, and flow creation.
---

# Trellis sync

Use this skill to create or update `api-docs/` specs for a project. The guiding principle: leverage your understanding of the codebase, domain model, and API semantics to produce specs that are correct and complete from the start — not stubs that need fixing later.

## Decision tree

```
api-docs/ missing?        → Init first (Step 1)
Routes missing or stale?  → Scan and author (Step 2)
Expected responses empty? → Enrich with real data (Step 3)
Need a workflow?          → Build a flow (Step 4)
```

---

## Step 1 — Init (first time only)

Run only if `api-docs/trellis.md` does not exist.

### Detect project name

Try in order: `package.json` → `Cargo.toml` → `pyproject.toml` → `go.mod` → `pom.xml` → README first heading → git remote → directory name. If the name looks like a template default (`my-api`, `app`), investigate further.

### Detect base URL

Check `.env`/`.env.local` for `PORT`/`HOST` → `docker-compose.yml` ports → framework defaults:

| Framework | Default URL |
|---|---|
| FastAPI / Django / Flask / Laravel | `http://localhost:8000` |
| Express / NestJS / Rails | `http://localhost:3000` |
| Spring Boot / Axum / Gin / Echo | `http://localhost:8080` |
| ASP.NET Core | `http://localhost:5000` |

If base URL cannot be determined, ask once.

### Run init

```bash
trellis init --name "<name>" --dev-url "<url>" --yes
```

Verify: `api-docs/trellis.md` name and `api-docs/_shared/env.md` baseUrl. Fix wrong values by editing directly.

---

## Step 2 — Scan routes and create specs

### Try automatic discovery first

```bash
trellis import project .
```

| Output | What happened | Next |
|---|---|---|
| `✓ Found OpenAPI spec:` | Full import | Step 5 |
| `✓ Fetched live spec from` | Full import via running server | Step 5 |
| `⚠ No OpenAPI spec found` | Stubs created (empty `{}`) | Step 3 |
| `no routes found` | Nothing detected | Intelligent scan below |

### If CLI finds nothing — read source intelligently

Find route files:
```bash
rg --files src/ | rg -i "route\|controller\|handler\|view\|api"
```

For each route, extract: method, path, path params, query params, request body type, response type, auth. Read the actual type definitions for real field names — never use `field1`, `field2`.

**Framework-specific reading:**

| Framework | Request body | Response type |
|---|---|---|
| FastAPI | `body: CreateUserRequest` → read class fields | `response_model=UserResponse` → read class |
| NestJS | `@Body() dto: CreateDto` → read DTO | return type → read interface |
| Spring Boot | `@RequestBody CreateRequest` → read class | method return type → read class |
| Express | Zod/Joi schema near the handler | `res.json(result)` → read what `result` is |
| Gin | `c.ShouldBindJSON(&req)` → read struct json tags | `c.JSON(200, resp)` → read resp struct |
| DRF | `serializer_class` → read serializer fields | ViewSet return type |
| Rails | `params.require(:x).permit(...)` | jbuilder / serializer file |

Write specs via `trellis_author` MCP tool (validates before writing, refuses to overwrite).

---

## Step 3 — Enrich partial specs

Split enrichment into two explicit modes. During sync, default to expected-response enrichment only. Generate tests only when the user explicitly asks for test cases or scenarios.

### 3A. Expected contracts

For each spec with a stub `{}` response, gather only bounded evidence:

1. Read the spec itself.
2. Search for OpenAPI/schema files already in the repo.
3. Read the matching handler and directly referenced DTO/schema/serializer/model files.
4. Execute `trellis exec <file> --env=dev` only when the API is already running and variables are available.
5. Ask the user for a success/error example if code does not define the shape.

Search by exact method, path fragments, resource name, and type names. Do not scan the whole repo repeatedly. Per spec, read at most 3 likely handlers and only directly referenced type files; stop after 6 source files unless the user asks for deeper analysis.

Use the real response or response type but apply judgment: replace volatile fields (`created_at`, request IDs) with `{{placeholder}}` variables, keep stable structural fields as literals.

Find the handler, read its response type, and write realistic example values:

| Field pattern | Example |
|---|---|
| `email` | `"user@example.com"` |
| `created_at` | `"2024-01-15T10:30:00Z"` |
| UUID `id` | `"550e8400-e29b-41d4-a716-446655440000"` |
| `name` | `"Ada Lovelace"` |
| `status` enum | first or most common variant |
| `price` | `"29.99"` |

Write the primary success case into `## Expected response`.

Also add a `## Error responses` section when the error behavior is defined in code or OpenAPI. Trellis executes only the single `## Expected response` block today, so error responses are reference examples for humans, web preview, and agents.

Prefer one best-evidenced error case:

| Evidence | Error example |
|---|---|
| auth required | `401 Unauthorized` |
| path id lookup | `404 Not Found` |
| validation schema | `400 Bad Request` or `422 Unprocessable Entity` |
| duplicate/unique guard | `409 Conflict` |
| invalid state transition | `409 Conflict` or `422 Unprocessable Entity` |

If code does not define request or response shape, do not invent it. Report the missing evidence and ask for one concrete API example.

Always validate after each file: `trellis validate <file>`

### 3B. Tests on request

Generate `## Tests` only when the user asks for tests, cases, or a scenario. Base tests on the user's stated goal; if the goal is unclear, ask what behavior the API tests should prove.

Keep the block concise and executable by an agent:

1. Scope: the behavior under test.
2. Setup data and variables.
3. Terminal command: `trellis exec` or `trellis flow`.
4. Web path: how to run it from `trellis serve`.
5. LLM/MCP path: which Trellis tool and variables to pass.
6. Assertions: status, headers, and concrete `response.body.<path>` checks.

Do not pretend `agent-task` is executable code. If the user asks for runnable multi-case tests, recommend a future Trellis `## Cases` YAML block plus `trellis test <file> --case <name>`, a web Cases tab, and an MCP `trellis_test` tool.

---

## Step 4 — Build a flow (when multi-step scenario is needed)

Reason about data dependencies: which step produces what the next step needs.

```markdown
---
name: <flow-name>
type: pipeline
---

## Steps

### 1. <Name>
spec: api-docs/apis/<resource>/<file>.md
Capture:
- <var>: response.body.<field>
Assert:
- response.status == <N>

### 2. <Name>
spec: api-docs/apis/<resource>/<file>.md
Inject:
- <var>
Assert:
- response.status == <N>
```

Validate: `trellis validate api-docs/flows/<name>.md`

---

## Step 5 — Post-sync

```bash
trellis index
```

Report: routes found, specs created, specs skipped (already existed), enrichment strategy used.

## Operating rules

- Never overwrite existing spec files (use `overwrite: true` only with explicit user approval).
- Never inline secrets — use `{{variable}}` placeholders.
- Always validate after writing: `trellis validate <file>`.
- One spec file per `(method, path)` pair.
