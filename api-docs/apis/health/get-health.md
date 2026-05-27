---
resource: health
protocol: http
method: GET
path: /health
tags: [health]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Health

## Request

```http
GET {{baseUrl}}/health
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `./.claude/worktrees/agent-aae7a8e70ea5e0fc6/src/importer/project.rs`
