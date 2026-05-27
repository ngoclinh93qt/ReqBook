---
resource: comments
protocol: http
method: POST
path: /comments
tags: [comments, create]
version: 1
env: [dev, staging]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---

# Create comment

Create a new comment associated with a post. Returns the created comment object with a server-assigned id.

## Request

```http
POST {{baseUrl}}/comments
Content-Type: application/json
Accept: application/json
X-Request-Id: {{requestId}}

{
  "postId": {{postId}},
  "name": "{{commentName}}",
  "email": "{{commentEmail}}",
  "body": "{{commentBody}}"
}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "id": 501,
  "postId": 1,
  "name": "Example commenter",
  "email": "commenter@example.com",
  "body": "Great post!"
}
```

## Tests

```agent-task
- Verify response status is 201.
- Verify response.body.id is present and is a number.
- Verify response.body.postId matches the postId sent in the request body.
- Verify response.body.name and response.body.email are not empty.
```

## Notes

`postId` must reference an existing post. JSONPlaceholder accepts any integer 1–100 for `dev` and `staging`.
Variables `commentName`, `commentEmail`, and `commentBody` should be defined in `_shared/env.md` or passed via `--var`.
