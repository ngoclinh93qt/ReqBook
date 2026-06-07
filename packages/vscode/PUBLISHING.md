# Publishing the VS Code extension

This package publishes the Reqbook VS Code extension from `packages/vscode`.

## Prerequisites

- A Visual Studio Marketplace publisher that matches the `publisher` field in `package.json`.
- A Marketplace personal access token with permission to manage extensions for that publisher.
- A released `rqb` binary that matches the extension documentation.

## Preflight

```bash
cd packages/vscode
npm ci
npm test
npm run check
npm run package -- --out /tmp/reqbook-vscode-0.1.4.vsix
```

Install the generated VSIX in VS Code and smoke test these commands against a real Reqbook collection:

- `Reqbook: Preview Endpoint`
- `Reqbook: Run Spec`
- `Reqbook: Validate Current File`
- `Reqbook: Show Agent Context`

Also verify the failure path by setting `reqbook.rqbPath` to a missing binary.

## Publish

```bash
cd packages/vscode
npm exec -- vsce login reqbook
npm run publish
```

If the Marketplace publisher is not `reqbook`, update `publisher` in `package.json` before packaging and publishing.

## GitHub Actions

The repository release workflow packages the VSIX on every `v*` tag and attaches it to the GitHub Release. Marketplace publishing is gated by repository variables so release tags stay safe before tokens are configured.

| Channel | Repository variable | Secret |
|---|---|---|
| Visual Studio Marketplace | `PUBLISH_VSCODE=true` | `VSCE_PAT` |
| Open VSX | `PUBLISH_OPEN_VSX=true` | `OVSX_PAT` |

## Optional Open VSX

Open VSX uses a separate namespace/token flow.

```bash
cd packages/vscode
npm exec -- ovsx publish /tmp/reqbook-vscode-0.1.4.vsix -p "$OVSX_PAT"
```

Do not commit generated `.vsix` artifacts or personal access tokens.
