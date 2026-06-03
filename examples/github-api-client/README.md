# GitHub API client example

This example uses public GitHub API endpoints so a new user can run a real
request without creating a local server or handling secrets.

```bash
mad validate api-docs/
mad exec api-docs/apis/repos/get-repository.md
mad flow api-docs/flows/repository-release-smoke.md
mad serve
```

Optional authenticated rate limits:

```bash
MAD_GITHUB_TOKEN=... mad exec api-docs/apis/repos/get-repository.md
```

Agent prompt:

```text
Use the GitHub API specs in examples/github-api-client/api-docs to add a
workflow that checks issues for a repo. Keep path variables in _shared/env.md and
add assertions that are stable for public repos.
```
