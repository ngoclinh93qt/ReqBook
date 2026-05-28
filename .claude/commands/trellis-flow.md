---
description: Build or run a Trellis workflow pipeline — reasons about data dependencies between endpoints, capture/inject patterns, and meaningful assertions to connect a sequence of API calls
---
Build or run the workflow described in $ARGUMENTS.

---

### If running an existing flow:

```bash
trellis flow api-docs/flows/<name>.md --env=dev
```

Or via MCP:
```json
{ "tool": "trellis_flow", "pipeline_path": "api-docs/flows/<name>.md", "env": "dev" }
```

Report for each step: endpoint, HTTP status, captured values (mask secrets with `****`), assertion result, and the first failure with diagnosis.

---

### If building a new flow from a description:

**Step 1 — Read all relevant specs:**
```bash
rg -rn "^method:\|^path:\|^# " api-docs/apis/
```

**Step 2 — Reason about data dependencies before writing anything:**

Ask yourself for each step in the scenario:
- What does this step produce that the next step needs?
- What is the minimal set of captures to make the flow work?
- What assertions make each step meaningful (not just "status 200")?
- What happens if a step fails — does the failure message tell you why?

Example reasoning for a checkout flow:
```
POST /auth/login
  → captures: authToken (needed by all subsequent steps)

POST /cart
  → injects: authToken
  → captures: cartId (needed by add-item)

POST /cart/:cartId/items
  → injects: authToken, cartId
  → captures: itemId (for later verification)
  → asserts: item count in response = 1

POST /orders
  → injects: authToken, cartId
  → captures: orderId
  → asserts: status = "pending"

GET /orders/:orderId
  → injects: authToken, orderId
  → asserts: status = "pending", items array is not empty
```

**Step 3 — Identify missing specs:**

If a step needs an endpoint that doesn't have a spec yet, stop and list the missing ones. Offer to create them with `/trellis-scan` or author them directly. Don't write a flow that references spec files that don't exist.

**Step 4 — Write the pipeline file:**

```markdown
---
name: <descriptive-flow-name>
type: pipeline
description: <one sentence what this flow validates end-to-end>
---

## Steps

### 1. <Step name>
spec: api-docs/apis/<resource>/<file>.md
env: dev

Capture:
- <varName>: response.body.<field>

Assert:
- response.status == <expected>

### 2. <Step name>
spec: api-docs/apis/<resource>/<file>.md
env: dev

Inject:
- <varName>

Capture:
- <nextVar>: response.body.<field>

Assert:
- response.status == <expected>
- response.body.<key_field> != null
```

**Capture expression patterns:**
- `response.body.id` — simple field
- `response.body.data.token` — nested field
- `response.body[0].id` — first item in an array
- `response.headers.Location` — from response header

**Step 5 — Validate:**
```bash
trellis validate api-docs/flows/<name>.md
trellis index
```

Report: flow path, steps in order, what each step captures, what each step asserts, and any spec files that were missing.

Do not run the flow unless the user explicitly asks to execute it.
