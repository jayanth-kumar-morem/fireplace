import type { MarketInfo } from "./types.js";

// ─── Server ────────────────────────────────────────

export const SERVER_PORT = 3001;

// ─── Polymarket ────────────────────────────────────

export const POLYMARKET_CLOB_BASE = "https://clob.polymarket.com";
export const POLYMARKET_WS_URL =
  "wss://ws-subscriptions-clob.polymarket.com/ws/market";

export const POLYMARKET_CONDITION_ID =
  "0x7ad403c3508f8e3912940fd1a913f227591145ca0614074208e0b962d5fcc422";

export const POLYMARKET_YES_TOKEN_ID =
  "16040015440196279900485035793550429453516625694844857319147506590755961451627";

export const POLYMARKET_NO_TOKEN_ID =
  "94476829201604408463453426454480212459887267917122244941405244686637914508323";

export const POLYMARKET_SLUG =
  "will-jd-vance-win-the-2028-us-presidential-election";

export const POLYMARKET_PING_INTERVAL_MS = 10_000;

// ─── Kalshi ────────────────────────────────────────

export const KALSHI_API_BASE =
  "https://api.elections.kalshi.com/trade-api/v2";

export const KALSHI_WS_URL =
  "wss://api.elections.kalshi.com/trade-api/ws/v2";

export const KALSHI_MARKET_TICKER = "KXPRESPERSON-28-JVAN";
export const KALSHI_EVENT_TICKER = "KXPRESPERSON-28";

export const KALSHI_POLL_INTERVAL_MS = 2_000;

// ─── Aggregation ───────────────────────────────────

export const TICK_SIZE = 0.01;
export const MAX_BOOK_DEPTH = 200;

// ─── Connection Health ─────────────────────────────

export const RECONNECT_BASE_DELAY_MS = 1_000;
export const RECONNECT_MAX_DELAY_MS = 30_000;
export const RECONNECT_MAX_ATTEMPTS = 10;
export const STALE_THRESHOLD_MS = 10_000;
export const HEARTBEAT_INTERVAL_MS = 15_000;
export const RECONCILIATION_INTERVAL_MS = 5 * 60 * 1_000; // 5 minutes

// ─── Market Metadata ───────────────────────────────

export const MARKET_INFO: MarketInfo = {
  title: "Will JD Vance win the 2028 US Presidential Election?",
  outcomes: ["Yes", "No"],
  polymarketSlug: POLYMARKET_SLUG,
  kalshiTicker: KALSHI_MARKET_TICKER,
};
