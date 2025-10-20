<div align="center">

# 🧠 SynaGraph

**Synaptic Graph Engine for AI Knowledge**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![gRPC](https://img.shields.io/badge/gRPC-Tonic-green.svg)](https://github.com/hyperium/tonic)

*Graph + vector + temporal knowledge engine for distributed AI systems*

[Getting Started](#-getting-started) •
[Documentation](#-documentation) •
[Architecture](#-architecture) •
[Contributing](#-contributing) •
[Community](#-community)

</div>

---

## Overview

**SynaGraph** is the open-source synaptic graph engine that powers the Memophor Knowlemesh platform. It combines **graph storage**, **semantic vector search**, **temporal decay/reinforcement**, and **policy-aware provenance tracking** to create a living knowledge mesh for AI systems.

### Key Features

| Feature | Description |
|---------|-------------|
| 🔗 **Graph Storage** | Native graph model for knowledge relationships |
| 🎯 **Semantic Search** | Vector embeddings with pgvector for similarity lookup |
| ⏰ **Temporal Intelligence** | Decay and reinforcement of knowledge over time |
| 🔐 **Policy-Aware** | Built-in provenance tracking and governance |
| 🚀 **gRPC API** | High-performance Tonic-based gRPC interface |
| 📊 **Observable** | Structured logging and telemetry-ready |

### Part of the Memophor Knowledge Mesh

SynaGraph is the **graph layer** of the Memophor platform:

| Component | Role |
|-----------|------|
| **SynaGraph** | Graph + vector + temporal knowledge engine — *this repository* |
| **[Knowlemesh](https://github.com/memophor/knowlemesh)** | Orchestration and governance control plane |
| **[Scedge Core](https://github.com/memophor/scedge-core)** | Smart edge cache for low-latency delivery |
| **[SeTGIN](https://github.com/memophor/setgin)** | Self-tuning intelligence network |

---

## 🚀 Getting Started

### Quick Start (5 minutes)

```bash
make fmt   # cargo fmt
make lint  # cargo clippy -- -D warnings
make test  # cargo test
make migrate # cargo sqlx migrate run
make prepare # cargo sqlx prepare -- --all-targets --all-features
make ui-build # npm install && npm run build (dash dashboard)
```

# 2. Clone and setup SynaGraph
git clone https://github.com/memophor/synagraph.git
cd synagraph
cp .env.example .env  # Configure database connection

# 3. Run migrations
cargo sqlx migrate run

# 4. Start the service
cargo run
```
This will start the HTTP server on `0.0.0.0:8080` and the gRPC server on `0.0.0.0:50051`.
Provide `DATABASE_URL` to enable the PostgreSQL repository; without it, the service falls back to an in-memory store.
Run `docker compose up` to start the local Postgres (pgvector) + Redis stack, then apply migrations with `cargo sqlx migrate run` before `cargo run`.
Compose reads `.env`, so customize `POSTGRES_PORT`/`REDIS_PORT` (defaults: 55432/6379) if the host ports are occupied and update your `DATABASE_URL` (e.g. `postgres://postgres:postgres@localhost:55432/synagraph`).
The admin dashboard is available at `/dashboard` when `dashboard/dist` exists—build it via `cd dashboard && npm install && npm run build`, or run `npm run dev` for a live UI during development.

# 5. Smoke test HTTP endpoint
curl http://localhost:8080/health
curl http://localhost:8080/ready

# 6. Test gRPC API
evans --proto proto/synagraph.proto --host localhost --port 50051 repl
```

**🎨 See [gRPC CLI Quickstart](docs/grpc_cli.md) for interactive API testing**

### Prerequisites

- **Rust 1.75+** ([Install](https://rustup.rs/))
- **protoc** compiler for gRPC bindings
- **PostgreSQL 15+** with pgvector extension
- **Redis 7+** (optional, for caching layer)
- **Docker** (optional, for containerized development)

---

## 📚 Documentation

- **[Developer Setup](docs/development.md)** - Toolchain install, environment prep, smoke tests
- **[Vision & Platform Principles](docs/vision.md)** - Product pillars and architectural direction
- **[Testing Strategy](docs/testing.md)** - Testing layers, commands, and coverage tasks
- **[gRPC CLI Quickstart](docs/grpc_cli.md)** - Step-by-step guide for driving the API with Evans
- **[Storage Architecture Plan](docs/storage_plan.md)** - Phase 1 persistence design and repository interfaces
- **[Observability Roadmap](docs/observability.md)** - Planned metrics, tracing, and readiness enhancements

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────┐
│   Client / Knowlemesh / AI Agent        │
└───────────────┬─────────────────────────┘
                │ gRPC / HTTP
                ▼
┌─────────────────────────────────────────┐
│          SynaGraph Engine               │
│  ┌─────────────────────────────────┐   │
│  │  gRPC API (Tonic)                │   │
│  │  • Knowledge node operations     │   │
│  │  • Vector similarity search      │   │
│  │  • Graph traversal               │   │
│  └─────────────────────────────────┘   │
│  ┌─────────────────────────────────┐   │
│  │  Domain Layer                    │   │
│  │  • Knowledge nodes & edges       │   │
│  │  • Temporal decay/reinforcement  │   │
│  │  • Policy enforcement            │   │
│  └─────────────────────────────────┘   │
│  ┌─────────────────────────────────┐   │
│  │  Storage Layer                   │   │
│  │  • PostgreSQL + pgvector         │   │
│  │  • Graph persistence             │   │
│  │  • Vector indexing               │   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
                │
                │ Telemetry
                ▼
         [Observability Stack]
```

### Core Components

- **gRPC API** - High-performance Tonic-based service (`proto/synagraph.proto`)
- **HTTP Endpoints** - Health checks and readiness probes via Axum
- **Domain Model** - Knowledge nodes, edges, and JSON payload handling
- **Storage Layer** - PostgreSQL with pgvector for hybrid graph+vector storage
- **Telemetry** - Structured logging with tracing-subscriber

---

## 📦 Installation

### From Source

```bash
git clone https://github.com/memophor/synagraph.git
cd synagraph
cargo build --release
./target/release/synagraph
```

### With Docker

```bash
docker build -t synagraph:latest .
docker run -p 8080:8080 -p 50051:50051 \
  -e DATABASE_URL=postgres://postgres:postgres@db:5432/synagraph \
  synagraph:latest
```

### With Docker Compose

```bash
docker compose up
```

This starts:
- PostgreSQL 15 with pgvector (port 55432)
- Redis 7 (port 6379)
- SynaGraph service (HTTP: 8080, gRPC: 50051)

**📝 Note:** Customize ports in `.env` if defaults conflict:
```bash
POSTGRES_PORT=55432
REDIS_PORT=6379
DATABASE_URL=postgres://postgres:postgres@localhost:55432/synagraph
```

---

## 🔧 Configuration

Configure via environment variables (see [.env.example](.env.example)):

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | - | PostgreSQL connection string (required) |
| `HTTP_PORT` | `8080` | HTTP server port |
| `GRPC_PORT` | `50051` | gRPC server port |
| `RUST_LOG` | `synagraph=info` | Log level filter |
| `POSTGRES_PORT` | `55432` | Docker Compose PostgreSQL port |
| `REDIS_PORT` | `6379` | Docker Compose Redis port |

---

## 🌐 API Endpoints

### HTTP Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Basic health check |
| `GET` | `/ready` | Readiness probe (checks DB connection) |

### gRPC API

See [`proto/synagraph.proto`](proto/synagraph.proto) for the complete service definition.

**Key RPCs:**
- `CreateKnowledgeNode` - Store new knowledge with embeddings
- `QueryKnowledgeNodes` - Vector similarity search
- `TraverseGraph` - Navigate knowledge relationships
- `ApplyTemporalDecay` - Age knowledge based on usage patterns

**📖 See [gRPC CLI Quickstart](docs/grpc_cli.md) for interactive examples**

---

## 🧑‍💻 Development

### Common Commands

The `Makefile` provides convenient shortcuts:

```bash
make fmt       # Format code with cargo fmt
make lint      # Lint with cargo clippy
make test      # Run test suite
make migrate   # Apply database migrations
make prepare   # Prepare sqlx metadata for CI
```

### Running Tests

```bash
cargo test
```

### Code Quality

```bash
cargo fmt        # Format code
cargo clippy     # Lint with strict warnings
cargo audit      # Security audit
```

### Local Development

```bash
# Start dependencies
docker compose up -d postgres redis

# Run with hot-reload (install cargo-watch)
cargo watch -x run

# Enable debug logging
RUST_LOG=debug cargo run
```

### Database Migrations

```bash
# Apply migrations
cargo sqlx migrate run

# Create new migration
cargo sqlx migrate add <migration_name>

# Prepare offline metadata (for CI)
cargo sqlx prepare
```

---

## 🤝 Contributing

We welcome contributions! Please see:

- **[CONTRIBUTING.md](CONTRIBUTING.md)** - Contribution guidelines
- **[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)** - Community standards
- **[Good First Issues](https://github.com/memophor/synagraph/labels/good-first-issue)** - Great starting points

### How to Contribute

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes and add tests
4. Run `make fmt && make lint && make test`
5. Commit with signed commits (`git commit -S -m "feat: add amazing feature"`)
6. Push and create a Pull Request

### CI Requirements

All pull requests must pass:
- ✅ `cargo fmt` - Code formatting
- ✅ `cargo clippy -- -D warnings` - Linting with no warnings
- ✅ `cargo test` - Full test suite
- ✅ Runs on GitHub Actions (`.github/workflows/ci.yml`)

---

## 🗺️ Roadmap

| Milestone | Status | Target | Features |
|-----------|--------|--------|----------|
| **v0.1 (Foundation)** | 🔄 In Progress | Q4 2025 | gRPC scaffold, HTTP health, basic domain model |
| **v0.2 (Persistence)** | 🧱 Planned | Q1 2026 | PostgreSQL repository, pgvector integration, migrations |
| **v0.3 (Intelligence)** | 🧱 Planned | Q2 2026 | Temporal decay, reinforcement learning, graph algorithms |
| **v1.0 (Production)** | ⏳ Planned | Q3 2026 | Production-ready, stable API, horizontal scaling |

### Current Status (v0.1)
- ✅ HTTP `/health` and `/ready` endpoints
- ✅ gRPC API scaffold with Tonic
- ✅ Basic domain model for knowledge nodes
- ✅ Shared configuration and telemetry
- ✅ Docker Compose development stack
- ✅ Database migrations framework
- 🔄 PostgreSQL + pgvector persistence (in progress)

**📖 See [Vision & Platform Principles](docs/vision.md) for long-term roadmap**

---

## 🔒 Security

- **Multi-tenant isolation** via policy-aware repositories
- **Provenance tracking** for knowledge lineage
- **Signed commits** required for contributions
- **Dependency scanning** via `cargo audit`

**📖 See [SECURITY.md](SECURITY.md) for reporting vulnerabilities**

---

## 📜 License

Copyright © 2025 Memophor Labs

Licensed under the **Apache License, Version 2.0**.
See [LICENSE](LICENSE) for details.

---

## 🌟 Community

- **GitHub Discussions** - [Join the conversation](https://github.com/memophor/synagraph/discussions)
- **Issues** - [Report bugs or request features](https://github.com/memophor/synagraph/issues)
- **Twitter** - [@memophor](https://twitter.com/memophor)
- **Discord** - [Join our community](https://discord.gg/memophor)

---

## 🙏 Acknowledgments

Built with:
- [Rust](https://www.rust-lang.org/) - Blazing fast and memory safe
- [Tonic](https://github.com/hyperium/tonic) - gRPC framework for Rust
- [Axum](https://github.com/tokio-rs/axum) - Ergonomic web framework
- [PostgreSQL](https://www.postgresql.org/) - Advanced relational database
- [pgvector](https://github.com/pgvector/pgvector) - Vector similarity search

---

## 📂 Project Layout

```
synagraph/
├── src/
│   ├── main.rs              # Service entrypoint
│   ├── grpc/                # gRPC server implementation
│   ├── http/                # HTTP health endpoints
│   ├── domain/              # Knowledge node models
│   └── storage/             # PostgreSQL repository layer
├── proto/
│   └── synagraph.proto      # gRPC service definitions
├── migrations/              # Database schema migrations
├── docs/                    # Documentation
├── build.rs                 # Protobuf compilation
├── Makefile                 # Development shortcuts
└── docker-compose.yml       # Local development stack
```

---

<div align="center">

**🧠 Move knowledge, not data.**

Made with ❤️ by [Memophor Labs](https://memophor.com)

</div>
