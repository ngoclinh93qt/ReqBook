---
description: Start the Trellis mock server to replay recorded API responses
---
Start the Trellis mock server so the frontend can work without a live backend.

```bash
trellis mock ${ARGUMENTS:-api-docs/} --port 4001
```

The mock server reads every `## Expected response` block from endpoint specs and serves those
responses over HTTP. Path parameters like `/users/:id` are matched automatically.

Report:
- The base URL (e.g. `http://127.0.0.1:4001`)
- The number of routes loaded and their method + path
- Any duplicate routes that were skipped

To add artificial latency (useful for testing loading states):
```bash
trellis mock api-docs/ --port 4001 --latency 300
```
