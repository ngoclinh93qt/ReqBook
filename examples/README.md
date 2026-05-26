# Trellis examples

Ready-to-run example projects. Each example is a complete Trellis project with a full
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
trellis validate api-docs/

# Execute one endpoint (requires network access to jsonplaceholder.typicode.com)
trellis exec api-docs/posts/get-post-by-id.md

# Execute the pipeline
trellis flow api-docs/pipelines/post-with-author.md

# Open the web preview
trellis serve

# Dry-run without sending
trellis exec api-docs/posts/create-post.md --dry-run
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
trellis flow api-docs/pipelines/post-with-author.md
```
