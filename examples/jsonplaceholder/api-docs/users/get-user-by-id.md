---
resource: users
protocol: http
method: GET
path: /users/:userId
tags: [users, read]
version: 1
env: [dev]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Get user by id

Fetches a single user record by stable user identifier.

## Request

```http
GET {{baseUrl}}/users/:userId
Accept: application/json
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
- Verify the response status is 200.
- Verify response.body.id equals userId.
- Verify response.body.name is a non-empty string.
- Verify response.body.email contains "@".
```
