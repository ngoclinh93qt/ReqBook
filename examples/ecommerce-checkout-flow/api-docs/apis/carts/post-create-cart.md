---
resource: carts
protocol: http
method: POST
path: /carts
tags: [commerce, carts, checkout]
version: 1
env: [dev]
auth: bearer
timeout: 5000
retry:
  attempts: 1
  backoff: fixed
---
# Create cart

Creates an empty cart for a checkout session. This is the first contract in the
cart-to-checkout business flow.

## Request

```http
POST {{baseUrl}}/carts
Content-Type: application/json
Accept: application/json

{
  "currency": "{{currency}}"
}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "id": "{{cartId}}",
  "status": "open",
  "currency": "{{currency}}"
}
```

## Assertions

- status: 201
- body.id: exists
- body.status: equals open

## Tests

```agent-task
- Capture response.body.id as cartId for downstream steps.
- Verify a cart starts with status open.
```

## Notes

This response is also what `rqb mock` serves in local demos.
