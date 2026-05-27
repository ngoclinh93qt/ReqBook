---
name: trellis-exec
description: Use this skill when the user wants to test, run, verify, hit, or execute one existing Trellis endpoint spec in api-docs/. Triggers on phrases like "test GET /users/:id", "run this endpoint", "does /webhook work", "verify response shape", or "dry-run this request". Do NOT use for authoring endpoint specs (use trellis-author), creating workflows (use trellis-workflow), or running multi-step pipelines (use trellis-flow).
---

# Trellis exec

Use this skill to execute one existing Trellis endpoint spec. The endpoint file must already exist under `api-docs/`. If the user asks to create or document a missing endpoint, use `trellis-author`. If the user asks to chain multiple endpoints or create a workflow, use `trellis-workflow`. If the workflow already exists and the user wants to run it, use `trellis-flow`.

## Operating rules

- Locate the existing endpoint before running anything.
- Never print secrets. Trellis masks Authorization headers, but you must also avoid repeating raw tokens from user messages.
- Ask before running against `prod` unless the user explicitly passed `--yes` or clearly approved production execution.
- Prefer `--dry-run` when variables are missing or when the user asks what would be sent.
- Report the request, status, duration, and diff outcome.
- If execution fails, include the file path and the suggested fix from Trellis output.

## Locate the endpoint

Resolve the user's request to a markdown file:

1. Search `api-docs/**/*.md`.
2. Match by frontmatter `method` and `path` when possible.
3. Match by filename when the user names a file.
4. Match by title when the user describes the endpoint.
5. If multiple files match, ask the user to choose.
6. If none match, stop and suggest `trellis-author`.

Useful search commands:

```bash
rg -n "method: GET|path: /users/:id|# Get user" api-docs
rg --files api-docs | rg "users|get-user"
```

## Determine environment

Use this priority:

1. User-provided environment.
2. `default-env` in `api-docs/trellis.md`.
3. `dev`.

For production:

- Confirm before execution in an interactive session.
- Mention the exact file and environment.
- Do not ask again if the user already approved this exact run.

## Determine variables

Read the endpoint request block and identify placeholders like `{{baseUrl}}`, `{{authToken}}`, and path params like `:id`.

Variables may come from:

- Pipeline capture, not relevant for single endpoint execution.
- CLI `--var key=value`.
- Endpoint frontmatter.
- `_shared/env.md`.
- `.env.local`.
- OS env prefixed `TRELLIS_`.

If a required variable is missing, either ask for it or run:

```bash
trellis exec api-docs/apis/users/get-user-by-id.md --env=dev --dry-run
```

## Execute

Use:

```bash
trellis exec <file> --env=<env>
```

Add repeated variables as needed:

```bash
trellis exec api-docs/apis/users/get-user-by-id.md --env=dev --var id=usr_123
```

For machine-readable output:

```bash
trellis exec api-docs/apis/users/get-user-by-id.md --env=dev --output=json
```

For CI reports:

```bash
trellis exec api-docs/apis/users/get-user-by-id.md --env=dev --output=junit
```

## Parse output

Report these fields:

- Endpoint file.
- Environment.
- Method and URL, with secrets masked.
- Status code.
- Duration.
- Diff result.
- Failed status/header/body expectations, if any.

Do not paste huge response bodies unless the user asks. Summarize and point to the report output.

## Failure handling

Common failures:

- Exit 2: invalid spec. Fix markdown structure, frontmatter, or variables.
- Exit 3: engine error. Check protocol, request block, or expected response block.
- Exit 4: network error. Check base URL, DNS, VPN, and service availability.
- Exit 5: secret detected. Move the value to `.env.local` or `TRELLIS_*`.

When Trellis prints a suggested fix, include it in your response.

## Worked example: happy path

User request:

```text
Test GET /users/:id in dev with id usr_123.
```

Steps:

1. Locate `api-docs/apis/users/get-user-by-id.md`.
2. Confirm env is `dev`.
3. Run:

```bash
trellis exec api-docs/apis/users/get-user-by-id.md --env=dev --var id=usr_123
```

Response summary:

```text
GET https://api.example.test/users/usr_123
status: 200
duration: 42ms
diff: passed
```

Final response to user:

```text
GET /users/:id passed in dev. It returned 200 in 42 ms and matched the expected response shape.
```

## Worked example: response mismatch

User request:

```text
Verify the order detail endpoint.
```

Run:

```bash
trellis exec api-docs/apis/orders/get-order-by-id.md --env=staging --var orderId=ord_123
```

Trellis reports:

```text
status: expected 200, got 404
body: response body did not match expected shape
```

Final response to user:

```text
The order detail endpoint did not match the spec in staging. `api-docs/apis/orders/get-order-by-id.md` expected 200, but the API returned 404 for `orderId=ord_123`. Check whether the fixture order exists in staging or update the expected response if 404 is now correct.
```

## Masking examples

Safe:

```text
Authorization: Bearer ****
authToken=****
```

Unsafe:

```text
Authorization: Bearer real-token-value
```

Never include the unsafe form in your response.

## MCP mode

When the Trellis MCP server is registered (`claude mcp add trellis -- trellis mcp`), you can call
`trellis_exec` directly as an MCP tool — no bash required.

**Locate the spec first** using `trellis_list_specs`:

```json
{
  "tool": "trellis_list_specs",
  "arguments": { "dir": "api-docs/" }
}
```

Returns:

```json
{
  "count": 12,
  "specs": [
    { "method": "GET", "path": "/users/:id", "spec_path": "api-docs/apis/users/get-user-by-id.md" }
  ]
}
```

**Execute** using `trellis_exec`:

```json
{
  "tool": "trellis_exec",
  "arguments": {
    "spec_path": "api-docs/apis/users/get-user-by-id.md",
    "env": "dev",
    "vars": { "id": "usr_123" }
  }
}
```

Returns a JSON result with `status`, `duration_ms`, `diff_passed`, and any assertion failures.

**Prefer MCP tools over bash** when the Trellis MCP server is available — responses are structured
JSON, secrets are masked, and no shell escaping is needed.
