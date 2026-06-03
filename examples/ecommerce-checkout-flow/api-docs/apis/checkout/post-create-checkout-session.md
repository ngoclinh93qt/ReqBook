---
resource: checkout
protocol: http
method: POST
path: /checkout/sessions
tags: [commerce, checkout, payments]
version: 1
env: [dev]
auth: bearer
timeout: 5000
retry:
  attempts: 1
  backoff: fixed
---
# Create checkout session

Creates a hosted checkout session for an open cart. This is the handoff point
from product selection to payment.

## Request

```http
POST {{baseUrl}}/checkout/sessions
Content-Type: application/json
Accept: application/json

{
  "cartId": "{{cartId}}",
  "successUrl": "https://example.test/success",
  "cancelUrl": "https://example.test/cancel"
}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "id": "{{checkoutSessionId}}",
  "cartId": "{{cartId}}",
  "status": "ready",
  "url": "https://checkout.example.test/session/{{checkoutSessionId}}"
}
```

## Assertions

- status: 201
- body.id: exists
- body.status: equals ready
- body.url: contains checkout.example.test

## Tests

```agent-task
- Capture response.body.id as checkoutSessionId.
- Verify closed or empty carts return a stable 409 error.
- Verify successUrl and cancelUrl are validated before payment handoff.
```

## Notes

Use this contract when changing payment provider adapters.
