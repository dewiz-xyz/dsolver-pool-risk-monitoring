# ── Build stage ────────────────────────────────────────────────────
FROM rust:1.85-bookworm AS builder

WORKDIR /app

# Cache dependencies by building a dummy project first
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Build the real project
COPY src/ src/
COPY migrations/ migrations/
RUN touch src/main.rs src/lib.rs && \
    RUSTFLAGS="-C target-cpu=generic -C link-arg=-s" cargo build --release

# ── Runtime stage ─────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 1000 app && \
    useradd --uid 1000 --gid app --create-home app

COPY --from=builder /app/target/release/tycho-simulator-server_risk-monitoring /usr/local/bin/app
COPY config.json /etc/app/config.json

USER app

ENV CONFIG_PATH=/etc/app/config.json
ENV RUST_LOG=info

EXPOSE 3000

ENTRYPOINT ["app"]
