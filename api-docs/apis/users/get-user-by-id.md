---
resource: users
protocol: http
method: GET
path: /users/:id
tags: [users, read]
version: 1
env: [dev, staging]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---

# Get user by id

Fetch one user by path parameter.

## Request

```http
GET {{baseUrl}}/users/:id
Accept: application/json
X-Request-Id: {{requestId}}
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": 1,
  "name": "Leanne Graham",
  "email": "Sincere@april.biz"
}
```

## Tests

```agent-task
Run with id=1 and verify response.body.id equals 1.
```
