# Benchmarks

Benchmarks must be captured before every release candidate.

Current status: release benchmark numbers have not been captured on clean release hardware yet. The table below is the release gate; fill the "Measured" column during the release candidate pass.

| Metric | Target | Measured | Command |
| --- | ---: | ---: | --- |
| Default binary size, stripped | < 10 MB | TBD | `make bench-size` |
| Minimal binary size, stripped | < 4 MB | TBD | `make bench-size-minimal` |
| Cold start, `mad --help` | < 20 ms | TBD | `make bench-cold-start` |
| `mad validate <file>` parser time | < 10 ms | TBD | `cargo bench --bench parse_endpoint` |
| Engine overhead excluding network | < 5 ms | TBD | targeted engine benchmark |
| Web first response | < 100 ms | TBD | `make bench-web` |
| File watcher to web refresh | < 100 ms | TBD | manual browser timing |
| Install via shell script | < 30 s | TBD | fresh macOS/Linux VM |

## Local benchmark commands

```bash
make release-check
cargo bench --bench parse_endpoint
cargo build --release --locked
ls -lh target/release/mad
```

## Release notes

Record:

- Machine model and OS
- Rust version
- Node version
- Git commit
- Build features
- Any variance or known bottlenecks
