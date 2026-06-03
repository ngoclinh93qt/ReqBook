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
   - Capture: `response.body.input_2` as `input_2`
2. **List users** -> `apis/users/get-users.md`
   - Inject: `postId`
