---
type: pipeline
name: agent-release-checkout-flow
description: Created in Reqbook web canvas
continue-on-error: false
parallel: false
---

# Agent release checkout flow

## Steps

1. **List users** -> `apis/users/get-users.md`
   - Capture: `response.body[0].id` as `userId`
   - Capture: `response.body[0].email` as `userEmail`
2. **Draft protected order** -> `apis/orders/create-order.md`
   - Capture: `response.body.id` as `orderId`
3. **Fetch selected user** -> `apis/users/get-user-by-id.md`
   - Inject: `userId`
   - Capture: `response.body.name` as `userName`
4. **Create launch post** -> `apis/posts/create-post.md`
   - Inject: `userId`
   - Capture: `response.body.id` as `postId`
5. **Emit order webhook** -> `apis/webhooks/receive-event.md`
   - Inject: `orderId`
   - Capture: `response.body.received` as `webhookReceived`
6. **Create review comment** -> `apis/comments/post-comments.md`
   - Inject: `postId`
   - Capture: `response.body.id` as `commentId`
