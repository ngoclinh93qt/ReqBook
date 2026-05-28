---
description: Scan the project for API routes and create complete Trellis specs — uses LLM understanding of framework, types, and domain to produce real field names and examples, not stubs
---
Scan $ARGUMENTS (default: current directory) for API route definitions and generate complete Trellis endpoint specs.

This is an intelligent scan, not a mechanical one. Use your understanding of the framework, the domain model, and the request/response types to produce specs with real field names, realistic example values, and meaningful tests from the start.

---

### Step 1 — Try automatic discovery first

```bash
trellis import project ${ARGUMENTS:-.}
```

| CLI output | What happened | Next step |
|---|---|---|
| `✓ Found OpenAPI spec:` or `✓ Fetched live spec from` | Full import succeeded | Jump to Step 4 |
| `⚠ No OpenAPI spec found` + routes listed | Stubs created (empty `{}`) | Continue to Step 2 to enrich them |
| `no routes found` | Nothing detected | Continue to Step 2 for a full agent scan |

**If the CLI printed a Tip: export command**, offer to run it for a richer import:
- FastAPI: `python -c "import json; from main import app; print(json.dumps(app.openapi()))" > openapi.json`
- Django REST: `python manage.py spectacular --file openapi.yaml`
- Spring Boot: `mvn springdoc-openapi:generate`
- Gin: `swag init` → `trellis import openapi docs/swagger.json`

If export succeeds: `trellis import openapi openapi.json` → jump to Step 4.

---

### Step 2 — Read source intelligently

Find route files across common patterns:
```bash
rg --files src/ | rg -i "route\|controller\|handler\|view\|api"
rg -l "@GetMapping\|@PostMapping\|@RestController" src/  # Spring Boot
rg -l "@app\.get\|@router\." .                           # FastAPI / Flask
rg -l "router\.get\|app\.get\|app\.post" src/            # Express / NestJS
rg -l "r\.GET\|r\.POST\|r\.PUT" .                        # Gin / Echo
```

For each route, extract:
- Method, path, path params, query params
- Request body type → read the type/struct/class definition for real field names
- Response type → read it, understand what each field means
- Auth requirements (middleware, annotations, guards)
- Business context from names and comments

**Reading types for realistic examples:**

| Framework | Where to find request body | Where to find response shape |
|---|---|---|
| FastAPI | `body: CreateUserRequest` annotation → read class | `response_model=UserResponse` → read class |
| NestJS | `@Body() dto: CreateDto` → read DTO | return type annotation → read interface |
| Spring Boot | `@RequestBody CreateRequest` → read class fields | method return type → read class |
| Express | Zod/Joi schema near the handler | `res.json(result)` → read what `result` is typed as |
| Gin | `c.ShouldBindJSON(&req)` → read struct json tags | `c.JSON(200, resp)` → read resp struct |
| DRF | `serializer_class` → read serializer fields | ViewSet return queryset type |
| Rails | `params.require(:x).permit(...)` | jbuilder / serializer file |

**Infer realistic example values from field semantics — never use generic `"string"` or `1`:**

| Field pattern | Example value |
|---|---|
| `email`, `user_email` | `"user@example.com"` |
| `created_at`, `updated_at`, `timestamp` | `"2024-01-15T10:30:00Z"` |
| UUID `id` field | `"550e8400-e29b-41d4-a716-446655440000"` |
| Integer ID | `1` but contextual (user→`42`, order→`1001`) |
| Prefixed ID (`usr_`, `ord_`) | `"usr_550e8400"` matching the prefix |
| `name`, `full_name` | `"Ada Lovelace"` |
| `status` enum | use the first or most common enum variant |
| `price`, `amount` | `"29.99"` |
| `count`, `total`, `page` | `1` |
| `url`, `avatar_url` | `"https://example.com/avatar.png"` |
| `slug` | `"my-resource-slug"` |

---

### Step 3 — Write complete specs

For each discovered endpoint, create a spec with real content:

Use `trellis_author` MCP tool (validates before writing, refuses to overwrite existing specs).

Every spec must include:
- Frontmatter with correct `resource`, `method`, `path`, `auth`, `version: 1`
- `## Request` block with real headers and body shape (not `{}`)
- `## Expected response` with realistic field values (not `{}` stubs)
- `## Tests` with at minimum: happy path, auth failure, validation error

After writing each file: `trellis validate <file>`

---

### Step 4 — Post-scan

```bash
trellis index
```

Report: framework detected, strategy used, routes found, specs created, specs skipped (already existed).
If any spec was still written as a stub (fields genuinely unknown), say exactly which ones and why.
Offer to run `/trellis-enrich` to fill those stubs with real data.
