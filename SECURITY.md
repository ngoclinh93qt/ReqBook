# Security policy

## Supported versions

Trellis is preparing its first public release. Security fixes target the latest released version and the current `main` branch.

## Reporting a vulnerability

Please do not open public issues for vulnerabilities.

Report security issues by emailing:

```text
security@trellis.dev
```

Include:

- Affected version or commit
- Operating system and install method
- Reproduction steps
- Impact assessment
- Any logs or sample files with secrets removed

We aim to acknowledge reports within 72 hours and provide a remediation plan after triage.

## Scope

In scope:

- Secret leakage in logs, reports, web preview, or generated files
- Path traversal in web preview or importers
- Unsafe install or release scripts
- Supply-chain issues in release artifacts
- Vulnerabilities in parser, resolver, execution, or flow handling

Out of scope:

- Denial of service from intentionally huge local files
- Issues requiring malicious local filesystem access
- Vulnerabilities in third-party APIs executed by user-authored specs

## Security expectations

- Trellis should mask Authorization headers and known secret patterns.
- `api-docs/_shared/env.md` must not contain secrets.
- `.env.local` should be gitignored and used for local sensitive values.
- The web preview binds to `127.0.0.1` by default.
