---
resource: api
protocol: http
method: GET
path: /api/time
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Time

## Request

```http
GET {{baseUrl}}/api/time
accept: application/json, text/plain, */*
authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJsaW5obmd1eWVuIiwiaWF0IjoxNzc5OTY5NDA1LCJleHAiOjE4NDMwNDE0MDV9.Oqy8aHSf8qpDLBTJovD4zKeJnS3zmNYM2xRJeww1FKI
referer: https://noc.eyecast.com/
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{}
```

## Notes

Imported from: `https://noc.eyecast.com/api/time`

Set `baseUrl` to `https://noc.eyecast.com` in `api-docs/_shared/env.md` (or `.env.local`).

11 browser-specific header(s) removed (user-agent, sec-*, pragma, cache-control, …). Add them back if your API actually requires them.
