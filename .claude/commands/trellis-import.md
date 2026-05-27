---
description: Scan the current project source code for API routes and import them as Trellis specs
---
Scan the project source code for API route definitions and generate Trellis endpoint specs.

```bash
trellis import project ${ARGUMENTS:-.}
```

Trellis detects routes from Express, FastAPI, Flask, Axum, Actix, Gin, Spring Boot, Laravel, Rails, and more.
After import:
- Show the list of created spec files.
- Remind the user to set `baseUrl` in `api-docs/_shared/env.md`.
- Offer to run `trellis validate api-docs/` to confirm all generated specs are valid.
