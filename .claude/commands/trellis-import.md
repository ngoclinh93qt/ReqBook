---
description: Scan the current project source code for API routes and import them as Trellis specs
---
Scan the project for API route definitions and generate Trellis endpoint specs.
Follow the trellis-import skill decision tree exactly.

### Step 1 — Run the CLI importer

```bash
trellis import project ${ARGUMENTS:-.}
```

Read the output to determine which strategy ran:
- `✓ Found OpenAPI spec:` → full import (proceed to post-import).
- `✓ Fetched live spec from` → full import via running server (proceed to post-import).
- `⚠ No OpenAPI spec found` → static scan only → run Enrichment Protocol.
- `no routes found` → nothing detected → run Full Agent Scan.

### Step 2 — If CLI printed a Tip: export command

Offer to run it:
- FastAPI: `python -c "import json; from main import app; print(json.dumps(app.openapi()))" > openapi.json`
- Django REST: `python manage.py spectacular --file openapi.yaml`
- Spring Boot: `mvn springdoc-openapi:generate`
- Gin (Go): `swag init` → `trellis import openapi docs/swagger.json`
- Laravel: `php artisan l5-swagger:generate`

If export succeeds: `trellis import openapi openapi.json`

### Step 3 — Enrichment Protocol (static scan found routes but specs are incomplete)

For each spec with empty body/response:
1. Find the handler in source: `rg -n "path-fragment\|handler-name" src/`
2. Extract path params, query params, request body type, response type.
3. Read the body/response class/struct/model to get real field names and types.
4. Rewrite `## Request` with real body and `## Expected response` with real schema.
5. `trellis validate <file>` after each update.

Framework lookup:
- Spring Boot: `@PathVariable`, `@RequestParam`, `@RequestBody CreateUserRequest` → read class
- FastAPI: function signature types, `response_model=`, `responses={404: ...}`
- NestJS: `@Param()`, `@Query()`, `@Body() dto: CreateDto` → read DTO class
- Express: look for Zod/Joi schema near the route handler
- DRF: `serializer_class`, ViewSet queryset → generates CRUD routes automatically
- Gin: `ShouldBindJSON(&req)` → read struct json tags; `c.JSON(200, resp)` → read resp struct

### Step 4 — Full Agent Scan (no routes detected)

1. Find route files: `rg --files src/ | rg -i "route\|controller\|handler\|view\|api"`
2. For each route extract: method, path, path params, query params, body, response, auth.
3. Write spec files using the trellis-author format. Validate each one.

### Post-import

```bash
trellis index
```

Report routes found, specs created, strategy used.
Offer: set `baseUrl` in `api-docs/_shared/env.md`, run `trellis validate api-docs/`, open `trellis serve`.
