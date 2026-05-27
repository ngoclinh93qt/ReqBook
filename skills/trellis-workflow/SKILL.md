---
name: trellis-workflow
description: Use this skill when the user wants to create, design, scaffold, or edit a Trellis workflow/pipeline/flow in api-docs/flows/. Triggers on phrases like "create workflow", "build a signup flow", "chain these APIs", "connect endpoint A to endpoint B", "capture id then use it", "make a flow canvas", or when the user describes a multi-step API scenario that does not yet have a pipeline file. Do NOT use for running an existing pipeline (use trellis-flow), executing one endpoint (use trellis-exec), or documenting a single endpoint (use trellis-author).
---

# Trellis workflow

Use this skill to create or edit Trellis workflow files under `api-docs/flows/`. A workflow is a markdown-native pipeline that connects existing endpoint specs, captures values from earlier responses, injects them into later requests, and can be executed by `trellis flow` or edited in the web canvas.

## Decision boundary

Use this skill when the user wants to author a multi-step workflow:

- "create workflow"
- "make a flow"
- "chain login and create order"
- "capture the user id and use it in the next call"
- "connect these APIs"
- "build onboarding pipeline"
- "add a payment happy path"
- "turn this scenario into a Trellis flow"

Do not use this skill for these cases:

- One missing endpoint only: use `trellis-author`.
- One existing endpoint execution: use `trellis-exec`.
- Running an existing pipeline without editing it: use `trellis-flow`.
- Importing routes from source code: use `trellis import project` or the scan feature.

If the request mixes endpoint creation and workflow creation, create or update the endpoint specs first, validate them, then create the workflow.

## Operating rules

- Workflow files live in `api-docs/flows/<workflow-name>.md`.
- Workflows are markdown, not JSON, TOML, or hidden agent state.
- Never invent secrets. Use variables such as `{{authToken}}`, `{{userId}}`, and `{{orderId}}`.
- Never inline bearer tokens, cookies, API keys, passwords, or production identifiers.
- Prefer existing endpoint specs. Do not create duplicate endpoint specs just to satisfy a workflow.
- Do not execute the workflow unless the user asks to run or verify it.
- Always validate after writing: `trellis validate api-docs/flows/<workflow>.md`.
- If endpoint specs referenced by the workflow are missing, stop and create them with `trellis-author` or ask for permission to scaffold them.
- Keep workflow step names short and action-oriented.
- Use capture names that are stable and readable: `userId`, `authToken`, `orderId`, `workspaceId`.
- A downstream `Inject` must reference names captured by previous steps.

## Workflow format

Use this exact markdown shape:

```markdown
---
type: pipeline
name: user-onboarding
description: Create a user, authenticate, and fetch the profile
continue-on-error: false
parallel: false
---

# User onboarding

## Steps

1. **Create user** -> `apis/users/create-user.md`
   - Capture: `response.body.id` as `userId`
2. **Login** -> `apis/auth/login.md`
   - Inject: `userId`
   - Capture: `response.body.token` as `authToken`
3. **Fetch profile** -> `apis/users/get-user-by-id.md`
   - Inject: `userId`, `authToken`
   - Assert: `response.status == 200`
```

Use `->` or the unicode arrow only if the project already uses it. `->` is safer across agents and terminals.

## Authoring workflow

Follow these steps in order.

1. Understand the scenario.
2. List the endpoint specs needed.
3. Check whether each endpoint file exists.
4. Decide the capture and inject variables.
5. Write or update the pipeline markdown.
6. Validate the pipeline.
7. Regenerate the index if the project uses one.
8. Report the created file and the data passed between steps.

Useful commands:

```bash
rg --files api-docs | rg '\.md$'
rg -n '^method:|^path:|^# ' api-docs
rg --files api-docs/flows
trellis validate api-docs/flows/<workflow>.md
trellis index
```

## Endpoint selection

Use existing Trellis endpoint specs as workflow blocks.

Match requested APIs by:

- HTTP method and path in frontmatter.
- Filename.
- H1 title.
- Resource folder.
- Existing project naming conventions.

If there are multiple plausible endpoint files, ask one concise question with the choices. If exactly one file clearly matches, proceed.

Do not reference:

- `api-docs/README.md`
- `api-docs/trellis.md`
- `_shared/*.md`
- Another pipeline file as an endpoint step

## Capture rules

Capture values from the response of a step when later steps need them.

Common capture expressions:

- `response.body.id` as `userId`
- `response.body.user.id` as `userId`
- `response.body.token` as `authToken`
- `response.body.refresh_token` as `refreshToken`
- `response.body.order.id` as `orderId`
- `response.headers.etag` as `etag`
- `response.status` as `statusCode`

Use the response shape from the endpoint's `## Expected response` block when possible. If the shape is unknown, choose the most likely JSONPath-like expression and mark it in the final report as an assumption.

Never capture raw secret values into logs. It is fine to capture `authToken` as a variable for pipeline use, but do not print its value.

## Inject rules

Inject a variable into a step only if the endpoint request uses it or should use it.

Common destinations:

- Path params: `/users/:id` uses `userId` when the step is fetching the created user.
- Headers: `Authorization: Bearer {{authToken}}`.
- JSON body: `"userId": "{{userId}}"`.
- Query string: `?cursor={{cursor}}`.

