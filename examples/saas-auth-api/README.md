# SaaS auth API example

This example shows MarkApiDown as executable API documentation for an auth
workflow. The specs are intentionally realistic enough for code review: create a
user, log in, capture a token, and fetch the current profile.

```bash
mad validate api-docs/
mad mock api-docs --port 8080
mad flow api-docs/flows/signup-login-profile.md
mad serve
```

Agent prompt:

```text
Use the MarkApiDown specs in examples/saas-auth-api/api-docs to add a password
reset API. Keep request, expected response, assertions, and the onboarding flow
reviewable in Markdown. Validate the specs before you finish.
```

Run against a real API by changing `api-docs/_shared/env.md` or passing
`--var baseUrl=http://localhost:8080`.
