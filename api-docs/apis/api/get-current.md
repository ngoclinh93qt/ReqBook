---
resource: api
protocol: http
method: GET
path: /api/workspace/current
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Current

## Request

```http
GET {{baseUrl}}/api/workspace/current
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `/Users/linh/linh/markapidown/MarkApiDown/src/preview.rs`
