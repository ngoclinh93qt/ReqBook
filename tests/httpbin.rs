use trellis::{engine, parser, ExecOpts};

#[tokio::test]
async fn executes_real_get_to_httpbin() {
    let source = r#"---
resource: httpbin
protocol: http
method: GET
path: /get
version: 1
auth: none
timeout: 10000
retry:
  attempts: 3
  backoff: fixed
---
# Get httpbin

Executes a public GET request.

## Request

```http
GET https://httpbin.org/get
Accept: application/json
```

## Expected response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "url": "https://httpbin.org/get"
}
```
"#;
    let endpoint = parser::parse_endpoint(source, "tests/httpbin.md").unwrap();
    let execution = engine::execute(&endpoint, "dev", ExecOpts::default())
        .await
        .unwrap();
    assert_eq!(execution.response.unwrap().status, 200);
}
