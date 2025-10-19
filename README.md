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

### Getting Started Docs
- [Developer Setup](docs/development.md) – toolchain install, environment prep, and smoke tests.
- [Testing Strategy](docs/testing.md) – testing layers, commands, and upcoming coverage tasks.
- [gRPC CLI Quickstart](docs/grpc_cli.md) – step-by-step guide for driving the API with Evans.
- [Storage Architecture Plan](docs/storage_plan.md) – Phase 1 persistence design and repository interfaces.
- [Observability Roadmap](docs/observability.md) – planned metrics, tracing, and readiness enhancements.

### Dev Workflows

Common commands are captured in the `Makefile`:

```bash
make fmt   # cargo fmt
make lint  # cargo clippy -- -D warnings
make test  # cargo test
```

### CI Status

Pull requests must pass the GitHub Actions workflow (`.github/workflows/ci.yml`) which runs formatting, clippy linting, and the test suite against `stable` Rust.

### Run the service
```bash
cargo run
```
This will start the HTTP server on `0.0.0.0:8080` and the gRPC server on `0.0.0.0:50051`.

### Smoke test (HTTP)
```bash
curl http://localhost:8080/health
curl http://localhost:8080/ready
```

### Smoke test (gRPC)
```bash
evans --proto proto/synagraph.proto --host localhost --port 50051 repl
```
Then follow the steps in the [gRPC CLI Quickstart](docs/grpc_cli.md) to select the package/service and invoke RPCs.

### Telemetry

Structured logs default to `synagraph=info,tower_http=info`. Override with `RUST_LOG` when running locally, e.g. `RUST_LOG=debug cargo run`.

## Project Layout
- `src/` – Runtime source code (HTTP + gRPC servers, domain models).
- `proto/` – Protobuf contracts shared with Knowlemesh and Scedge.
- `build.rs` – Compiles protobuf definitions at build time.

## License

Apache License 2.0. See `LICENSE` for details.
