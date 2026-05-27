---
resource: api
protocol: http
method: GET
path: /api/scan/project
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Project

## Request

```http
GET {{baseUrl}}/api/scan/project
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `./src/preview.rs`
