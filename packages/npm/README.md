# mark-api-down

Thin npm wrapper for the MarkApiDown Rust binary.

```bash
npx mark-api-down@latest version
npx mark-api-down@latest validate api-docs/
```

The wrapper downloads the matching binary from GitHub Releases on first run and caches it in `~/.cache/mark-api-down/`.
