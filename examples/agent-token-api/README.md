# Agent token benchmark API

Small local API fixture for measuring how many tokens an agent spends when it
discovers endpoint behavior from source files versus Reqbook specs.

The benchmark does not need this server to run; the source files are the
source-only corpus and `api-docs/` is the Reqbook-assisted corpus.

```bash
# Optional: run the sample API locally.
npm start

# Validate the Reqbook specs.
rqb validate api-docs/
```

## Endpoint

| Method | Path | Purpose |
| --- | --- | --- |
| POST | /v1/refunds/quote | Calculate a refund quote for support workflows |
