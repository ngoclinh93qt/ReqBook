# Configuration reference

This page documents the project config file, the environment config file, variable resolution, secret detection, auth modes, and retry policy. For the full endpoint and pipeline format see [Spec convention](spec/convention.md). For CLI flags see [CLI reference](cli.md).

## Project config: `api-docs/trellis.md`

`api-docs/trellis.md` is the project configuration file. It uses YAML frontmatter and structured YAML code blocks inside markdown sections.

### Frontmatter

```yaml
---
name: my-api
version: 1
default-env: dev
---
```

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `name` | yes | string | Project name shown in the CLI, web preview, reports, and generated index. |
| `version` | yes | integer | Trellis spec format version. Must be `1` for v1.0.0. |
| `default-env` | yes | string | Environment used when a command does not receive `--env`. |

Unknown frontmatter keys produce a warning and are ignored. This forward-compatible behavior allows future versions to add fields without breaking older clients.

### Complete example

````markdown
---
name: my-api
version: 1
default-env: dev
---

# My API

One-paragraph description of the project.

## Defaults

```yaml
timeout: 5000
retry:
  attempts: 3
  backoff: exponential
auth: bearer
```

## Web preview

```yaml
port: 8080
host: 127.0.0.1
theme: auto
autosave: 2s
```

## Plugins

```yaml
plugins: []
```

## Notes

Free-form team notes and conventions. The parser ignores this section.
````

### Defaults section

The `## Defaults` code block sets project-wide defaults for every endpoint. Individual endpoint frontmatter may override these values. CLI flags (`--timeout`) override both.

| Key | Type | Built-in default | Description |
| --- | --- | --- | --- |
| `timeout` | integer (ms) | `5000` | Request timeout in milliseconds. |
| `retry.attempts` | integer | `0` | Number of retry attempts after the first failure. `0` means no retry. |
| `retry.backoff` | enum | `fixed` | Backoff strategy: `fixed` or `exponential`. |
| `auth` | enum | `none` | Default auth mode: `none`, `bearer`, `basic`, or `custom`. |

If `## Defaults` is absent, the built-in defaults are used.

### Web preview section

The `## Web preview` code block controls `trellis serve`.

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `port` | integer | `8080` | TCP port for the preview server. |
| `host` | string | `127.0.0.1` | Host to bind to. |
| `theme` | string | `auto` | Color theme: `auto`, `light`, or `dark`. |
| `autosave` | string | `2s` | Debounce interval for live-reload after file changes. |

CLI flags (`--port`, `--host`) override these values.

### Plugins section

```yaml
plugins: []
```

Trellis v1.0.0 does not execute plugins. The `## Plugins` section is reserved for future use. Keep it as an empty list or omit the section.

---

## Environment config: `api-docs/_shared/env.md`

`api-docs/_shared/env.md` stores non-secret values for each environment. Each environment is a second-level heading (`## dev`, `## staging`, `## prod`) followed by a `yaml` code block.

### Example

````markdown
# Environments

## dev

```yaml
baseUrl: http://localhost:8080
userId: 123
pageSize: 20
```

## staging

```yaml
baseUrl: https://staging.example.com
userId: 456
```

## prod

```yaml
baseUrl: https://api.example.com
```
````

The environment name passed to `--env` must match one of these headings exactly. If the heading is missing, Trellis exits with code 2.

**Do not put secrets in `env.md`.** Tokens, passwords, private keys, and production credentials must go in `.env.local` or be passed via `TRELLIS_*` environment variables. The parser enforces this at validation time and exits with code 5 if a secret pattern is detected.

---

## Variable resolution priority

When the same variable name is defined in more than one source, the highest-priority source wins. The order from highest to lowest:

| Priority | Source | Example |
| --- | --- | --- |
| 1 (highest) | Pipeline step capture | `Capture: response.body.id as userId` |
| 2 | CLI `--var` flag | `--var userId=42` |
| 3 | Endpoint frontmatter | `userId: 42` in the endpoint's YAML block |
| 4 | `_shared/env.md` for the selected env | `## dev` block with `userId: 123` |
| 5 | `.env.local` | `authToken=local-dev-token` |
| 6 (lowest) | OS environment variables (`TRELLIS_*`) | `TRELLIS_USER_ID=99` |

### Variable syntax

Use `{{name}}` for inline variables in request blocks, response blocks, and pipeline definitions:

