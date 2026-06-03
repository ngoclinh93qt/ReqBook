---
resource: repos
protocol: http
method: GET
path: /repos/:owner/:repo/releases
tags: [github, repos, releases]
version: 1
env: [dev]
auth: none
timeout: 8000
retry:
  attempts: 1
  backoff: fixed
---
# List repository releases

Lists releases for a public repository. Use this after fetching repository
metadata when an agent needs release context.

## Request

```http
GET {{baseUrl}}/repos/:owner/:repo/releases
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2022-11-28
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

[
  {
    "tag_name": "v0.0.0",
    "html_url": "https://github.com/{{owner}}/{{repo}}/releases/tag/v0.0.0"
  }
]
```

## Assertions

- status: 200

## Tests

```agent-task
- Treat an empty releases array as acceptable for repos that do not publish releases.
- Keep assertions stable; do not assert the latest release tag unless the test controls the repo.
```

## Notes

The expected response documents the shape agents should look for. It is not a
guarantee that every public repo has releases.
