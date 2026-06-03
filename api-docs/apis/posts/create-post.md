---
resource: posts
protocol: http
method: POST
path: /posts
tags: [posts, write]
version: 1
env: [dev, staging]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---

# Create post

Create a demo post with a JSON request body.

## Request

```http
POST {{baseUrl}}/posts
Accept: application/json
Content-Type: application/json
X-Workspace-Id: {{workspaceId}}
X-Request-Id: {{requestId}}

{
  "title": "Reqbook preview",
  "body": "Runtime body edits should not change markdown.",
  "userId": 1
}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "title": "Reqbook preview",
  "body": "Runtime body edits should not change markdown.",
  "userId": 1,
  "id": 101
}
```

## Tests

```agent-task
Verify status is 201 and response.body.id exists.
```
