---
resource: refunds
protocol: http
method: POST
path: /v1/refunds/quote
tags: [refunds, support, benchmark]
version: 1
env: [dev]
auth: bearer
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---
# Create refund quote

Calculates the support-facing refund quote for an order before a refund is issued.

## Request

```http
POST {{baseUrl}}/v1/refunds/quote
Authorization: Bearer {{supportToken}}
Content-Type: application/json
Accept: application/json

{
  "orderId": "{{orderId}}",
  "lineItems": [
    {
      "sku": "sku_keyboard",
      "quantity": 1,
      "unitPriceCents": 12900
    }
  ],
  "reason": "damaged",
  "shippingRefundCents": 499
}
```

## Expected response

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "quoteId": "rfq_ord_1001_damaged",
  "orderId": "ord_1001",
  "currency": "USD",
  "subtotalRefundCents": 12900,
  "shippingRefundCents": 499,
  "restockingFeeCents": 0,
  "totalRefundCents": 13399,
  "expiresInSeconds": 900,
  "requiresApproval": false
}
```

## Error responses

```http
HTTP/1.1 400 Bad Request
Content-Type: application/json

{
  "error": "invalid_json",
  "message": "Request body must be a JSON object."
}
```

```http
HTTP/1.1 401 Unauthorized
Content-Type: application/json

{
  "error": "missing_bearer_token",
  "message": "Authorization header must use Bearer auth."
}
```

```http
HTTP/1.1 401 Unauthorized
Content-Type: application/json

{
  "error": "invalid_token",
  "message": "Bearer token is not authorized for refund quotes."
}
```

```http
HTTP/1.1 422 Unprocessable Entity
Content-Type: application/json

{
  "error": "validation_error",
  "message": "Refund quote request failed validation.",
  "fields": {
    "orderId": "Must match ord_<digits>.",
    "reason": "Must be duplicate, damaged, customer_request, or late_delivery."
  }
}
```

```http
HTTP/1.1 422 Unprocessable Entity
Content-Type: application/json

{
  "error": "policy_rejected",
  "message": "Refund quote total must be greater than zero."
}
```

```http
HTTP/1.1 500 Internal Server Error
Content-Type: application/json

{
  "error": "internal_error",
  "message": "Unexpected server error."
}
```

## Notes

- `orderId` must match `ord_<digits>`.
- `lineItems` is required, must contain 1 to 25 items, and each quantity must be 1 to 10.
- Each line item requires `sku`, `quantity`, and `unitPriceCents`; `unitPriceCents` must be a non-negative integer.
- `reason` must be `duplicate`, `damaged`, `customer_request`, or `late_delivery`.
- `shippingRefundCents` is optional, defaults to 0, and must be 0 to 2500 when present.
- Computed `totalRefundCents` must be greater than 0; otherwise the server returns `422 policy_rejected`.
- `damaged` and `late_delivery` waive the restocking fee.
- Other reasons apply a 15% restocking fee capped at 2500 cents.
- `requiresApproval` is true when the refund total is greater than 50000 cents or the total quantity is greater than 5.
- The server returns `405 method_not_allowed` when this path is called with the wrong HTTP method.
- The server returns `404 not_found` when no route matches the request path.
