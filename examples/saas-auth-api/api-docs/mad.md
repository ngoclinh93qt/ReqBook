---
name: saas-auth-api
version: 1
default-env: dev
---

# SaaS auth API

Executable API documentation for a small SaaS authentication workflow.

## Defaults

```yaml
timeout: 5000
retry:
  attempts: 1
  backoff: fixed
auth: bearer
```

## Web preview

```yaml
port: 8080
host: 127.0.0.1
theme: auto
autosave: 2s
```

## Notes

Use `.env.local` or `MAD_AUTH_TOKEN` for real secrets. Committed markdown keeps
only safe sample values.
