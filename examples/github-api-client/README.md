# GitHub API client example

This example uses public GitHub API endpoints so a new user can run a real
request without creating a local server or handling secrets.

```bash
rqb validate api-docs/
rqb exec api-docs/apis/repos/get-repository.md
rqb flow api-docs/flows/repository-release-smoke.md
rqb serve
```

Optional authenticated rate limits:

```bash
RQB_GITHUB_TOKEN=... rqb exec api-docs/apis/repos/get-repository.md
```

Agent prompt:

```text
Use the GitHub API specs in examples/github-api-client/api-docs to add a
workflow that checks issues for a repo. Keep path variables in _shared/env.md and
add assertions that are stable for public repos.
```
