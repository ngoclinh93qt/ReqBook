---
name: ecommerce-checkout-flow
version: 1
default-env: dev
---

# Ecommerce checkout flow

Executable API documentation for a cart-to-checkout journey.

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

Run `mad mock api-docs --port 4001` to serve the expected responses locally.
