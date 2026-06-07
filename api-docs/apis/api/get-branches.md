---
resource: api
protocol: http
method: GET
path: /api/git/branches
tags: [api]
version: 1
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Branches

## Request

```http
GET {{baseUrl}}/api/git/branches
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "is_repo": true,
  "root": "/Users/example/project",
  "current": "main",
  "dirty": false,
  "branches": [
    {
      "name": "main",
      "current": true,
      "remote": false,
      "upstream": "origin/main",
      "commit": "a1b2c3d",
      "summary": "Update API docs"
    },
    {
      "name": "feature/api-docs",
      "current": false,
      "remote": false,
      "upstream": null,
      "commit": "d4e5f6a",
      "summary": "Add branch switch smoke test"
    }
  ]
}
```

## Notes

Returns `is_repo: false` with an empty `branches` array when the current workspace is not inside a Git repository. `dirty` is true when the worktree has uncommitted changes, including untracked files.
