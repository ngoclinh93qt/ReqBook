---
name: trellis-author
description: Use this skill when the user wants to add, scaffold, document, or update one API endpoint spec in api-docs/. Triggers on phrases like "add endpoint", "document POST /users", "create spec for GET /orders/:id", "scaffold route", or when a single HTTP method+path is missing from Trellis. Do NOT use for executing endpoints (use trellis-exec), running existing pipelines (use trellis-flow), or creating multi-step workflows/pipelines/flows (use trellis-workflow).
---

# Trellis author

Use this skill to add or update one Trellis endpoint specification in `api-docs/apis/`. Trellis specs are executable markdown. The goal is to create files that a developer can read, a CLI can validate, and an agent can use safely without guessing.

If the user asks to connect multiple endpoints, capture values between calls, create a flow canvas, or author a workflow, stop and use `trellis-workflow` instead.

## Operating rules

- Never overwrite an existing endpoint file.
- Never execute the endpoint while authoring it.
- Never inline secrets, tokens, passwords, session cookies, or production credentials.
- Always use `{{variable}}` placeholders for values that differ by environment or user.
- Always validate after writing: `trellis validate <file>`.
- Always regenerate the index after successful validation: `trellis index`.
- Keep headings in sentence case.
- Keep one endpoint per file.
- Keep one executable `http` block in `## Request`.
- Keep one expected `http` block in `## Expected response`.
- Use `agent-task` only for instructions, not executable code.

## Intent parsing

Extract these details from the user request:

- HTTP method: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, or `OPTIONS`.
- Path: for example `/users/:id`, `/orders`, `/webhooks/stripe`.
- Resource: usually the first path segment, singular or plural as already used in the project.
- Action: read, create, update, delete, search, login, logout, send, verify, or receive.
- Auth expectation: `none`, `bearer`, `basic`, or `custom`.
- Request body shape, if the method usually has a body.
- Expected status and response shape, if known.
- Environment scope, usually `[dev, staging, prod]` unless the user says otherwise.

If method or path is missing, ask one concise question. If both are present, proceed.

## Resource routing

Inspect `api-docs/apis/` before creating files.

1. If a matching resource folder exists, use it.
2. If a near match exists, prefer the existing project naming.
3. If no folder exists, create `api-docs/apis/<resource>/`.
4. Do not ask for confirmation when the resource is obvious from the path.
5. Ask before creating a surprising folder, such as `misc`, `api`, or `v1`.

Examples:

- `/users/:id` routes to `api-docs/apis/users/`.
- `/orders/:id/items` routes to `api-docs/apis/orders/`.
- `/webhooks/stripe` routes to `api-docs/apis/webhooks/`.

## Filename derivation

Use `<method-lower>-<slug>.md`.

Derive the slug from the path:

- Remove leading and trailing slashes.
- Drop generic version prefixes such as `v1` only if the project already omits them.
- Convert path params from `:id` to `by-id`.
- Convert `{id}` path params to `by-id` if imported from another style.
- Use hyphens.
- Keep filenames short but unambiguous.

Examples:

- `GET /users/:id` -> `get-user-by-id.md`
- `POST /users` -> `post-users.md`
- `PATCH /orders/:orderId/items/:itemId` -> `patch-order-by-order-id-item-by-item-id.md`
- `POST /webhooks/stripe` -> `post-webhooks-stripe.md`

Before writing, check whether the target filename already exists. If it exists, inspect it and report that the endpoint is already documented. Do not overwrite.

## Frontmatter generation

Every endpoint file starts with YAML frontmatter.

```yaml
---
resource: users
protocol: http
method: GET
path: /users/:id
tags: [users, read]
version: 1
env: [dev, staging, prod]
auth: bearer
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
```

Defaults:

- `protocol: http`
- `version: 1`
- `env: [dev]` for local-only examples, otherwise `[dev, staging, prod]`
- `auth: bearer` when the API appears authenticated
- `auth: none` for public APIs, health checks, login, and public webhooks
- `timeout: 5000`
- `retry.attempts: 0`
- `retry.backoff: fixed`

Tags should include the resource and action:

- read: `GET`
- create: `POST` collection
- update: `PUT` or `PATCH`
- delete: `DELETE`
- auth: login, logout, token refresh
- webhook: inbound webhook endpoints

## Body generation

Use this section order exactly:

1. `# <Sentence-case title>`
2. One-paragraph description
3. `## Request` with one `http` code block
4. `## Expected response` with one `http` code block
5. `## Tests` with one `agent-task` code block
6. `## Notes` only when useful

The request block must use variables:

