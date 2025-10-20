# SynaGraph Dashboard

React + Vite single-page app that surfaces SynaGraph metrics, history, and operational tooling.

## Design System

- SynaGraph uses the bespoke palette defined in `src/styles.css` (`--accent`, `--bg`, `--panel`, etc.) to keep light and dark themes in sync.
- Typography leans on the Sora sans-serif family for UI copy and JetBrains Mono for payload or log data.
- Buttons and cards pull shared radii/shadows (`--card-shadow`, `--focus-ring`) so new components inherit the same depth and motion.
- Add new surfaces by consuming `var(--panel)` as the base layer and overlaying gradients via pseudo-elements for consistency with existing panels and metric cards.

## Core Workflows

- **Upsert nodes** – populate the Node form (kind, payload, optional embedding/provenance/decay λ) and submit; response metadata appears inline.
- **Relate nodes** – use the Edge tab to connect two nodes with optional weight/payload/provenance, mapping directly to `POST /edges`.
- **Hybrid search** – run vector+symbolic queries from the Search tab; the results panel echoes top-k hits, scores, and metadata.
- **Ingest capsules** – drop a CCP payload into the Capsule tab and optionally unwrap to nodes, surfacing emitted events immediately.
- **Graph explorer** – fetch node detail or neighbor context via the Explorer panel, wired to `GET /nodes/{id}` and `GET /neighbors/{id}`.
- **Decay & test events** – trigger decay/reinforce or emit synthetic events from the Events tab for verification and pipeline exercise.
- **Diagnostics** – monitor Postgres/pgvector health, index state, embedding dimensionality, and decay profiles next to the graph explorer.
- **Scedge observability** – open the Scedge tab to inspect cache health/metrics and run lookup/store/purge operations when `SCEDGE_BASE_URL` is configured.

## Commands

```bash
npm install          # install dependencies
npm run dev          # start dev server (http://localhost:5173 by default)
npm run build        # produce static assets in dashboard/dist
npm run preview      # serve production build locally
```

`cargo run` serves the compiled dashboard at `/dashboard` when `dashboard/dist` exists. During development, run `npm run dev` and rely on Vite's dev server while the Rust backend handles API requests on port 8080.
Set `VITE_API_TARGET` (defaults to `http://localhost:8080`) if your API runs on a different origin; the dev and preview servers proxy `/api` calls accordingly.

## Environment

The UI talks to the backend via the same origin (`/api/...`). When running the Vite dev server, configure a proxy in `vite.config.ts` if you need to target a different backend origin.
