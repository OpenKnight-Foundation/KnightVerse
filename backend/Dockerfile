FROM rust:slim-bookworm AS chef
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev build-essential file && \
    rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef
WORKDIR /usr/src/app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /usr/src/app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release -p api

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/app/target/release/api /app/api

# Create a non-root user
RUN useradd -m -s /bin/bash appuser
RUN chown -R appuser:appuser /app
USER appuser

ENV PORT=8080
EXPOSE 8080

CMD ["./api"]
