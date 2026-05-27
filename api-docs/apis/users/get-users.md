---
resource: users
protocol: http
method: GET
path: /users
tags: [users, read]
version: 1
env: [dev, staging]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---

# List users

Fetch all demo users from the public JSONPlaceholder API.

## Request

```http
GET {{baseUrl}}/users
Accept: application/json
X-Request-Id: {{requestId}}
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

[
  {
    "id": 1,
    "name": "Leanne Graham",
    "email": "Sincere@april.biz"
  }
]
```

## Tests

```agent-task
Verify the response is a non-empty array and each item has id, name, and email.
```

## Notes

Public API endpoint, no authentication required.
