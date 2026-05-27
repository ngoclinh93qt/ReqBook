---
resource: webhooks
protocol: http
method: POST
path: /webhooks/:eventType
tags: [webhooks, write]
version: 1
env: [dev, staging]
auth: custom
timeout: 5000
retry:
  attempts: 0
  backoff: fixed
---

# Receive webhook event

Preview a webhook-style request with a path parameter, custom signature header, and nested body.

## Request

```http
POST {{baseUrl}}/webhooks/:eventType
Content-Type: application/json
X-Trellis-Signature: {{signature}}
X-Request-Id: {{requestId}}

{
  "event": "order.created",
  "createdAt": "2026-05-27T00:00:00Z",
  "data": {
    "orderId": "ord_demo_123",
    "amount": 4200
  }
}
```

## Expected response

```http
HTTP/1.1 202 Accepted
Content-Type: application/json

{
  "received": true
}
```

## Tests

```agent-task
Run with eventType=order.created and verify the signature header is present.
```
