---
description: Initialise a new Trellis api-docs/ project in the current directory
---
Initialise a new Trellis project in the current directory.
Follow the trellis-init skill decision tree exactly.

### Step 1 — Check if already initialised

```bash
ls api-docs/trellis.md 2>/dev/null && echo EXISTS
```

If EXISTS → stop and inform the user. Offer `trellis import project` instead.

### Step 2 — Detect project name

Try in order: `package.json` → `Cargo.toml` → `pyproject.toml` → `go.mod` →
`composer.json` → `pom.xml` → `README.md` first heading → git remote → directory name.

If the detected name looks like a template default (`my-api`, `app`, `project`),
investigate further before using it.

### Step 3 — Detect base URL

Check: `.env` / `.env.local` for PORT/HOST → `docker-compose.yml` ports →
framework defaults (FastAPI=8000, Express/Rails=3000, Spring Boot/Gin=8080) →
`README.md` for localhost mentions → ask the user as a last resort.

### Step 4 — Run init

```bash
trellis init --name "<detected-name>" --dev-url "<detected-url>" --yes
```

If either value is uncertain, run without `--yes` and let trellis prompt interactively.

### Step 5 — Verify and fix scaffold

```bash
cat api-docs/trellis.md        # verify name field is correct
cat api-docs/_shared/env.md   # verify baseUrl is correct
trellis validate api-docs/
```

Fix any wrong values by editing the files directly.

### Step 6 — Offer next steps

Report: files created, how name/URL were detected, any guessed values.
Then offer: `trellis import project .` to scan existing routes, `trellis serve` for web preview.
