---
resource: api
protocol: http
method: POST
path: /api/exec/:path
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Create exec

## Request

```http
POST {{baseUrl}}/api/exec/:path
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `./.claude/worktrees/agent-aae7a8e70ea5e0fc6/src/preview.rs`
