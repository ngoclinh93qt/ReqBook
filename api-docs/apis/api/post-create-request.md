---
resource: api
protocol: http
method: POST
path: /api/request
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Send ad-hoc request

## Request

```http
POST {{baseUrl}}/api/request
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `/Users/linh/linh/reqbook/Reqbook/src/preview.rs`
