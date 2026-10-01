# syntax=docker/dockerfile:1

# Keep in sync with server/rust-toolchain.toml.
ARG RUST_VERSION=1.98.1

# Base images are pinned by digest; bump RUST_VERSION and the cargo-chef digest together.

FROM oven/bun:1@sha256:9114c058aeae42162ee16dd5084b95fe9473970bb6bcb5b232ab1630f0546895 AS web
WORKDIR /app/web
COPY locales/ /app/locales/
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web/ ./
RUN bun run build

FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION}-slim-trixie@sha256:38dfdbf4fda95c516f873f33032e490baa988b75f7d83c7d12f788f770785b36 AS chef
# Skips the rustfmt/clippy download that rust-toolchain.toml would trigger.
ARG RUST_VERSION
ENV RUSTUP_TOOLCHAIN=${RUST_VERSION}
WORKDIR /app/server

FROM chef AS planner
COPY server/ ./
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/server/recipe.json recipe.json
COPY proto/ /app/proto/
COPY locales/ /app/locales/
RUN cargo chef cook --release --locked --recipe-path recipe.json
COPY server/ ./
ENV SQLX_OFFLINE=true
RUN cargo build --release --locked -p api --bin api

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2
COPY --from=builder /app/server/target/release/api /usr/local/bin/api
COPY --from=web /app/web/build /srv/web
ENV BIND_ADDRESS=0.0.0.0:3000 \
    STATIC_DIR=/srv/web \
    LOG_FORMAT=json \
    RUST_LOG=info
USER nonroot
EXPOSE 3000
# No shell or curl in the image, so the binary checks itself.
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/api", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/api"]
CMD ["serve"]
