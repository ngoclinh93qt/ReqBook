---
resource: posts
protocol: http
method: GET
path: /posts/:postId
tags: [posts, read]
version: 1
env: [dev]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Get post by id

Fetches a single post by its stable numeric identifier.

## Request

```http
GET {{baseUrl}}/posts/:postId
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": 1,
  "userId": 1,
  "title": "sunt aut facere repellat provident occaecati excepturi optio reprehenderit",
  "body": "quia et suscipit"
}
```

## Tests

```agent-task
- Verify the response status is 200.
- Verify response.body.id equals postId.
- Verify response.body.userId is a positive integer.
- Verify response.body.title is a non-empty string.
```
