---
type: pipeline
name: demo-post-flow
description: Created in Trellis web canvas
continue-on-error: false
parallel: false
---

# Demo post flow

## Steps

1. **Create post** → `apis/posts/create-post.md`
   - Capture: `response.body.id` as `postId`
2. **List users** → `apis/users/get-users.md`
   - Inject: `postId`
