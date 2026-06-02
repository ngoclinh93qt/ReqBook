You are running a read-only token benchmark for source-only API discovery.

Rules:
- Do not edit files.
- Do not start servers.
- Do not call external network services.
- Do not use MarkApiDown artifacts or commands.
- Do not read any `api-docs/` directory, `docs/`, `README.md`, `BENCHMARKS.md`, `commands/`, `skills/`, or `skill-templates/`.
- Use only implementation source files under `examples/agent-token-api/src/`.

Task:
In `examples/agent-token-api`, find the local API endpoint that creates a refund quote.

Return only:
1. HTTP method and path.
2. Request body format and required content.
3. Success response fields.
4. Error cases.
5. Exact source files inspected.
6. Shell commands run.

Stop once the answer is supported by source files.
