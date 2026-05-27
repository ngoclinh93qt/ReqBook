---
description: Create or edit a Trellis workflow pipeline from existing endpoint specs
---
Create or edit a Trellis workflow in `api-docs/flows/` from the scenario in $ARGUMENTS.

First inspect existing endpoint specs:

```bash
rg --files api-docs | rg '\.md$'
rg -n '^method:|^path:|^# ' api-docs
```

Then write a markdown pipeline with frontmatter `type: pipeline`, ordered `## Steps`, `Capture`, `Inject`, and `Assert` directives.

After writing:

```bash
trellis validate api-docs/flows/<workflow>.md
trellis index
```

Report the workflow path, each endpoint step, captured values, injected values, and any endpoint specs that were missing.
Do not run the workflow unless the user explicitly asks to execute it.
