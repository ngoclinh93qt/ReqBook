---
description: Validate Trellis endpoint specs in a file or directory
---
Validate the Trellis spec(s) at $ARGUMENTS (defaults to `api-docs/` if not given).

```bash
trellis validate ${ARGUMENTS:-api-docs/}
```

Report: number of files checked, any validation errors with file paths and line references, and the exit code.
Exit 2 = invalid spec. Exit 5 = secret detected in a versioned file.
