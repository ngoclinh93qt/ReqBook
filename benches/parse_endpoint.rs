use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mark_api_down::parser::parse_endpoint;

fn parse_50_line_endpoint(c: &mut Criterion) {
    let source = r#"---
resource: users
protocol: http
method: GET
path: /users/:id
tags: [users, read]
version: 1
env: [dev]
auth: bearer
timeout: 5000
retry:
  attempts: 3
  backoff: exponential
---
# Get user by id

Fetches a user by id.

## Request

```http
GET {{baseUrl}}/users/:id
Authorization: Bearer {{authToken}}
Accept: application/json
X-Trace: {{traceId}}
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": "{{id}}",
  "email": "user@example.com",
  "name": "Example User"
}
```

## Tests

```agent-task
- Verify status is 200.
- Verify id matches.
- Verify email is valid.
```

## Notes

Used by onboarding and account settings.
"#;

    c.bench_function("parse_50_line_endpoint", |b| {
        b.iter(|| parse_endpoint(black_box(source), black_box("bench.md")).unwrap());
    });
}

criterion_group!(benches, parse_50_line_endpoint);
criterion_main!(benches);
