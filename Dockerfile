# The public backend needs Rust inference only; Python training stays offline.
FROM rust:1.96-bookworm AS build
WORKDIR /source
COPY rust/ rust/
RUN CARGO_BUILD_JOBS=2 cargo build --locked --release --manifest-path rust/Cargo.toml --bin quantctl

FROM python:3.14-slim-bookworm AS artifacts
WORKDIR /source
COPY scripts/deployment_bundle.py scripts/deployment_bundle.py
COPY deploy/artifacts.tar.gz deploy/artifacts.tar.json deploy/
RUN python scripts/deployment_bundle.py extract --output /artifacts

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home quant
COPY --from=build /source/rust/target/release/quantctl /usr/local/bin/quantctl
COPY --from=artifacts --chown=quant:quant /artifacts /app
USER quant
WORKDIR /app
ENV PORT=10000
EXPOSE 10000
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s --retries=3 \
    CMD curl --fail --silent "http://127.0.0.1:${PORT}/api/health" || exit 1
CMD ["quantctl", "serve", "--remote", "--workers", "1", "--root", "/app"]
