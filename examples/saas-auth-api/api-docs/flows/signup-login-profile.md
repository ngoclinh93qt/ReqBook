---
type: pipeline
name: signup-login-profile
description: Create a user, log in, and fetch the current profile with captured values.
continue-on-error: false
parallel: false
---
# Signup, login, profile

This flow is the onboarding contract reviewers should expect beside signup and
auth code changes.

## Steps

1. **Create user** -> `apis/users/post-create-user.md`
   - Capture: `response.body.id` as `userId`
   - Assert: `response.status == 201`
2. **Login** -> `apis/auth/post-login.md`
   - Inject: `email`, `password`, `userId`
   - Capture: `response.body.token` as `authToken`
   - Assert: `response.status == 200`
3. **Get profile** -> `apis/users/get-current-user.md`
   - Inject: `authToken`, `userId`, `email`, `role`
   - Assert: `response.status == 200`
