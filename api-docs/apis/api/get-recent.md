---
resource: api
protocol: http
method: GET
path: /api/workspace/recent
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Recent

## Request

```http
GET {{baseUrl}}/api/workspace/recent
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `/Users/linh/linh/reqbook/Reqbook/src/preview.rs`
