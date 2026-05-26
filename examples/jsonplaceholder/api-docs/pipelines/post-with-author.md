---
type: pipeline
name: post-with-author
description: Fetch a post and then fetch its author using the captured userId.
continue-on-error: false
parallel: false
---
# Post with author

Demonstrates a two-step pipeline with variable capture. Fetches post 1, captures
the `userId` from the response body, then fetches that user's full record.

## Steps

1. **Get post** -> `posts/get-post-by-id.md`
   - Capture: `response.body.userId` as `userId`
2. **Get author** -> `users/get-user-by-id.md`
   - Inject: `userId`
   - Assert: `response.status == 200`
