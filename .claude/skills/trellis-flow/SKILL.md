---
name: trellis-flow
description: Use this skill when the user wants to run, execute, verify, debug, or inspect an existing Trellis workflow/pipeline/flow in api-docs/flows/. Triggers on phrases like "run the onboarding flow", "execute pipeline X", "test the signup scenario", or "why did this flow fail". Pipeline file must already exist. Do NOT use for single endpoint runs (use trellis-exec), endpoint authoring (use trellis-author), or creating/editing workflows (use trellis-workflow).
---

# Trellis flow

Use this skill to run an existing Trellis pipeline from `api-docs/flows/`. A pipeline chains endpoint specs and passes captured values between steps. If the user asks for one endpoint, use `trellis-exec`. If the user asks to create or edit a new pipeline/workflow/flow, use `trellis-workflow`.

## Operating rules

- Pipeline file must already exist.
- Read the pipeline before executing it.
- Never print captured secrets such as auth tokens.
- Confirm before running against `prod`.
- Report each step, captures by name, and the first failing step.
- Do not rewrite endpoint specs during flow execution unless the user explicitly asks after reviewing results.

## Locate the pipeline

Search `api-docs/flows/`.

```bash
rg --files api-docs/flows
rg -n "name: user-onboarding|# User onboarding" api-docs/flows
```

Match by:

- Pipeline frontmatter `name`.
- Filename.
- H1 title.
- User phrase.

If multiple pipelines match, ask the user to choose. If none match, say that no pipeline exists and offer to author one.

## Read the pipeline

Inspect:

- `continue-on-error`
- `parallel`
- Ordered steps
- Endpoint file paths
- `Inject` directives
- `Capture` directives
- `Assert` directives

Make sure referenced endpoint files exist before running.

## Determine environment

Use:

1. User-provided env.
2. `default-env` from `api-docs/trellis.md`.
3. `dev`.

For production, ask for confirmation and mention the pipeline file.

## Execute

Use:

```bash
trellis flow api-docs/flows/user-onboarding.md --env=dev
```

Pass variables when needed:

```bash
trellis flow api-docs/flows/user-onboarding.md --env=dev --var email=test@example.com
```

Use JSON output when detailed step data is needed:

```bash
trellis flow api-docs/flows/user-onboarding.md --env=dev --output=json
```

## Capture and inject behavior

Captures use response paths:

```markdown
- Capture: `response.body.id` as `userId`
- Capture: `response.body.token` as `authToken`
```

Later steps inject captured names:

```markdown
- Inject: `authToken`, `userId`
```

When reporting captures:

- Safe: `userId=usr_123`
- Masked: `authToken=****`
- Do not print raw bearer tokens.

## Reporting

Report:

- Pipeline file.
- Environment.
- Number of steps.
- Step result list.
- Captures created, with secrets masked.
- Whether the pipeline stopped early.
- Suggested fix for the first failure.

Keep summaries concise unless the user asks for raw JSON.

## Worked example: user onboarding

Pipeline:

```markdown
---
type: pipeline
name: user-onboarding
description: Full onboarding flow
continue-on-error: false
parallel: false
---

# User onboarding

## Steps

1. **Create user** -> `apis/users/create-user.md`
   - Capture: `response.body.id` as `userId`
2. **Login** -> `apis/users/login.md`
   - Inject: `userId`
   - Capture: `response.body.token` as `authToken`
3. **Pair device** -> `devices/pair-device.md`
   - Inject: `authToken`, `userId`
   - Assert: `response.status == 201`
```

Run:

```bash
trellis flow api-docs/flows/user-onboarding.md --env=dev --var email=test@example.com
```

Expected summary:

```text
Pipeline user-onboarding passed in dev.
1. Create user: 201, captured userId=usr_123
2. Login: 200, captured authToken=****
3. Pair device: 201
```

If step 2 fails:

```text
Pipeline user-onboarding failed at step 2, Login. The API returned 401, so step 3 did not run. Check the login request variables or the created user's test credentials.
```

## Continue-on-error

If `continue-on-error: true`, report all failed steps. Do not call the whole pipeline passed unless every required step matched.

## Parallel pipelines

If `parallel: true`, Trellis may run independent steps concurrently. A step that injects a captured value must wait for the capture source. When explaining results, keep the logical step order from the file.

## Safety checklist before final response

- Did you run the pipeline file the user asked for?
- Did you use the intended environment?
- Did you mask tokens and Authorization headers?
- Did you identify the first failing step?
- Did you include Trellis's suggested fix when available?

## MCP mode

When the Trellis MCP server is registered (`claude mcp add trellis -- trellis mcp`), you can call
`trellis_flow` directly as an MCP tool — no bash required.

**Locate the pipeline first** using `trellis_list_specs`:

```json
{
  "tool": "trellis_list_specs",
  "arguments": { "dir": "api-docs/flows/" }
}
```

**Run the pipeline** using `trellis_flow`:

```json
{
  "tool": "trellis_flow",
  "arguments": {
    "pipeline_path": "api-docs/flows/user-onboarding.md",
    "env": "dev",
    "vars": { "email": "test@example.com" }
  }
}
```

Returns a structured result with `passed`, `steps` array (each with `name`, `status`,
`diff_passed`, and `captures`), and `first_failure` when applicable. Captured secrets are
automatically masked in the response.

**Prefer MCP tools over bash** when the Trellis MCP server is available — responses are structured
JSON and no shell escaping is needed.