If the endpoint file does not currently include the needed variable placeholder, do not silently edit it unless the user asked for a full workflow implementation. Report the mismatch and ask whether to update the endpoint spec.

## Parallel workflows

Default to sequential workflows:

```yaml
parallel: false
```

Use `parallel: true` only when:

- Steps are independent.
- No step injects a captured value from another parallel step.
- The user specifically wants parallel execution or smoke testing.

For mixed fan-out/fan-in scenarios, write a sequential v1 workflow unless the engine supports the exact graph shape needed. Explain the limitation if necessary.

## Editing an existing workflow

When the workflow file already exists:

1. Read the whole file.
2. Preserve frontmatter keys and comments where practical.
3. Preserve step order unless the user asks to reorder or dependencies require it.
4. Add, delete, or edit only the requested steps.
5. Keep capture and inject names consistent with existing names.
6. Validate after the edit.

If deleting a step, also remove downstream injects that depend on captures from the deleted step, or replace them with a new source if the user provided one.

## Workflow naming

Use a concise slug for frontmatter `name` and filename:

- "user onboarding" -> `user-onboarding`
- "billing happy path" -> `billing-happy-path`
- "auth refresh cycle" -> `auth-refresh-cycle`
- "webhook delivery test" -> `webhook-delivery-test`

File path:

```text
api-docs/flows/<slug>.md
```

H1:

```markdown
# User onboarding
```

## Validation checklist

Before finishing, verify:

- Frontmatter has `type: pipeline`.
- `name` matches the file slug.
- There is exactly one `## Steps` section.
- Every step points to an endpoint markdown file that exists.
- Every `Inject` variable was captured by an earlier step or is intentionally provided from environment/runtime variables.
- Every `Capture` has a source and a variable name.
- No raw secrets appear in the workflow.
- `trellis validate <file>` passes.

## Worked example: user onboarding

User request:

> Create a workflow that creates a user, logs in, and fetches the profile.

Endpoint files found:

- `apis/users/create-user.md`
- `apis/auth/login.md`
- `apis/users/get-user-by-id.md`

Workflow:

```markdown
---
type: pipeline
name: user-onboarding
description: Create a user, authenticate, and fetch the profile
continue-on-error: false
parallel: false
---

# User onboarding

## Steps

1. **Create user** -> `apis/users/create-user.md`
   - Capture: `response.body.id` as `userId`
2. **Login** -> `apis/auth/login.md`
   - Inject: `userId`
   - Capture: `response.body.token` as `authToken`
3. **Fetch profile** -> `apis/users/get-user-by-id.md`
   - Inject: `userId`, `authToken`
   - Assert: `response.status == 200`
```

Report:

- Created `api-docs/flows/user-onboarding.md`.
- Captures `userId` from create user and `authToken` from login.
- Injects both into the profile request.

## Worked example: order checkout

User request:

> Make a checkout workflow: create order, add line item, pay order, then fetch receipt.

Endpoint files found:

- `apis/orders/create-order.md`
- `apis/orders/add-line-item.md`
- `apis/payments/pay-order.md`
- `apis/receipts/get-receipt.md`

Workflow:

```markdown
---
type: pipeline
name: order-checkout
description: Create an order, add an item, pay, and fetch the receipt
continue-on-error: false
parallel: false
---

# Order checkout

## Steps

1. **Create order** -> `apis/orders/create-order.md`
   - Capture: `response.body.id` as `orderId`
2. **Add line item** -> `apis/orders/add-line-item.md`
   - Inject: `orderId`
   - Capture: `response.body.items.0.id` as `lineItemId`
3. **Pay order** -> `apis/payments/pay-order.md`
   - Inject: `orderId`
   - Capture: `response.body.payment.id` as `paymentId`
4. **Fetch receipt** -> `apis/receipts/get-receipt.md`
   - Inject: `orderId`, `paymentId`
   - Assert: `response.status == 200`
```

## Worked example: webhook setup

User request:

> Build a workflow for registering a webhook and sending a test delivery.

Endpoint files found:

- `apis/webhooks/create-webhook.md`
- `apis/webhooks/send-test-event.md`
- `apis/webhooks/get-delivery.md`

Workflow:

```markdown
---
type: pipeline
name: webhook-delivery-test
description: Register a webhook, send a test event, and inspect delivery state
continue-on-error: false
parallel: false
---

# Webhook delivery test

## Steps

1. **Register webhook** -> `apis/webhooks/create-webhook.md`
   - Capture: `response.body.id` as `webhookId`
2. **Send test event** -> `apis/webhooks/send-test-event.md`
   - Inject: `webhookId`
   - Capture: `response.body.delivery_id` as `deliveryId`
3. **Inspect delivery** -> `apis/webhooks/get-delivery.md`
   - Inject: `webhookId`, `deliveryId`
   - Assert: `response.status == 200`
```

## Final response pattern

Keep the final answer concise:

- Workflow file created or edited.
- Steps included.
- Captures and injects.
- Validation result.
- Any assumptions or missing endpoint specs.

Do not paste the entire workflow unless the user asks for it or the file cannot be written.
