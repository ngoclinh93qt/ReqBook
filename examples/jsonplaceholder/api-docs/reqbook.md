---
name: jsonplaceholder
version: 1
default-env: dev
---
# JSONPlaceholder

Example Reqbook project using the free [JSONPlaceholder](https://jsonplaceholder.typicode.com) REST API.
Run `rqb exec`, `rqb flow`, or `rqb serve` from this directory.

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
autosave: 2s
```

## Plugins

```yaml
plugins: []
```
