---
resource: api
protocol: http
method: GET
path: /api/workspace/all
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# All

## Request

```http
GET {{baseUrl}}/api/workspace/all
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `/Users/linh/linh/reqbook/Reqbook/src/preview.rs`
