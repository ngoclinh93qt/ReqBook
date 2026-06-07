---
type: pipeline
name: demo-post-flow
description: Created in Reqbook web canvas
continue-on-error: false
parallel: false
---

# Demo post flow

## Steps

1. **Create post** -> `apis/posts/create-post.md`
   - Capture: `response.body.id` as `postId`
   - Capture: `response.body.userId` as `id`
2. **Get user by id** -> `apis/users/get-user-by-id.md`
   - Inject: `id`
