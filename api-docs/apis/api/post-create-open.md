---
resource: api
protocol: http
method: POST
path: /api/workspace/open
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Open workspace

## Request

```http
POST {{baseUrl}}/api/workspace/open
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `/Users/linh/linh/reqbook/Reqbook/src/preview.rs`
