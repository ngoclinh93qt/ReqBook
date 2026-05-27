---
description: Initialise a new Trellis api-docs/ project in the current directory
---
Initialise a new Trellis project in the current directory.

```bash
trellis init
```

Trellis will auto-detect the project name from `package.json`, `Cargo.toml`, `pyproject.toml`, or `go.mod` if present.
After init:
- Show the files created.
- Suggest running `trellis serve` to open the web preview.
- Suggest running `trellis import project .` to scan for existing API routes.
