---
resource: api
protocol: http
method: POST
path: /api/git/checkout
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Create checkout

## Request

```http
POST {{baseUrl}}/api/git/checkout
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `/Users/linh/linh/markapidown/MarkApiDown/src/preview.rs`
