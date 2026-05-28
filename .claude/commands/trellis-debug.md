---
description: Diagnose a failing endpoint or pipeline — validates, dry-runs, executes, and interprets diffs with context
---
Follow the trellis-debug skill decision tree to diagnose the issue described in $ARGUMENTS.

---

### Single endpoint

1. **Locate the spec** — by method+path, filename, or natural description:
   ```bash
   rg -rn "^method:\|^path:" api-docs/apis/
   rg --files api-docs/apis/ | grep -i "$ARGUMENTS"
   ```

2. **Validate before running:**
   ```bash
   trellis validate <file>
   ```
   Exit 2 = structural error. Fix before continuing.

3. **Dry-run to see the resolved request:**
   ```bash
   trellis exec <file> --env=dev --dry-run
   ```
   Check: correct baseUrl, auth header present, path params substituted.

4. **Execute:**
   ```bash
   trellis exec <file> --env=dev --var key=value
   ```
   Or via MCP: `trellis_exec { spec_path, env, vars }`

5. **Interpret the result:**

   | Exit code | Meaning | Check |
   |---|---|---|
   | 0 | Passed | — |
   | 2 | Invalid spec | Fix frontmatter or http block |
   | 3 | Engine error | Check protocol, request block |
   | 4 | Network error | Check baseUrl, DNS, service running |
   | 5 | Secret detected | Move to `.env.local` |

   **On diff mismatch:** decide whether the spec's `## Expected response` is outdated (API changed intentionally) or the API broke. Update the spec if the change is intentional.

---

### Pipeline failure

1. Locate: `rg --files api-docs/flows/`
2. Execute: `trellis flow <file> --env=dev`
3. On failure: identify the first failing step.
4. Debug that step as a single endpoint, injecting captured values from prior steps as `--var`.
5. Check capture expressions match the actual response shape (`response.body.id` vs `response.body.userId`).

---

**Rules:**
- Never print raw auth tokens. Trellis masks them; don't re-print.
- Confirm before running against `prod` — use `--env=staging` or `--env=dev` by default.
- If no spec exists for the failing endpoint, run `/trellis-scan` first.
