FROM rust:1.90-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN cargo build --release --no-default-features --features minimal --locked

FROM alpine:3.20
RUN addgroup -S mad && adduser -S mad -G mad
COPY --from=builder /src/target/release/mad /usr/local/bin/mad
USER mad
WORKDIR /work
ENTRYPOINT ["mad"]
