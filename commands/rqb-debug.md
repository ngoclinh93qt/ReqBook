---
description: Diagnose a failing endpoint or pipeline — validates, dry-runs, executes, and interprets diffs.
---

Diagnose the issue in $ARGUMENTS (a spec file, pipeline file, or endpoint description).

---

## Single endpoint

**1. Locate:**
```bash
rqb_search   # via MCP — search by method/path/tag
rg -rn "^method:\|^path:" api-docs/apis/ | grep -i "$ARGUMENTS"
```

**2. Validate then dry-run:**
```bash
rqb validate <file>
rqb exec <file> --env=dev --dry-run
```
Check: correct baseUrl, auth header present, path params substituted, body shape correct.

**3. Execute:**
```json
{ "tool": "rqb_exec", "spec_path": "<file>", "env": "dev", "vars": { "id": "123" } }
```

**4. Interpret:**

| Result | Check |
|---|---|
| Exit 2 — invalid spec | Fix frontmatter or `## Request` http block |
| Exit 4 — network error | Check `baseUrl` in `_shared/env.md`, server running |
| Exit 5 — secret detected | Move value to `.env.local` or `RQB_*` |
| Response mismatch | Update `## Expected response` if API changed intentionally, otherwise fix the API |
| 401 / 403 | Check `authToken` in env, `auth:` frontmatter matches header |
| Unresolved variable | Check `_shared/env.md` for `baseUrl`, `.env.local` for tokens |

---

## Pipeline

**1. Locate and read:**
```bash
rg --files api-docs/flows/
```
Check step order, capture expressions, inject names, all referenced spec files exist.

**2. Execute:**
```json
{ "tool": "rqb_flow", "pipeline_path": "<file>", "env": "dev" }
```

**3. Trace the failure:**
- First failing step → debug as single endpoint with captured values from prior steps as `--var`.
- Capture mismatch → update capture expression to match real response shape.
- Inject not resolved → check the producing step succeeded and capture name matches inject name exactly.

---

**Rules:** Never print raw auth tokens. Default to `--env=dev`. Confirm before `--env=prod`. If no spec exists for the failing endpoint, run `/rqb` first.
