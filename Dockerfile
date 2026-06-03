FROM rust:1.90-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN cargo build --release --no-default-features --features minimal --locked

FROM alpine:3.20
RUN addgroup -S rqb && adduser -S rqb -G rqb
COPY --from=builder /src/target/release/rqb /usr/local/bin/rqb
USER rqb
WORKDIR /work
ENTRYPOINT ["rqb"]
