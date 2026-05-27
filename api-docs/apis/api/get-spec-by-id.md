---
resource: api
protocol: http
method: GET
path: /api/spec/:path
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Spec by id

## Request

```http
GET {{baseUrl}}/api/spec/:path
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `./.claude/worktrees/agent-aae7a8e70ea5e0fc6/src/preview.rs`
