---
name: github-api-client
version: 1
default-env: dev
---

# GitHub API client

Executable Markdown specs for public GitHub repository smoke checks.

## Defaults

```yaml
timeout: 8000
retry:
  attempts: 1
  backoff: fixed
auth: none
```

## Web preview

```yaml
port: 8080
host: 127.0.0.1
theme: auto
autosave: 2s
```

## Notes

The default requests are public and unauthenticated. Add Authorization headers in
local-only copies if your CI needs higher GitHub rate limits.
