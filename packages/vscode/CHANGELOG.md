# Changelog

## 0.1.0 - 2026-06-03

Initial preview release.

- Preview Reqbook endpoint and flow markdown files inside VS Code.
- Auto-detect runnable endpoint and flow files and show inline CodeLens run buttons.
- Detect runnable files from any collection root with `reqbook.md` or `mad.md`, not only `api-docs/`.
- Auto-detect the local `rqb` binary from workspace builds and common install paths.
- Run the current endpoint or flow through the local `rqb` binary.
- Validate the current markdown file.
- Show agent-ready context from `rqb context`.
- Suggest variables from shared env docs, `.env.local`, path params, and flow captures.
- Render command results with status, timing, assertions, diff, and raw output.
