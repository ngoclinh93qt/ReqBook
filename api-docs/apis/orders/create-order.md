---
resource: orders
protocol: http
method: POST
path: /orders
tags: [orders, write]
version: 1
env: [dev, staging]
auth: bearer
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---

# Create order

Draft order endpoint used to preview headers, browser variables, and body editing.

## Request

```http
POST {{baseUrl}}/orders
Accept: application/json
Content-Type: application/json
Authorization: Bearer {{token}}
Idempotency-Key: {{requestId}}

{
  "customerId": "cus_demo_123",
  "items": [
    {
      "sku": "sku_basic",
      "quantity": 2
    }
  ],
  "currency": "USD"
}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "id": "ord_demo_123",
  "status": "created",
  "currency": "USD"
}
```

## Tests

```agent-task
Use a browser-local token variable. Verify Authorization is masked in output.
```

## Notes

This endpoint is intentionally non-runnable against JSONPlaceholder. It is here to preview protected API UI states.
