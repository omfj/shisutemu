# syntax=docker/dockerfile:1

FROM rust:1.99.0-bookworm@sha256:fbc3a359627c6b5d9c8b20aae5c413a87392954f020006d7a9f7d95938964b23 AS builder

ARG TARGETARCH
ARG TAILWIND_VERSION=4.3.3
ARG TAILWIND_SHA256_X64=dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a
ARG TAILWIND_SHA256_ARM64=55fd0b241214eff3de1e8ee4f22796662f2d2e7a49bcfca7477cfd0bac398195
RUN case "$TARGETARCH" in \
        amd64) arch=x64; sha256="$TAILWIND_SHA256_X64" ;; \
        arm64) arch=arm64; sha256="$TAILWIND_SHA256_ARM64" ;; \
        *) echo "unsupported arch: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && curl -fsSL -o /usr/local/bin/tailwindcss \
        "https://github.com/tailwindlabs/tailwindcss/releases/download/v${TAILWIND_VERSION}/tailwindcss-linux-${arch}" \
    && echo "${sha256}  /usr/local/bin/tailwindcss" | sha256sum -c - \
    && chmod +x /usr/local/bin/tailwindcss

WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target,id=target-${TARGETARCH} \
    cargo build --release --locked \
    && cp target/release/shisutemu /usr/local/bin/shisutemu

FROM debian:bookworm-20261005-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /data shisutemu

COPY --from=builder /usr/local/bin/shisutemu /usr/local/bin/shisutemu

# For the history file
WORKDIR /data
VOLUME /data
USER shisutemu

ENV PORT=3000
EXPOSE 3000

ENTRYPOINT ["shisutemu"]
CMD ["/etc/shisutemu/config.toml"]
