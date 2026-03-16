# Prediction Market Aggregator

Real-time order book aggregator for Polymarket + Kalshi, focused on the JD Vance 2028 presidential election market.

## Setup

```bash
npm install
cp .env.example .env
npm run dev
```

Opens at http://localhost:5173. Rust server runs on :3001.

Needs Node 20+, npm 10+, and Rust stable.

Optionally set `KALSHI_API_KEY` + `KALSHI_PRIVATE_KEY_PATH` in `.env` for Kalshi WS auth — without these it falls back to REST polling.

## Design decisions

**Rust backend** — went with Rust (Axum + Tokio) over Node because the order book normalization + aggregation is CPU-bound work that benefits from zero-cost abstractions. also wanted to avoid GC pauses since this thing runs for hours.

**Server-side aggregation** — the server merges both venue books into one normalized format before sending to the client. this keeps the frontend simple and means we only push diffs over one WS connection instead of the client managing two separate feeds.

**Price normalization** — Polymarket uses 0-1 decimals, Kalshi uses cents. server normalizes everything to 0-1 so the client doesn't care about venue-specific quirks.

**Quote engine walks both books** — when you enter a dollar amount, the calculator walks the combined order book best-price-first across both venues. it shows exactly which venue fills what portion and at what price.

**Graceful degradation** — if one venue drops, the other keeps flowing. connection status shows per-venue health with staleness detection, and a warning banner pops up automatically.

## Assumptions & tradeoffs

- hardcoded to one market (JD Vance 2028) — no market discovery/search
- read-only, no real orders placed
- fees not included in quote calculations (noted in UI)
- Polymarket WS can be flaky — server handles reconnection with exponential backoff + jitter
- cross-venue market pairing is manually configured, not auto-matched

## What I'd improve with more time

- add depth chart visualization (cumulative bid/ask by venue)
- market search / multi-market support
- historical price charts
- fee estimation per venue in the quote calculator
- E2E tests with playwright
- deploy with docker compose

## Commands

| Command | What it does |
|---|---|
| `npm run dev` | starts rust server + react client |
| `npm run dev:server-rs` | rust server only |
| `npm run build` | full workspace build |
| `npm run test:server-rs` | rust integration tests |
