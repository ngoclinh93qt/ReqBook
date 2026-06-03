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
npm run package -- --out /tmp/reqbook-vscode-0.1.0.vsix
```

Install the generated VSIX in VS Code and smoke test these commands against a real `api-docs/` workspace:

- `Reqbook: Preview Endpoint`
- `Reqbook: Run Endpoint`
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

## Optional Open VSX

Open VSX uses a separate namespace/token flow.

```bash
cd packages/vscode
npx ovsx publish /tmp/reqbook-vscode-0.1.0.vsix
```

Do not commit generated `.vsix` artifacts or personal access tokens.
