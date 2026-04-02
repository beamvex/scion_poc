# syntax=docker/dockerfile:1

FROM rust:1.85-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/scion_router_proto /usr/local/bin/scion-router-proto

EXPOSE 3000/tcp
EXPOSE 4001/udp
EXPOSE 4010/udp

ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/scion-router-proto"]
