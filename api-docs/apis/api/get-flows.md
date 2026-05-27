---
resource: api
protocol: http
method: GET
path: /api/flows
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Flows

## Request

```http
GET {{baseUrl}}/api/flows
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `./src/preview.rs`
