---
resource: posts
protocol: http
method: GET
path: /posts
tags: [posts, read]
version: 1
env: [dev]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Get posts

Returns all 100 posts from JSONPlaceholder.

## Request

```http
GET {{baseUrl}}/posts
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

[{"id": 1}]
```

## Tests

```agent-task
- Verify the response status is 200.
- Verify response.body is an array with 100 items.
- Verify each item has id, userId, title, and body fields.
```
