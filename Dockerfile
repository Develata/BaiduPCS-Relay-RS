# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.91
FROM rust:${RUST_VERSION}-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --locked --release \
      --bin baidu-web-server \
      --bin baidu-direct-link \
    && mkdir -p /out \
    && cp target/release/baidu-web-server target/release/baidu-direct-link /out/

FROM debian:bookworm-slim AS runtime

COPY --from=builder /out/baidu-web-server /usr/local/bin/baidu-web-server
COPY --from=builder /out/baidu-direct-link /usr/local/bin/baidu-direct-link

ENV PORT=5200
EXPOSE 5200
USER 10001:10001
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/baidu-web-server"]
