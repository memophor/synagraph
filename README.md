# SynaGraph

SynaGraph is the open-source synaptic graph engine that powers the Memophor Knowlemesh platform. It combines graph storage, semantic vector search, temporal decay/reinforcement, and policy-aware provenance tracking.

## Features (scaffold)
- HTTP `/health` endpoint served via Axum for readiness checks.
- gRPC API (see `proto/synagraph.proto`) compiled with Tonic.
- Basic domain model for knowledge nodes and JSON payload handling.
- Shared configuration + telemetry setup for consistent logging.

## Getting Started

### Prerequisites
- Rust toolchain (`rustup` recommended)
- `protoc` compiler for regenerating gRPC bindings

### Run the service
```bash
cargo run
```
This will start the HTTP server on `0.0.0.0:8080` and the gRPC server on `0.0.0.0:50051`.

### Smoke test (HTTP)
```bash
curl http://localhost:8080/health
```

### Smoke test (gRPC)
```bash
evans --proto proto/synagraph.proto --host localhost --port 50051
```

## Project Layout
- `src/` – Runtime source code (HTTP + gRPC servers, domain models).
- `proto/` – Protobuf contracts shared with Knowlemesh and Scedge.
- `build.rs` – Compiles protobuf definitions at build time.

## License

Apache License 2.0. See `LICENSE` for details.

