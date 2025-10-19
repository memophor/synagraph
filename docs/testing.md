<!-- SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions. -->
# Testing Strategy

This document outlines the multi-layer testing approach for SynaGraph. It complements the developer setup and gRPC CLI guides.

## Layers

1. **Unit tests** (`cargo test` default target)
   - Scope: pure functions, domain models, helpers.
   - Examples: `KnowledgeNode::new`, payload parsing utilities.
   - Expectations: fast (<1s), deterministic, no network/filesystem dependencies.

2. **Integration tests** (`tests/` harness, async with Tokio)
   - Scope: HTTP/gRPC handlers, configuration bootstrapping, telemetry wiring.
   - Spin up in-process servers using `tokio::spawn` and hit them with `reqwest`/`tonic` clients.
   - Validate readiness/health endpoints, gRPC round trips, error codes.
   - First example: `tests/grpc_upsert.rs` exercises the `UpsertNode` flow via a real tonic client (`cargo test --test grpc_upsert`).

3. **Contract/API tests**
   - Scope: ensure protobuf-defined behaviour remains backwards compatible.
   - Tooling: `grpcurl` or `evans` CLI commands scripted via shell in CI.
   - Plan: add smoke scripts under `scripts/` that run against `cargo run -- --once` or a spawned binary.

4. **End-to-end scenarios** (future)
   - Once persistence/vector stores exist, orchestrate docker-compose services and run scenario tests covering upsert/query flows and policy enforcement.

5. **Static analysis**
   - `cargo fmt`, `cargo clippy -- -D warnings` already enforced via CI.
   - Consider adding `cargo deny` for dependency auditing in future phases.

## Command Reference

| Purpose             | Command                                                                 |
|---------------------|-------------------------------------------------------------------------| 
| Unit tests          | `cargo test`                                                            |
| Integration suite   | `cargo test --test <name>` (once tests land in `tests/`)                |
| Linting             | `cargo clippy --all-targets --all-features -- -D warnings`              |
| Formatting          | `cargo fmt` or `make fmt`                                               |
| gRPC contract check | `scripts/grpc_smoke.sh` (planned) or manual `evans`/`grpcurl` commands  |

## Upcoming Tasks

- [ ] Add `tests/http_ready.rs` verifying `/ready` responds `200` and toggles once dependencies are mocked.
- [ ] Create `tests/grpc_ping.rs` using Tonic client for roundtrip validation.
- [ ] Introduce `scripts/grpc_smoke.sh` that wraps Evans/`grpcurl` commands for CI use.
- [ ] Wire the script into GitHub Actions once it is idempotent.
- [ ] Explore property tests for temporal decay algorithms when implemented.

## Tips

- Use `cargo test -- --nocapture` during development to see handler logs.
- For CLI smoke tests, run the server in another terminal (`cargo run`) and execute the commands from [gRPC CLI Quickstart](grpc_cli.md).
- Keep tests hermetic: mock external systems until real dependencies are available in CI.
