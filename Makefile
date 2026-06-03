.PHONY: release-check fmt clippy test web-build validate bench bench-agent-token bench-size bench-size-minimal bench-cold-start bench-web

release-check: fmt clippy test web-build validate

fmt:
	cargo fmt --check

clippy:
	cargo clippy --locked -- -D warnings

test:
	cargo test --locked
	cargo test --locked --no-default-features --features minimal

web-build:
	cd web && npm ci && npm run build

validate:
	cargo run --bin rqb -- validate api-docs
	cargo run --bin rqb -- validate examples/jsonplaceholder/api-docs
	cargo run --bin rqb -- validate examples/agent-token-api/api-docs

bench: web-build
	cargo build --release --locked
	node scripts/benchmark.mjs

bench-agent-token:
	node scripts/codex-token-benchmark.mjs

bench-size:
	cargo build --release --locked
	ls -lh target/release/rqb

bench-size-minimal:
	cargo build --release --locked --no-default-features --features minimal
	ls -lh target/release/rqb

bench-cold-start:
	hyperfine --warmup 5 './target/release/rqb --help'

bench-web:
	cargo run --release --bin rqb -- serve --port 7700 --host 127.0.0.1
