---
name: mad
description: Apply when working with api-docs/ files, authoring or running specs, building flows, or debugging API calls. Covers the full MarkApiDown workflow.
---

# MarkApiDown

Local-first API workspace — specs, environments, and pipelines are plain markdown files in `api-docs/`. The CLI validates and executes them; the browser UI (`mad serve`) renders them interactively.

## Project layout

```
api-docs/
├── mad.md                  # project config (name, default-env, timeouts)
├── _shared/env.md          # base URLs and variables per environment
├── apis/<resource>/<method>-<slug>.md   # one file per endpoint
└── flows/<name>.md         # multi-step pipelines
```

## What to do

| Situation | Action |
|---|---|
| Create or update specs | `/mad` |
| Debug a failing endpoint or pipeline | `/mad-debug` |
| Execute a spec | `mad_exec` MCP tool |
| Run a pipeline | `mad_flow` MCP tool |

## Endpoint format

Required frontmatter: `resource`, `protocol: http`, `method`, `path` (`:param` for path params), `version: 1`.

Sections in order: `## Request` → `## Expected response` → `## Error responses` (optional, reference only) → `## Assertions` (optional) → `## Tests` (optional) → `## Notes` (optional).

````markdown
---
resource: users
protocol: http
method: GET
path: /users/:id
version: 1
env: [dev]
auth: none
---
# Get user by id

## Request
```http
GET {{baseUrl}}/users/:id
Accept: application/json
```

## Expected response
```http
HTTP/1.1 200 OK
Content-Type: application/json

{"id": 1, "name": "Ada Lovelace"}
```

## Assertions
- status: 200
- body.id: exists
````

## Assertions operators

`status: 200` · `body.id: exists` · `body.role: in [admin, user]` · `headers.content-type: contains json` · `body.slug: matches ^[a-z]+$`

## Pipeline capture patterns

`response.body.id` · `response.body[0].id` · `response.body.data.token` · `response.headers.Location`

## MCP tools

| Tool | Use for |
|---|---|
| `mad_exec` | Run one spec |
| `mad_flow` | Run a pipeline |
| `mad_author` | Create or update a spec (validates before writing — prefer over direct file writes) |
| `mad_search` | Find specs by method, path, or tag |
| `mad_vars` | Show variable resolution for a spec |
| `mad_exec_batch` | Run multiple specs in one call |

## Rules

- Variables: `{{name}}` resolved from `_shared/env.md` → `.env.local` → `MAD_*` env vars.
- Secrets never in markdown — use `.env.local` or `MAD_*`.
- Use `mad_author` not direct file writes.
- After writing specs: `mad index`.
- Default to `--env=dev`. Confirm before `--env=prod`.
