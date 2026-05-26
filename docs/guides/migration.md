# Migration guide

This guide covers importing existing API specs from Postman, Insomnia, and OpenAPI into Trellis. For the full Trellis spec format see [Spec convention](../spec/convention.md). For CLI flags see [CLI reference](../cli.md).

## General approach

The import commands convert foreign formats into Trellis markdown endpoint files. The conversion is best-effort: structural information (URLs, methods, headers, bodies, status codes) maps cleanly, but dynamic behavior (pre-request scripts, dynamic variable generation, complex JavaScript assertions) cannot be mechanically converted and is imported as human-readable `agent-task` blocks for manual review.

After any import:

1. Run `trellis validate api-docs/` to catch structural issues.
2. Move secrets from imported files to `.env.local` or `TRELLIS_*` environment variables.
3. Review `agent-task` blocks for pre-request logic that needs manual implementation.
4. Run `trellis index` if the index is out of date (the import commands do this automatically).

---

## Migrating from Postman

Trellis imports Postman Collection v2.1 JSON exports.

### Concept mapping

| Postman concept | Trellis concept | Notes |
| --- | --- | --- |
| Collection | `api-docs/` project | Collection name and description become project prose. |
| Folder | Resource directory | Folder names become resource directories such as `users/` or `orders/`. |
| Request | Endpoint markdown file | One request becomes one `<method>-<slug>.md` file. |
| Request URL | `path` frontmatter + `http` request block | `{{var}}` syntax is preserved. |
| HTTP method | `method` frontmatter + request line | Normalized to uppercase. |
| Headers | Headers in `http` request block | Authorization headers are converted to variable references. |
| JSON body | Body in `http` request block | Formatted for readable diffs. |
| Test scripts (simple) | `## Expected response` block | Status code and simple body checks become expected responses. |
| Test scripts (complex) | `agent-task` block | JavaScript logic is converted to manual review instructions. |
| Pre-request script | `agent-task` block | Imported as review instructions, not executable code. |
| Collection variables | `_shared/variables.md` or `_shared/env.md` | Non-secret values can be committed. Secrets must move to `.env.local`. |
| Environment | `_shared/env.md` section | Each Postman environment becomes a `## <name>` section. |
| Auth helper (bearer/basic) | Endpoint or project `auth` setting | Maps directly when the auth type is `bearer` or `basic`. |
| Examples | `## Expected response` block | Postman saved examples become expected response candidates. |

### Step-by-step workflow

1. In Postman, export the collection: **File > Export > Collection v2.1**.
2. Export Postman environments separately if they contain non-secret base URLs.
3. Run the import:

```bash
trellis import postman my-collection.json
```

4. Validate the result:

```bash
trellis validate api-docs/
```

5. Review the output. Move any secrets detected by validation (exit code 5) to `.env.local`:

```dotenv
authToken=your-token-here
```

6. Regenerate the index (the import command does this, but re-run if you made manual edits):

```bash
trellis index
```

7. Review all generated `agent-task` blocks. Each one marks a place where Postman had JavaScript logic that needs manual implementation or verification.

### What is preserved

- URL structure including path parameters (`{{var}}` syntax).
- HTTP method.
- Request headers (with auth values replaced by variable references).
- Request body.
- Status code assertions (simple checks become `## Expected response`).
- Folder structure as resource directories.

### What needs manual review

| Item | Reason |
| --- | --- |
| Pre-request scripts | JavaScript cannot be converted to markdown. Imported as `agent-task` instructions. |
| Dynamic variables (`{{$timestamp}}`, `{{$randomEmail}}`) | Require explicit Trellis variables or pipeline captures. Replace with `--var` flags or `_shared/env.md` values. |
| Complex assertions | JavaScript test logic becomes `agent-task` items. Re-implement as `## Expected response` checks where possible. |
| Environment secrets | Postman environments containing tokens must move to `.env.local` or `TRELLIS_*` env vars. |
| Chained requests | Use a Trellis pipeline to chain requests and capture response values between steps. |

---

## Migrating from Insomnia

Trellis imports Insomnia v4 JSON exports.

### Concept mapping

| Insomnia concept | Trellis concept | Notes |
| --- | --- | --- |
| Workspace | `api-docs/` project | Workspace name becomes project name. |
| Request group (folder) | Resource directory | Group names become resource directories. |
| Request | Endpoint markdown file | One request becomes one `<method>-<slug>.md` file. |
| URL | `path` frontmatter + `http` request block | Template variables (`{{ var }}`) are converted to Trellis `{{var}}` syntax. |
| Method | `method` frontmatter + request line | |
| Headers | Headers in `http` request block | |
| Body | Body in `http` request block | |
| Environment | `_shared/env.md` section | Base and sub-environments map to `## <name>` sections. |
| Environment variables | `_shared/env.md` or `.env.local` | Non-secret values go to `env.md`. Secrets go to `.env.local`. |
| Test results | `## Expected response` block | Simple status checks are preserved. |
| Plugins / scripts | `agent-task` block | Imported as manual review instructions. |

