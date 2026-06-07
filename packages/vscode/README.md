# Reqbook for VS Code

Preview, validate, run, and inspect Reqbook API specs from VS Code.

## Requirements

Install the `rqb` binary first:

```bash
cargo install reqbook
# or
npm install -g reqbook
```

The extension auto-detects `rqb` from `RQB_PATH`, the workspace `target/debug` or `target/release` directories, Cargo's `~/.cargo/bin`, Homebrew paths, and the VS Code process `PATH`. Set `reqbook.rqbPath` only when you need an explicit override.

## Demo

Watch the official VS Code demo in the docs: https://docs.markapidown.net/guides/vscode-extension#demo

## Commands

- `Reqbook: Preview Endpoint`
- `Reqbook: Run Spec`
- `Reqbook: Validate Current File`
- `Reqbook: Show Agent Context`

## Auto-detect run buttons

When you open a runnable Reqbook file, the extension shows a CodeLens run button at the top of the editor and a Run button in the editor title bar.

- The collection root is the nearest parent directory with `reqbook.md` or `mad.md`; it does not need to be named `api-docs`.
- Endpoint specs with `method:` and `path:` frontmatter run with `rqb exec`.
- Flow and pipeline specs under `<collection>/flows/` or `<collection>/pipelines/` run with `rqb flow`.
- Collection docs such as `<collection>/reqbook.md`, `<collection>/mad.md`, `README.md`, and `_shared/env.md` do not show run buttons.

## Completion

Markdown completion suggests variables from:

- `<collection>/_shared/env.md`
- `.env.local`
- path params in the current spec
- `Capture: ... as <name>` directives in related flows and pipelines

Completion is offered while editing `{{variable}}` templates and path params.

## Settings

- `reqbook.rqbPath`: optional path to the `rqb` binary, default `rqb`
- `reqbook.env`: default environment for run/context, default `dev`
- `reqbook.apiDocsRoot`: optional collection root override
- `reqbook.resultPanel`: show command output in a result panel

## Install from VSIX

For pre-release builds, install the packaged VSIX from the command line:

```bash
code --install-extension reqbook-vscode-0.1.5.vsix
```

Then open a workspace with a Reqbook collection and run `Reqbook: Validate Current File` against an endpoint markdown file.

## Release package

```bash
npm ci
npm test
npm run check
npm run package -- --out /tmp/reqbook-vscode-0.1.5.vsix
```

See the [publishing checklist](https://github.com/ngoclinh93qt/ReqBook/blob/main/packages/vscode/PUBLISHING.md) for Marketplace publish steps.
