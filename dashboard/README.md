# SynaGraph Dashboard

React + Vite single-page app that surfaces SynaGraph metrics, history, and operational tooling.

## Commands

```bash
npm install          # install dependencies
npm run dev          # start dev server (http://localhost:5173 by default)
npm run build        # produce static assets in dashboard/dist
npm run preview      # serve production build locally
```

`cargo run` serves the compiled dashboard at `/dashboard` when `dashboard/dist` exists. During development, run `npm run dev` and rely on Vite's dev server while the Rust backend handles API requests on port 8080.

## Environment

The UI talks to the backend via the same origin (`/api/...`). When running the Vite dev server, configure a proxy in `vite.config.ts` if you need to target a different backend origin.