```http
GET {{baseUrl}}/users/{{userId}}
Authorization: Bearer {{authToken}}
```

Use `:param` for path parameters in URL paths:

```http
GET {{baseUrl}}/users/:userId
```

Both `{{userId}}` and `:userId` resolve from the same variable sources.

### OS environment variables

Only variables prefixed with `TRELLIS_` are read from the OS environment. The prefix is stripped and the remaining name is converted to lower camel case.

| OS variable | Trellis variable |
| --- | --- |
| `TRELLIS_AUTH_TOKEN` | `authToken` |
| `TRELLIS_BASE_URL` | `baseUrl` |
| `TRELLIS_USER_ID` | `userId` |

### `.env.local`

`.env.local` uses standard dotenv syntax. It must be listed in `.gitignore` (Trellis adds it automatically on `trellis init` and warns if missing during `trellis doctor`).

```dotenv
authToken=local-development-token
webhookSecret=local-development-secret
```

### Missing variables

If a variable is referenced and cannot be resolved from any source, Trellis exits with code 2 before making any network request:

```text
api-docs/users/get-user.md: unresolved variable "authToken"
Fix: define authToken in .env.local, pass --var authToken=..., or set TRELLIS_AUTH_TOKEN.
```

### Nested variables

Nested variable references are not supported in v1.0.0. If resolving `{{baseUrl}}` produces a string that contains another `{{variable}}`, Trellis returns an error rather than performing a second pass. This prevents unintended secret leakage through double-resolution.

---

## Secret detection

Trellis refuses secrets in versioned markdown files (`env.md`, endpoint files, pipeline files, `trellis.md`). If a secret pattern is detected during `trellis validate` or before execution, the command exits with code 5.

### Patterns that trigger exit code 5

| Pattern | Description |
| --- | --- |
| Strings starting with `Bearer eyJ` | JWT-like bearer tokens in plain text. |
| Hex strings longer than 32 characters | API keys and secrets commonly encoded as hex. |
| Values with prefix `sk_` | Stripe-style secret keys. |
| Values with prefix `pk_live_` | Stripe-style live publishable keys. |

The error message includes the file path and line number:

```text
api-docs/_shared/env.md:12: possible secret detected
Fix: move this value to .env.local or TRELLIS_* environment variables.
```

### Allowed secret locations

| Location | Notes |
| --- | --- |
| `.env.local` | Local machine only. Must be in `.gitignore`. |
| `TRELLIS_*` OS environment variables | Recommended for CI pipelines. |
| Secret manager integration | Use `TRELLIS_AUTH_TOKEN=$(vault read ...)` patterns. |

### Output masking

Trellis masks auth values in all output: CLI console, JSON reports, JUnit XML, markdown reports, and the web preview response history.

| Input | Masked output |
| --- | --- |
| `Authorization: Bearer abc123` | `Authorization: Bearer ****` |
| `Authorization: Basic dXNlcjpwYXNz` | `Authorization: Basic ****` |
| `authToken=abc123` | `authToken=****` |

---

## Auth modes

Set the auth mode in endpoint frontmatter or in `trellis.md`'s `## Defaults` section.

| Mode | Description |
| --- | --- |
| `none` | No authorization header added. |
| `bearer` | Adds `Authorization: Bearer {{authToken}}`. Requires `authToken` to be resolved. |
| `basic` | Adds `Authorization: Basic <base64(username:password)>`. Requires `username` and `password` variables. |
| `custom` | The request block must include the `Authorization` header explicitly. Trellis does not inject anything. |

The endpoint frontmatter `auth` value overrides the project default for that endpoint. If no `auth` is set at either level, the built-in default is `none`.

---

## Retry policy

Configure retries in `trellis.md`'s `## Defaults` section or in individual endpoint frontmatter.

```yaml
retry:
  attempts: 3
  backoff: exponential
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `attempts` | integer | `0` | Number of retry attempts after the first failure. `0` means no retry. |
| `backoff` | enum | `fixed` | `fixed`: retry immediately with no added delay. `exponential`: double the wait time between each attempt. |

Retries apply to network errors (exit code 4) and to 5xx responses. They do not retry on test assertion failures (exit code 1) or spec errors (exit code 2).

The `--timeout` CLI flag sets the per-attempt timeout. If all attempts exhaust the timeout, Trellis exits with code 4.
