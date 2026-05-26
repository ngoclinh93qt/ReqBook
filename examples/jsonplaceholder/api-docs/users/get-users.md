---
resource: users
protocol: http
method: GET
path: /users
tags: [users, read]
version: 1
env: [dev]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Get users

Returns all 10 users.

## Request

```http
GET {{baseUrl}}/users
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
- Verify response.body is an array with 10 items.
- Verify each item has id, name, email, and address fields.
```
