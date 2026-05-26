# Trellis

Trellis is a markdown-native API spec system with a small Rust engine, executable endpoint specs, pipelines, browser preview support, and cross-agent AI skills.

```bash
cargo install trellis
trellis init --name=demo --dev-url=https://jsonplaceholder.typicode.com --yes
trellis validate api-docs/
trellis exec api-docs/posts/get-posts.md --var baseUrl=https://jsonplaceholder.typicode.com --var postId=1
```

See `docs/spec/convention.md` for the canonical markdown convention.
