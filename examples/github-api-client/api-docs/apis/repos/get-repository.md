---
resource: repos
protocol: http
method: GET
path: /repos/:owner/:repo
tags: [github, repos, smoke]
version: 1
env: [dev]
auth: none
timeout: 8000
retry:
  attempts: 1
  backoff: fixed
---
# Get repository

Fetches public metadata for one GitHub repository. This is a small but real
endpoint for testing path variables and public API contracts.

## Request

```http
GET {{baseUrl}}/repos/:owner/:repo
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2022-11-28
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "full_name": "{{owner}}/{{repo}}",
  "private": false,
  "html_url": "https://github.com/{{owner}}/{{repo}}"
}
```

## Assertions

- status: 200
- body.full_name: equals "{{owner}}/{{repo}}"
- body.html_url: contains github.com

## Tests

```agent-task
- Verify the owner and repo variables resolve before running.
- If rate limited, suggest adding a local Authorization header rather than editing env.md with a token.
- Keep the expected response small so the contract does not fail on unrelated GitHub metadata changes.
```

## Notes

This endpoint is useful for demos because it can run against the live GitHub API.
