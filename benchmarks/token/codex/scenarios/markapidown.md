You are running a read-only token benchmark for MarkApiDown-assisted API discovery.

Rules:
- Do not edit files.
- Do not start servers.
- Do not call external network services.
- Prefer `examples/agent-token-api/api-docs/` and MarkApiDown validation over source inspection.
- If `target/release/mad` exists, use it only for validation.
- Do not inspect source files unless the MarkApiDown spec is missing or invalid.

Task:
In `examples/agent-token-api`, find the local API endpoint that creates a refund quote.

Return only:
1. HTTP method and path.
2. Request body format and required content.
3. Success response fields.
4. Error cases.
5. Exact spec file inspected.
6. Shell commands run.

Stop once the answer is supported by MarkApiDown specs.
