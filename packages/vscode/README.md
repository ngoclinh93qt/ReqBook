# MarkApiDown for VS Code

Preview, validate, run, and inspect MarkApiDown API specs from VS Code.

## Requirements

Install the `mad` binary first:

```bash
cargo install mark-api-down
# or
npm install -g mark-api-down
```

If `mad` is not on the VS Code process `PATH`, set `markapidown.madPath` to the absolute binary path.

## Commands

- `MarkApiDown: Preview Endpoint`
- `MarkApiDown: Run Endpoint`
- `MarkApiDown: Validate Current File`
- `MarkApiDown: Show Agent Context`

## Completion

Markdown completion suggests variables from:

- `api-docs/_shared/env.md`
- `.env.local`
- path params in the current spec
- `Capture: ... as <name>` directives in related flows

Completion is offered while editing `{{variable}}` templates and path params.

## Settings

- `markapidown.madPath`: path to the `mad` binary, default `mad`
- `markapidown.env`: default environment for run/context, default `dev`
- `markapidown.apiDocsRoot`: optional `api-docs` root override
- `markapidown.resultPanel`: show command output in a result panel
