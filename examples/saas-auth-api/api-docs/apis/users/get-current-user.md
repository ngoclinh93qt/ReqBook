---
resource: users
protocol: http
method: GET
path: /me
tags: [users, auth, profile]
version: 1
env: [dev, staging]
auth: bearer
timeout: 5000
retry:
  attempts: 1
  backoff: fixed
---
# Get current user

Returns the profile for the authenticated session. This keeps the agent from
guessing which identity fields are safe to depend on.

## Request

```http
GET {{baseUrl}}/me
Authorization: Bearer {{authToken}}
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": "{{userId}}",
  "email": "{{email}}",
  "role": "{{role}}"
}
```

## Assertions

- status: 200
- body.id: exists
- body.email: contains @
- body.role: in [owner, admin, member]

## Tests

```agent-task
- Verify missing Authorization returns 401.
- Verify the profile id matches the user captured during signup.
- Verify role changes are reflected here before UI authorization changes ship.
```

## Notes

This endpoint is intentionally small. It is the stable context agents should read
before changing user-facing authorization logic.
