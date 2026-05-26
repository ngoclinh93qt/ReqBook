---
name: trellis-author
description: Use this skill when the user wants to add, scaffold, or document a new API endpoint into the Trellis spec system. Triggers on phrases like "add endpoint", "create API for X", "scaffold a new route", "document the POST /users endpoint", or when the user describes an HTTP method+path not yet in api-docs/. Do NOT use for testing existing endpoints (use trellis-exec) or running pipelines (use trellis-flow).
---

# Trellis author

Use this skill to add or update Trellis endpoint specifications in `api-docs/`. Trellis specs are executable markdown. The goal is to create files that a developer can read, a CLI can validate, and an agent can use safely without guessing.

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

Inspect `api-docs/` before creating files.

1. If a matching resource folder exists, use it.
2. If a near match exists, prefer the existing project naming.
3. If no folder exists, create `api-docs/<resource>/`.
4. Do not ask for confirmation when the resource is obvious from the path.
5. Ask before creating a surprising folder, such as `misc`, `api`, or `v1`.

Examples:

- `/users/:id` routes to `api-docs/users/`.
- `/orders/:id/items` routes to `api-docs/orders/`.
- `/webhooks/stripe` routes to `api-docs/webhooks/`.

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
- File: `api-docs/users/get-user-by-id.md`
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
- File: `api-docs/orders/post-orders.md`
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
- File: `api-docs/webhooks/post-webhooks-invoice-paid.md`
- Auth: `custom`
- Tags: `[webhooks, receive]`

Key choices:

- Use `X-Signature: {{webhookSignature}}`.
- Put signing guidance in `## Notes`.
- Store `webhookSecret` only in `.env.local` or `TRELLIS_WEBHOOK_SECRET`.

