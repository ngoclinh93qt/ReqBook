.PHONY: release-check fmt clippy test web-build validate bench-size bench-size-minimal bench-cold-start bench-web

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
	cargo run -- validate api-docs
	cargo run -- validate examples/jsonplaceholder/api-docs

bench-size:
	cargo build --release --locked
	ls -lh target/release/trellis

bench-size-minimal:
	cargo build --release --locked --no-default-features --features minimal
	ls -lh target/release/trellis

bench-cold-start:
	hyperfine --warmup 5 './target/release/trellis --help'

bench-web:
	cargo run --release -- serve --port 7700 --host 127.0.0.1
