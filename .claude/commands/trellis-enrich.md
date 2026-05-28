---
description: Enrich Trellis specs in bounded modes: expected response examples or test instructions. Reads targeted evidence, stops when source is missing, and does not generate both unless explicitly asked.
---
Enrich the spec(s) identified by $ARGUMENTS with a controlled workflow.

The goal is quality, not exhaustive exploration. Use LLM judgment only after collecting targeted evidence from the spec, route handler, request schema, response schema, OpenAPI output, or a live dev response. If the shape is not defined in code and no live/example response is available, stop and ask the user for one concrete API example instead of guessing.

---

### Step 0 - Choose mode

Parse $ARGUMENTS before doing any source reading.

| User intent | Mode | What to update |
|---|---|---|
| omitted, `expected`, `response`, `contract` | `expected` | `## Request`, `## Expected response`, optional `## Error responses` |
| `tests`, `test`, `cases`, `scenario` | `tests` | `## Tests` only |
| explicit `all`, `expected and tests` | `all` | Run expected mode first, report, then tests mode |

Default to `expected`. Do not update `## Tests` in expected mode. Do not update expected response sections in tests mode.

If tests mode has no clear test goal, ask one short question such as:

> What behavior should these API tests prove for this endpoint?

---

### Step 1 - Identify target specs

If a specific file is given, use only that file.

If a directory is given, search only within that directory.

If no target is given, find specs that still look incomplete:

```bash
rg --files api-docs/apis
rg -l '^\{\s*\}$|^\[\s*\]$' api-docs/apis
rg -n '^method:|^path:|^resource:' api-docs/apis
```

If a natural language description is given, locate matching specs by method, path, resource, and title:

```bash
rg -n '^method:|^path:|^resource:|^# ' api-docs/apis
```

Work in small batches. If more than 5 specs match, summarize the candidates and ask which group to enrich first.

---

### Step 2 - Gather bounded evidence

For each spec, read evidence in this order and stop as soon as the request and response shapes are known:

1. The spec file itself: frontmatter, `## Request`, existing `## Expected response`, and `## Tests` if in tests mode.
2. OpenAPI/schema files already in the repo: `openapi.*`, `swagger.*`, generated schema files.
3. Route handler and directly referenced DTO/schema/serializer/model files.
4. A live dev response via `trellis exec <file> --env=dev` if the API is already running and variables are available.
5. User-provided success/error examples.

Use targeted searches. Start from the exact method, path fragments, resource name, and request/response type names. Search only source-like directories that exist; if none of these exist, ask the user where the route files live instead of scanning the whole repo.

```bash
rg -n 'GET|POST|PUT|PATCH|DELETE' src app routes api server
rg -n '/users|users|UsersController|user' src app routes api server
rg -n 'response_model|responses=|res\.json|return .*Response|c\.JSON|ResponseEntity|serializer' src app routes api server
rg -n 'z\.object|Joi\.object|ShouldBindJSON|@RequestBody|@Body|permit\(|Request|Dto|Schema' src app routes api server
```

Reading budget per spec:

- Read the spec.
- Read at most 3 likely handler files.
- Read only directly referenced DTO/schema/serializer/model files.
- Stop after 6 source files total unless the user explicitly asks for deeper analysis.

If evidence is missing, report the missing piece and ask for an example:

```text
I could not find the response shape for POST /orders in code. Please provide one success response and one error response example, or point me to the handler/schema file.
```

---

### Step 3A - Expected mode

Expected mode generates contract examples only.

Update `## Request` only when the request body is currently `{}` or incomplete and a request schema is found. Otherwise leave it unchanged.

Update `## Expected response` with the primary success case:

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "id": "usr_550e8400",
  "email": "user@example.com",
  "name": "Ada Lovelace",
  "role": "member",
  "created_at": "2024-01-15T10:30:00Z"
}
```

Rules:

- Status must be exact: `201` for create, `200` for read/update, `204` for delete with no body, unless code/OpenAPI says otherwise.
- Include only contract-relevant headers: usually `Content-Type`, plus `Location` for `201` if code sets it.
- Body must match evidence. Do not invent fields.
- For arrays, include 1 representative item unless the source clearly defines pagination or multiple shapes.
- Replace volatile values with Trellis variables only when they are already part of the project or clearly required: `{{timestamp}}`, `{{requestId}}`, `{{id}}`.
- If the response body is intentionally empty, keep it empty after the blank line.

Also add or update an optional `## Error responses` section with one best-evidenced error case. Trellis currently executes only the single `## Expected response` block, so error responses are reference examples for humans, web preview, and agents.

Prefer the error case that is defined in code or OpenAPI:

| Evidence | Error example |
|---|---|
| auth required | `401 Unauthorized` |
| path id lookup | `404 Not Found` |
| validation schema | `400 Bad Request` or `422 Unprocessable Entity` |
| unique constraint / duplicate check | `409 Conflict` |
| state transition guard | `409 Conflict` or `422 Unprocessable Entity` |

If no error behavior is defined in code/OpenAPI, do not invent it. Ask for an example.

Example:

````markdown
## Error responses

```http
HTTP/1.1 404 Not Found
Content-Type: application/json

{
  "error": "not_found",
  "message": "User not found"
}
```
````

Validate after editing:

```bash
trellis validate <file>
```

---

### Step 3B - Tests mode

Tests mode generates only `## Tests`, based on the user's stated testing goal. These tests are instructions for humans and LLM agents today; they should be precise enough that an agent can execute them with terminal, web, or MCP tools.

Do not write generic boilerplate. Use the user's goal plus source evidence to write a short checklist with:

- Scope: the behavior under test.
- Required setup data or variables.
- Terminal path: exact `trellis exec` or `trellis flow` command to run.
- Web path: how to run the same check from `trellis serve`.
- LLM/MCP path: which Trellis tool to call and which vars to pass.
- Assertions: status, important headers, and concrete `response.body.<path>` checks.
- Cleanup or follow-up endpoint when relevant.

Write as:

````markdown
## Tests

```agent-task
Scope: verify duplicate email handling for user creation.

Setup:
- Use an email that already exists in the dev database.

Terminal:
- Run `trellis exec api-docs/apis/users/post-users.md --env=dev --var email=existing@example.com`.

Web:
- Start `trellis serve`, open this spec, set email to `existing@example.com`, and send the request.

LLM/MCP:
- Call `trellis_exec` with `spec_path`, `env=dev`, and `vars.email=existing@example.com`.

Assertions:
- Verify response.status is 409.
- Verify response.body.error is `duplicate_email`.
- Verify no Authorization token is printed in logs.
```
````

If the user asks for runnable multi-case API tests, recommend this Trellis feature shape instead of pretending `agent-task` is executable:

- Add a future `## Cases` YAML block with named cases, request overrides, variables, and expected status/body subsets.
- Add `trellis test <file> --case <name>` and `trellis test <dir>` for terminal execution.
- Add a Cases tab in `trellis serve` so users can run selected cases in the web UI.
- Add an MCP tool such as `trellis_test` so LLM agents can execute named cases without parsing free-form prose.

---

### Step 4 - Final report

Report only useful facts:

- Files updated.
- Mode used: `expected`, `tests`, or explicit `all`.
- Evidence source: live response, OpenAPI/schema, handler/DTO, or user example.
- Success and error cases added or skipped.
- Any missing evidence and the exact example needed from the user.
- Validation result.
