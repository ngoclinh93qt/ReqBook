---
description: Import a curl command from the clipboard or user input as a Trellis endpoint spec
---
Ask the user to paste a `curl` command (e.g. copied from browser DevTools → Copy as cURL).

Once you have the curl text, run:

```bash
echo '<CURL_COMMAND>' | trellis import curl
```

Or save it to a temp file and run `trellis import curl /tmp/curl.txt`.

After import:
- Show the path of the created spec file.
- Remind the user to set `baseUrl` in `api-docs/_shared/env.md` if it's a new host.
- Offer to run `trellis exec <new-file>` to verify the endpoint works.
