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
# Checkout git branch

## Request

```http
POST {{baseUrl}}/api/git/checkout
Content-Type: application/json

{"branch":"feature/api-docs"}
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "is_repo": true,
  "root": "/Users/example/project",
  "current": "feature/api-docs",
  "dirty": false,
  "branches": [
    {
      "name": "feature/api-docs",
      "current": true,
      "remote": false,
      "upstream": null,
      "commit": "d4e5f6a",
      "summary": "Add branch switch smoke test"
    },
    {
      "name": "main",
      "current": false,
      "remote": false,
      "upstream": "origin/main",
      "commit": "a1b2c3d",
      "summary": "Update API docs"
    }
  ]
}
```

## Notes

Returns `400` when `branch` is empty, unknown, or the workspace is not inside a Git repository. Returns `409` when Git refuses the checkout, for example because uncommitted changes would be overwritten.
