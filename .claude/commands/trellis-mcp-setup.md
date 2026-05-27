---
description: Register the Trellis MCP server with your AI agent
---
Register the Trellis MCP server with Claude Code so Trellis tools are available
to the AI directly (no bash required).

Run:
```bash
claude mcp add trellis -- trellis mcp
```

After registration, the following tools become available inside Claude Code:
- `trellis_exec`       — execute an endpoint spec
- `trellis_flow`       — run a pipeline
- `trellis_validate`   — validate specs in a file or directory
- `trellis_list_specs` — list all endpoint specs with method + path
- `trellis_read_spec`  — read the full content of a spec file
- `trellis_author`     — create a spec file, or update one only after explicit user approval

Trellis spec files are also exposed as **MCP Resources** under the `trellis://spec/` URI scheme,
so models can browse and read specs directly via the resources protocol.

Verify registration:
```bash
claude mcp list
```
