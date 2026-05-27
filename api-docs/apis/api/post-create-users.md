---
resource: api
protocol: http
method: POST
path: /api/users
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Create users

## Request

```http
POST {{baseUrl}}/api/users
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `./.claude/worktrees/agent-aae7a8e70ea5e0fc6/src/importer/project.rs`
