---
resource: auth
protocol: http
method: POST
path: /auth/login
tags: [auth, session, onboarding]
version: 1
env: [dev, staging]
auth: none
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Login

Authenticates a newly created user and returns a session token for downstream
profile and workspace calls.

## Request

```http
POST {{baseUrl}}/auth/login
Content-Type: application/json
Accept: application/json

{
  "email": "{{email}}",
  "password": "{{password}}"
}
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "token": "session_token_example",
  "user": {
    "id": "{{userId}}",
    "email": "{{email}}"
  }
}
```

## Assertions

- status: 200
- body.token: exists
- body.user.id: exists
- body.user.email: equals "{{email}}"

## Tests

```agent-task
- Capture response.body.token as authToken before calling authenticated endpoints.
- Verify wrong password returns 401 with a stable error code.
- Verify token-like values are never committed to env.md.
```

## Notes

Use this endpoint as the auth dependency for profile, billing, and workspace
flows.
