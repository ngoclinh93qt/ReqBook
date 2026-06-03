---
type: pipeline
name: cart-to-checkout
description: Create a cart, add an item, and create a checkout session.
continue-on-error: false
parallel: false
---
# Cart to checkout

This flow is business documentation that also runs. It is the example to show
reviewers when API docs, tests, and agent context need to stay together.

## Steps

1. **Create cart** -> `apis/carts/post-create-cart.md`
   - Capture: `response.body.id` as `cartId`
   - Assert: `response.status == 201`
2. **Add item** -> `apis/carts/post-add-item.md`
   - Inject: `cartId`, `sku`, `quantity`
   - Assert: `response.status == 200`
3. **Create checkout session** -> `apis/checkout/post-create-checkout-session.md`
   - Inject: `cartId`
   - Capture: `response.body.id` as `checkoutSessionId`
   - Assert: `response.status == 201`
