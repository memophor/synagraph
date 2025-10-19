<!-- SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions. -->
# Developer Setup

This guide walks through preparing a local environment for hacking on SynaGraph.

## 1. Install Prerequisites

1. Install the Rust toolchain via [`rustup`](https://rustup.rs):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. Ensure the toolchain binaries are on your `PATH` (default `~/.cargo/bin`).
3. Install the Protocol Buffers compiler (`protoc`):
   - Debian/Ubuntu: `sudo apt install protobuf-compiler`
   - macOS (Homebrew): `brew install protobuf`
   - Windows (chocolatey): `choco install protoc`
   - Or download a prebuilt release from <https://github.com/protocolbuffers/protobuf/releases>.
4. Optional, but recommended: install [`just`](https://github.com/casey/just) or `make` if you prefer the provided task runner.

## 2. Clone and Toolchain Sync

```bash
git clone https://github.com/memophor/synagraph.git
cd synagraph
rustup show
```
The repository pins `stable` via `rust-toolchain.toml`; `rustup` will install it automatically on first build.

## 3. Bootstrap Checks

Run the baseline commands to confirm the environment is ready:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
All commands should complete without warnings or failures. If `protoc` is missing, install it and rerun the checks.
Refer to the [Testing Strategy](testing.md) for deeper coverage plans and additional targets as the suite evolves.

## 4. Running the Services

Launch the HTTP and gRPC servers simultaneously:

```bash
cargo run
```

Smoke-test the endpoints in another shell:

```bash
curl http://localhost:8080/health
curl http://localhost:8080/ready
evans --proto proto/synagraph.proto --host localhost --port 50051 repl
```

Inside the Evans REPL, select the package/service and invoke RPCs as documented in the [gRPC CLI Quickstart](grpc_cli.md).

## 5. Environment Configuration

Default bind addresses and metadata are controlled via environment variables:

| Variable        | Description                    | Default            |
|-----------------|--------------------------------|--------------------|
| `HTTP_ADDR`     | HTTP listener socket address   | `0.0.0.0:8080`     |
| `GRPC_ADDR`     | gRPC listener socket address   | `0.0.0.0:50051`    |
| `SERVICE_NAME`  | Service identifier in logs     | `synagraph`        |
| `SERVICE_VERSION` | Service version override     | crate package ver. |
| `RUST_LOG`        | Tracing verbosity             | `synagraph=info,tower_http=info` |

Create a `.env` at the project root to customize these when running locally.

## 6. IDE Tips

- Install the official Rust analyzer plugin for rich inline diagnostics.
- Enable `clippy` on save for faster feedback.
- Configure format-on-save to call `cargo fmt` or rust-analyzer formatting.

## 7. Troubleshooting

- If builds fail with missing `protoc`, ensure the binary is on the `PATH` or set `PROTOC=/absolute/path/to/protoc`.
- `cargo clean` can help resolve stale build artifacts.
- Run `RUST_LOG=debug cargo run` to increase logging verbosity during debugging.

Happy hacking!