```http
GET {{baseUrl}}/users/:id
Authorization: Bearer {{authToken}}
Accept: application/json
```

For JSON bodies:

```http
POST {{baseUrl}}/orders
Authorization: Bearer {{authToken}}
Content-Type: application/json

{
  "customerId": "{{customerId}}",
  "items": [
    {
      "sku": "{{sku}}",
      "quantity": 1
    }
  ]
}
```

Expected responses should be realistic but safe. Use example IDs like `usr_123`, `ord_123`, and `evt_123`. Do not invent secrets.

## Post-creation workflow

After writing a new endpoint file:

1. Run `trellis validate <file>`.
2. If validation fails, fix the file and rerun validation.
3. Run `trellis index`.
4. Summarize the created file path and any assumptions.

Do not run `trellis exec` from this skill.

## Worked example: document `GET /users/:id`

User request:

```text
Document GET /users/:id with bearer auth.
```

Action:

- Resource: `users`
- File: `api-docs/apis/users/get-user-by-id.md`
- Auth: `bearer`
- Tags: `[users, read]`

Expected file:

```markdown
---
resource: users
protocol: http
method: GET
path: /users/:id
tags: [users, read]
version: 1
env: [dev, staging, prod]
auth: bearer
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Get user by id

Fetches one user by stable user identifier.

## Request

```http
GET {{baseUrl}}/users/:id
Authorization: Bearer {{authToken}}
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": "usr_123",
  "email": "person@example.com",
  "name": "Example Person"
}
```

## Tests

```agent-task
- Verify the response status is 200.
- Verify response.body.id matches the requested id.
- Verify Authorization is masked in all output.
```
```

## Worked example: create an order

User request:

```text
Add POST /orders for creating an order.
```

Action:

- Resource: `orders`
- File: `api-docs/apis/orders/post-orders.md`
- Auth: `bearer`
- Tags: `[orders, create]`

Key choices:

- Use `customerId`, `sku`, and `quantity` variables.
- Expected status is `201 Created`.
- Do not include a real payment token.

## Worked example: webhook receiver

User request:

```text
Document POST /webhooks/invoice-paid for inbound invoice webhooks.
```

Action:

- Resource: `webhooks`
- File: `api-docs/apis/webhooks/post-webhooks-invoice-paid.md`
- Auth: `custom`
- Tags: `[webhooks, receive]`

Key choices:

- Use `X-Signature: {{webhookSignature}}`.
- Put signing guidance in `## Notes`.
- Store `webhookSecret` only in `.env.local` or `TRELLIS_WEBHOOK_SECRET`.

## MCP mode

When the Trellis MCP server is registered (`claude mcp add trellis -- trellis mcp`), you can
author, inspect, and validate specs using MCP tools — no bash required.

### Check existing specs before creating

Use `trellis_list_specs` to avoid duplicates:

```json
{
  "tool": "trellis_list_specs",
  "arguments": { "dir": "api-docs/" }
}
```

Or read a specific spec to understand current structure:

```json
{
  "tool": "trellis_read_spec",
  "arguments": { "spec_path": "api-docs/apis/users/get-user-by-id.md" }
}
```

Returns `content` (raw markdown), `method`, `path`, and `resource` fields.

### Write the spec

Use `trellis_author` to create a new endpoint file. The tool validates the content via
`trellis validate` **before** writing — it will return an error without touching the filesystem
if the spec is invalid:

```json
{
  "tool": "trellis_author",
  "arguments": {
    "spec_path": "api-docs/apis/users/get-user-by-id.md",
    "content": "---\nresource: users\nprotocol: http\nmethod: GET\npath: /users/:id\n...\n---\n# Get user by id\n\n..."
  }
}
```

If the file already exists, stop and ask before changing it. Do not set overwrite unless the user explicitly asked to replace that exact spec path.

### Validate after authoring

Use `trellis_validate` to confirm the spec is well-formed:

```json
{
  "tool": "trellis_validate",
  "arguments": { "path": "api-docs/apis/users/get-user-by-id.md" }
}
```

Returns `{ "valid": true, "file_count": 1 }` on success, or a list of errors with line
references on failure.

### Access specs as MCP Resources

Specs are also exposed under the `trellis://spec/` URI scheme. Models supporting the MCP
resources protocol can browse and read them directly:

- `trellis://spec/users/get-user-by-id.md`
- `trellis://spec/flows/user-onboarding.md`

**Prefer MCP tools over bash** when the Trellis MCP server is available — `trellis_author`
validates before writing, so you get the safety of `trellis validate` without a second shell call.
