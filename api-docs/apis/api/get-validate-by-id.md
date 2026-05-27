---
resource: api
protocol: http
method: GET
path: /api/validate/:path
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Validate by id

## Request

```http
GET {{baseUrl}}/api/validate/:path
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `./src/preview.rs`
