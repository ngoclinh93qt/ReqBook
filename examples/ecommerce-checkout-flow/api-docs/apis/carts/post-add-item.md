---
resource: carts
protocol: http
method: POST
path: /carts/:cartId/items
tags: [commerce, carts, checkout]
version: 1
env: [dev]
auth: bearer
timeout: 5000
retry:
  attempts: 1
  backoff: fixed
---
# Add item to cart

Adds one product SKU to the cart. Keep this contract close to product catalog
and pricing changes so reviewers can see checkout impact.

## Request

```http
POST {{baseUrl}}/carts/:cartId/items
Content-Type: application/json
Accept: application/json

{
  "sku": "{{sku}}",
  "quantity": {{quantity}}
}
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "cartId": "{{cartId}}",
  "items": [
    {
      "sku": "{{sku}}",
      "quantity": 1
    }
  ],
  "total": 2900
}
```

## Assertions

- status: 200
- body.cartId: exists
- body.total: exists

## Tests

```agent-task
- Verify adding the same SKU twice increments quantity rather than creating duplicate lines.
- Verify total changes when pricing fixtures change.
```

## Notes

This endpoint should fail loudly if the catalog changes a SKU shape used by
checkout.
