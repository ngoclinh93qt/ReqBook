---
name: trellis-sync
description: Use this skill to keep Trellis api-docs/ specs in sync with source code. Triggers when routes are added or changed, when specs are missing, when importing a project for the first time, or on phrases like "sync trellis", "document my routes", "my endpoint is not in trellis", "import this project". Handles init, import, and manual spec authoring in one decision tree.
---

# Trellis sync

Use this skill to keep `api-docs/` aligned with the actual source code. It covers first-time
setup (init), automatic route discovery (import), and manual spec authoring (author) as a single
decision tree — pick the right step based on what's missing.

## Decision tree

```
api-docs/ missing? → init first (Step 1)
api-docs/ exists?  → import (Step 2) → enrich if partial (Step 3) → author if scan fails (Step 4)
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

Verify: check `api-docs/trellis.md` name and `api-docs/_shared/env.md` baseUrl. Fix wrong values by editing directly. Then validate: `trellis validate api-docs/`.

---

## Step 2 — Import (primary sync method)

```bash
trellis import project ${ARGUMENTS:-.}
```

Read output to determine strategy:

| Output | Strategy | Quality | Next |
|---|---|---|---|
| `✓ Found OpenAPI spec:` | Static spec file | Full | Step 5 |
| `✓ Fetched live spec from` | Running server | Full | Step 5 |
| `⚠ No OpenAPI spec found` | Static scan only | Partial | Step 3 |
| `no routes found` | Nothing detected | None | Step 4 |

### If CLI printed a Tip: export command

Offer to run it:
- FastAPI: `python -c "import json; from main import app; print(json.dumps(app.openapi()))" > openapi.json`
- Django REST: `python manage.py spectacular --file openapi.yaml`
- Spring Boot: `mvn springdoc-openapi:generate`
- Gin: `swag init` → `trellis import openapi docs/swagger.json`
- Laravel: `php artisan l5-swagger:generate`

If export succeeds: `trellis import openapi openapi.json`

---

## Step 3 — Enrich partial specs

For each spec with empty body or `{}` response:

### Find the handler

```bash
rg -n "'/users/:id'" src/
rg -n "@GetMapping.*users" src/
rg -n "router\.get.*users" src/
```

### Extract params and body

| Framework | Where to look |
|---|---|
| FastAPI | Function params with type annotations; `response_model=`; `responses={404:...}` |
| NestJS | `@Param()`, `@Query()`, `@Body() dto: CreateDto` → read DTO class |
| Spring Boot | `@PathVariable`, `@RequestParam`, `@RequestBody CreateRequest` → read class fields |
| Express | Zod/Joi schema near the route; `req.params`, `req.body` |
| Gin | `c.ShouldBindJSON(&req)` → read struct json tags; `c.JSON(200, resp)` → read resp struct |
| DRF | `serializer_class` on ViewSet; generates full CRUD |
| Rails | `params.require(:x).permit(...)` strong params |

### Rewrite the spec section

Update `## Request` and `## Expected response` with real field names. Use `{{variable}}` for environment-specific values. Validate after each file:

```bash
trellis validate api-docs/apis/<resource>/<file>.md
```

---

## Step 4 — Full agent scan (no routes detected)

When the CLI finds nothing, scan manually.

```bash
rg --files src/ | rg -i "route\|controller\|handler\|view\|api"
rg -l "@GetMapping\|@PostMapping" src/       # Spring Boot
rg -l "@app\.get\|@router\.get" .            # FastAPI/Flask
rg -l "router\.get\|app\.get" src/           # Express
rg -l "r\.GET\|r\.POST" .                    # Gin/Echo
```

For each route, collect: method, path, path params, query params, body shape, response shape, auth. Create spec files via MCP `trellis_author` (validates before writing). Validate each file.

---

## Step 5 — Post-sync

```bash
trellis index
```

Report: routes found, specs created, specs skipped (already existed), strategy used. Offer:
- `trellis validate api-docs/` — verify all specs
- `trellis serve` — browse in web UI

## Operating rules

- Never overwrite existing spec files.
- Never inline secrets. Use `{{variable}}` placeholders.
- Always validate after writing: `trellis validate <file>`.
- One spec file per `(method, path)` pair.
