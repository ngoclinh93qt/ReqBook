---
type: pipeline
name: repository-release-smoke
description: Check repository metadata and then inspect release shape.
continue-on-error: false
parallel: false
---
# Repository release smoke

This flow shows how public API documentation can become a small CI smoke test.

## Steps

1. **Get repository** -> `apis/repos/get-repository.md`
   - Inject: `owner`, `repo`
   - Assert: `response.status == 200`
2. **List releases** -> `apis/repos/get-releases.md`
   - Inject: `owner`, `repo`
   - Assert: `response.status == 200`
