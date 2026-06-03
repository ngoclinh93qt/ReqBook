---
resource: users
protocol: http
method: POST
path: /users
tags: [users, auth, onboarding]
version: 1
env: [dev, staging]
auth: none
timeout: 5000
retry:
  attempts: 1
  backoff: fixed
---
# Create user

Creates the first account owner during SaaS onboarding. Review this contract in
the same pull request as changes to signup handlers, DTOs, or validation rules.

## Request

```http
POST {{baseUrl}}/users
Content-Type: application/json
Accept: application/json

{
  "email": "{{email}}",
  "password": "{{password}}",
  "name": "{{name}}",
  "role": "{{role}}"
}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "id": "{{userId}}",
  "email": "{{email}}",
  "status": "pending_verification"
}
```

## Assertions

- status: 201
- body.id: exists
- body.email: equals "{{email}}"
- body.status: in [pending_verification, active]

## Tests

```agent-task
- Verify duplicate email returns 409 and does not create another account.
- Verify the response never includes password or passwordHash fields.
- Verify the created id can be used by the signup-login-profile flow.
```

## Notes

This is the contract reviewers should check when signup payload fields change.