### Step-by-step workflow

1. In Insomnia, export the workspace: **Application > Preferences > Data > Export Data > Current Workspace**.
2. Select **Insomnia v4** format and save the JSON file.
3. Run the import:

```bash
trellis import insomnia insomnia_export.json
```

4. Validate:

```bash
trellis validate api-docs/
```

5. Move any flagged secrets to `.env.local`.
6. Review `agent-task` blocks for plugin logic and template tag usage that needs manual follow-up.

### What is preserved

- URL structure and template variables.
- HTTP method, headers, and body.
- Environment structure.
- Request grouping as resource directories.

### What needs manual review

| Item | Reason |
| --- | --- |
| Template tags (`{% now %}`, `{% uuid %}`) | Insomnia-specific tags have no direct Trellis equivalent. Replace with pipeline captures or `--var` flags. |
| Plugin-based pre/post-request logic | Imported as `agent-task` instructions. |
| Environment secrets | Must move to `.env.local` or `TRELLIS_*` env vars. |
| OAuth 2.0 flows | Multi-step auth flows should become a Trellis pipeline that captures the token and injects it into subsequent steps. |

---

## Migrating from OpenAPI

Trellis imports OpenAPI 3.x specs in YAML or JSON format.

### Concept mapping

| OpenAPI concept | Trellis concept | Notes |
| --- | --- | --- |
| `info.title` | Project name in `trellis.md` | |
| `servers[0].url` | `baseUrl` in `_shared/env.md` | Additional servers become additional environment sections. |
| `paths.<path>.<method>` | Endpoint markdown file | One operation becomes one `<method>-<slug>.md` file. |
| `operationId` | File slug | Used to generate the filename. Falls back to method + path. |
| `tags` | Resource directory and `tags` frontmatter | The first tag becomes the resource directory. |
| `parameters` (path/query/header) | Variables in the `http` request block | Path params use `:param` syntax. |
| `requestBody` | Body in `http` request block | First example or schema is used. |
| `responses.<code>` | `## Expected response` block | The first documented response code is used. |
| `security` | `auth` frontmatter | `bearerAuth` maps to `auth: bearer`. `basicAuth` maps to `auth: basic`. |
| `components/schemas` | Not directly imported | Schema documentation is preserved as prose in `## Notes`. |

### Step-by-step workflow

1. Ensure your OpenAPI file is valid 3.x YAML or JSON. Use a linter such as `spectral lint` if needed.
2. Run the import:

```bash
trellis import openapi openapi.yaml
# or
trellis import openapi openapi.json
```

3. Validate:

```bash
trellis validate api-docs/
```

4. Move any secrets detected in `env.md` to `.env.local`.
5. Review generated `agent-task` blocks. OpenAPI specs often have rich schema validation logic that becomes review tasks.
6. Add environment sections to `_shared/env.md` for any additional servers defined in the OpenAPI spec.

### What is preserved

- Path structure and path parameters.
- HTTP method.
- Request headers and query parameters (as variable references).
- Request body (first example or schema stub).
- Response status code.
- Security scheme type.
- Operation tags.

### What needs manual review

| Item | Reason |
| --- | --- |
| Multiple response codes | Only the first documented response is used as `## Expected response`. Add others manually in `## Notes`. |
| JSON Schema validation | Schema constraints are not executable in Trellis v1.0.0. Add as `agent-task` assertions. |
| `$ref` components | Schema references are resolved during import but not recursively expanded. Review complex nested schemas. |
| OAuth 2.0 / OpenID Connect flows | Model as a Trellis pipeline that captures the token. |
| Callbacks and webhooks | `protocol: ws` and `protocol: sse` are reserved; callback URLs must be documented manually. |
| Multiple servers | The first server becomes `dev` in `env.md`. Add the others as additional environments manually. |

---

## Post-import checklist

After running any import command, verify the following before using the specs in CI or sharing with your team.

- [ ] `trellis validate api-docs/` exits with code 0.
- [ ] No secrets in `api-docs/` — any secret detected (exit code 5) must be moved to `.env.local` or a `TRELLIS_*` environment variable.
- [ ] `.env.local` is listed in `.gitignore` (`trellis doctor` verifies this).
- [ ] All `agent-task` blocks reviewed — each one marks a place where the original spec had logic that needs manual attention.
- [ ] `trellis exec` runs successfully against the development environment with your local `.env.local` in place.
- [ ] For chained requests, create a pipeline file in `api-docs/pipelines/` using `Capture` and `Inject` directives. See the pipeline format in [Spec convention](../spec/convention.md#pipelines).
- [ ] Run `trellis index` if you added or renamed files manually after the import.
