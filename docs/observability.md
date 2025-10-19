<!-- SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions. -->
# Observability Roadmap

This document captures the Phase 1 observability enhancements needed as persistence and background services come online.

## Goals

- Provide actionable metrics, logs, and health signals across HTTP/gRPC handlers and storage backends.
- Support local debugging with minimal setup, while keeping production-ready hooks for exporters.

## Metrics

| Area            | Metric                                         | Notes |
|-----------------|------------------------------------------------|-------|
| HTTP            | Request count/duration (`synagraph_http_*`)    | Use `tower-http` `TraceLayer` or OpenTelemetry middleware. |
| gRPC            | Request count/duration/error codes             | Wrap Tonic services with middleware emitting Prometheus/OTel metrics. |
| Storage         | Upsert latency, connection pool usage          | Surface via repository layer instrumentation. |
| Background jobs | Queue depth, success/failure counts (Phase 2)  | Collect once workers exist. |

Initial implementation can rely on `metrics` crate + `prometheus` exporter; evaluate OpenTelemetry adoption once requirements stabilize.

## Logging

- Current defaults (`synagraph=info,tower_http=info`) remain.
- Add structured fields (`node_id`, `kind`, `status_code`) around persistence interactions.
- Introduce `tracing` spans in repository calls to tie logs and metrics together.

## Tracing

- Integrate `tracing-opentelemetry` with an OTLP exporter (feature-gated) for distributed traces.
- Provide `docker-compose` example with `otel-collector` and `jaeger` for local inspection.

## Health & Readiness

- Extend `/ready` to check downstreams:
  - Database connectivity (simple `SELECT 1`).
  - Vector index availability.
  - Background worker heartbeats (once implemented).
- Return JSON payload listing component statuses to aid debugging.
- Add `GET /metrics` endpoint once Prometheus exporter is wired.

## Alerting Hooks (Future)

- Document log/metric thresholds for production (e.g., error rate, latency SLOs).
- Provide sample Grafana dashboards and alert rules.

## Action Items

- [ ] Add `metrics` crate and expose Prometheus endpoint behind feature flag.
- [ ] Instrument HTTP/gRPC handlers with latency counters.
- [ ] Implement readiness checks for database (stub returning `degraded` until storage lands).
- [ ] Evaluate `opentelemetry-rust` integration for distributed tracing.

