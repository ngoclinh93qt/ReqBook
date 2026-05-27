---
name: trellis-demo
version: 1
default-env: dev
---

# Trellis demo

Demo workspace for previewing the Trellis web UI.

## Defaults

```yaml
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
auth: none
```

## Web preview

```yaml
port: 8080
host: 127.0.0.1
theme: auto
autosave: manual
```

## Plugins

```yaml
plugins: []
```

## Notes

This project is safe demo data. It does not contain real secrets.
