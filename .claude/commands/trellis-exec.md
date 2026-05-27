---
description: Execute a Trellis endpoint spec and report the result
---
Run the Trellis endpoint spec specified in $ARGUMENTS and report the result.

```bash
trellis exec $ARGUMENTS --env=dev
```

Report: endpoint file, environment, method + URL (mask auth headers), HTTP status, duration, and whether the diff passed.
If no file is specified, search `api-docs/**/*.md` for an endpoint matching the user's description and run that.
On failure include the exit code, error message, and the fix suggestion from trellis output.
