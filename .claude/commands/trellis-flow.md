---
description: Execute a Trellis pipeline and report step results
---
Run the Trellis pipeline specified in $ARGUMENTS and report each step's result.

```bash
trellis flow $ARGUMENTS --env=dev
```

Report: pipeline name, environment, each step's endpoint + status + diff outcome, and overall pass/fail.
If no file is specified, search `api-docs/flows/**/*.md` for a pipeline matching the user's description.
On failure include which step failed and the suggested fix.
