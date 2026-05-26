---
resource: posts
protocol: http
method: POST
path: /posts
tags: [posts, write]
version: 1
env: [dev]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Create post

Creates a new post. JSONPlaceholder simulates the create and returns a fake resource with id 101.

## Request

```http
POST {{baseUrl}}/posts
Content-Type: application/json
Accept: application/json

{
  "title": "My new post",
  "body": "Post content goes here.",
  "userId": {{userId}}
}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "id": 101
}
```

## Tests

```agent-task
- Verify the response status is 201.
- Verify response.body.id is a positive integer.
- Verify the request body was echoed back in the response.
```
