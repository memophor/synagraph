# syntax=docker/dockerfile:1.6

###############################################
## Dashboard build stage
###############################################
FROM node:20-bullseye AS dashboard-builder
WORKDIR /dashboard
COPY dashboard/package*.json ./
RUN npm ci
COPY dashboard/ ./
RUN npm run build

###############################################
## Rust build stage
###############################################
FROM rustlang/rust:nightly-bullseye AS rust-builder
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       pkg-config \
       libssl-dev \
       build-essential \
       protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY build.rs ./
# Create empty main to cache dependencies
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
COPY migrations ./migrations
COPY --from=dashboard-builder /dashboard/dist ./dashboard/dist
RUN cargo build --release

###############################################
## Runtime stage
###############################################
FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /app/target/release/synagraph ./synagraph
COPY --from=rust-builder /app/dashboard ./dashboard
COPY --from=rust-builder /app/migrations ./migrations

ENV RUST_LOG=info \
    SYNAGRAPH_HTTP_PORT=8080

EXPOSE 8080
CMD ["./synagraph"]
