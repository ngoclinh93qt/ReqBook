# MarkApiDown examples

Ready-to-run example projects. Each example is a complete MarkApiDown project with a full
`api-docs/` directory, environment config, and at least one pipeline.

## jsonplaceholder

A full API project against the free [JSONPlaceholder](https://jsonplaceholder.typicode.com)
REST API. No API key required. Demonstrates:

- Multiple resources (`posts`, `users`)
- GET, POST methods
- Path parameters (`:postId`, `:userId`)
- Environment variables (`{{baseUrl}}`, `{{postId}}`, `{{userId}}`)
- A two-step pipeline with variable capture (`post-with-author`)

```bash
cd examples/jsonplaceholder

# Validate all specs
mad validate api-docs/

# Execute one endpoint (requires network access to jsonplaceholder.typicode.com)
mad exec api-docs/apis/posts/get-post-by-id.md

# Execute the pipeline
mad flow api-docs/pipelines/post-with-author.md

# Open the web preview
mad serve

# Dry-run without sending
mad exec api-docs/apis/posts/create-post.md --dry-run
```

### Endpoints

| Method | Path | File |
| --- | --- | --- |
| GET | /posts | posts/get-posts.md |
| GET | /posts/:postId | posts/get-post-by-id.md |
| POST | /posts | posts/create-post.md |
| GET | /users | users/get-users.md |
| GET | /users/:userId | users/get-user-by-id.md |

### Pipeline: post-with-author

Fetches post 1, captures the `userId` from the response, then fetches that
user's full record. Run with:

```bash
mad flow api-docs/pipelines/post-with-author.md
```

## agent-token-api

A local fixture for the agent token benchmark. It includes a tiny Node.js API
implementation and matching MarkApiDown specs so agents can be measured in two
modes:

- source-only discovery from `examples/agent-token-api/src/`
- MarkApiDown-assisted discovery from `examples/agent-token-api/api-docs/`

```bash
cd examples/agent-token-api
mad validate api-docs/
npm start
```

## saas-auth-api

A realistic auth/onboarding workspace for agent-assisted backend changes. It
documents create user, login, and current-user endpoints plus a flow that
captures `userId` and `authToken`.

```bash
cd examples/saas-auth-api
mad validate api-docs/
mad mock api-docs --port 8080
mad flow api-docs/flows/signup-login-profile.md
mad serve
```

Use this example when testing PR-reviewable API docs, auth variables, and agent
prompts for "add a password reset API" style tasks.

## github-api-client

A public API workspace for GitHub repository smoke checks. No token is required
for the default public requests, but teams can add `MAD_GITHUB_TOKEN` locally if
they want authenticated rate limits.

```bash
cd examples/github-api-client
mad validate api-docs/
mad exec api-docs/apis/repos/get-repository.md
mad flow api-docs/flows/repository-release-smoke.md
```

Use this example to show path variables, public API docs, and API testing for
coding agents without running a local server.

## ecommerce-checkout-flow

A business-flow example that reads like checkout documentation and runs in mock
mode from recorded expected responses.

```bash
cd examples/ecommerce-checkout-flow
mad validate api-docs/
mad mock api-docs --port 4001
mad flow api-docs/flows/cart-to-checkout.md
```

Use this example to demonstrate executable business workflows, captured cart and
checkout IDs, and CI-friendly smoke checks.
