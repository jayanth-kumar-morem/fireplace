// ─── Venue & Connection ────────────────────────────

export type Venue = "polymarket" | "kalshi";

export type ConnectionStatus =
  | "connected"
  | "reconnecting"
  | "disconnected"
  | "stale";

// ─── Normalized Price Level (venue-tagged) ─────────

export interface PriceLevel {
  price: number; // 0.00–1.00, rounded to 0.01
  size: number; // Number of shares/contracts
  venue: Venue;
}

// ─── Normalized Book (per venue) ───────────────────

export interface NormalizedBook {
  bids: PriceLevel[]; // Sorted descending by price
  asks: PriceLevel[]; // Sorted ascending by price
  lastUpdated: number; // Unix ms
}

// ─── Aggregated Level ──────────────────────────────

export interface VenueContribution {
  venue: Venue;
  size: number;
}

export interface AggregatedLevel {
  price: number;
  totalSize: number;
  venues: VenueContribution[];
  cumulativeSize?: number; // Populated for depth chart rendering
}

// ─── Aggregated Book ───────────────────────────────

export interface AggregatedBook {
  bids: AggregatedLevel[]; // Sorted descending by price (best bid first)
  asks: AggregatedLevel[]; // Sorted ascending by price (best ask first)
  spread: number;
  midpoint: number;
  bestBid: number;
  bestAsk: number;
}

// ─── Market Info ───────────────────────────────────

export interface MarketInfo {
  title: string;
  outcomes: [string, string];
  polymarketSlug: string;
  kalshiTicker: string;
  polymarketVolume?: number;
  kalshiVolume?: number;
}

// ─── Quote ─────────────────────────────────────────

export interface QuoteInput {
  dollarAmount: number;
  outcome: "yes" | "no";
}

export interface Fill {
  price: number;
  shares: number;
  cost: number;
  venue: Venue;
}

export interface VenueFillSummary {
  shares: number;
  cost: number;
}

export interface QuoteResult {
  totalShares: number;
  avgPrice: number;
  fills: Fill[];
  venueSplit: Record<Venue, VenueFillSummary>;
  slippage: number; // Percentage above best available price
  unfilled: number; // Remaining dollars if book exhausted
  impliedProbability: number;
}

// ─── WebSocket Messages (Server → Client) ──────────

export interface SnapshotMessage {
  type: "snapshot";
  data: {
    market: MarketInfo;
    books: {
      polymarket: NormalizedBook | null;
      kalshi: NormalizedBook | null;
      aggregated: AggregatedBook | null;
    };
    connections: Record<Venue, ConnectionStatus>;
  };
}

export interface BookChange {
  side: "bid" | "ask";
  price: number;
  size: number; // 0 = level removed
}

export interface BookUpdateMessage {
  type: "book_update";
  venue: Venue;
  changes: BookChange[];
  timestamp: number;
}

export interface ConnectionStatusMessage {
  type: "connection_status";
  venue: Venue;
  status: ConnectionStatus;
  timestamp: number;
}

export interface HeartbeatMessage {
  type: "heartbeat";
  timestamp: number;
  connections: Record<Venue, ConnectionStatus>;
}

export type ServerMessage =
  | SnapshotMessage
  | BookUpdateMessage
  | ConnectionStatusMessage
  | HeartbeatMessage;

// ─── Polymarket Raw Types ──────────────────────────

export interface PolymarketRawLevel {
  price: string;
  size: string;
}

export interface PolymarketBookResponse {
  market: string; // Condition ID
  asset_id: string; // Token ID
  bids: PolymarketRawLevel[];
  asks: PolymarketRawLevel[];
  timestamp: string;
  hash: string;
}

export interface PolymarketPriceChangeEntry {
  asset_id: string;
  price: string;
  size: string; // "0" = level removed
  side: "BUY" | "SELL";
  hash?: string;
  best_bid?: string;
  best_ask?: string;
}

export interface PolymarketPriceChangeEvent {
  event_type: "price_change";
  market: string;
  price_changes: PolymarketPriceChangeEntry[];
  timestamp: string;
}

export interface PolymarketBookEvent {
  event_type: "book";
  asset_id: string;
  market: string;
  bids: PolymarketRawLevel[];
  asks: PolymarketRawLevel[];
  timestamp: string;
  hash?: string;
}

export type PolymarketWsEvent =
  | PolymarketBookEvent
  | PolymarketPriceChangeEvent;

// ─── Kalshi Raw Types ──────────────────────────────

export interface KalshiOrderbookResponse {
  orderbook_fp: {
    yes_dollars: [string, string][]; // [price, quantity]
    no_dollars: [string, string][];
  };
}

export interface KalshiOrderbookSnapshotMsg {
  market_ticker: string;
  market_id: string;
  yes_dollars_fp: [string, string][];
  no_dollars_fp: [string, string][];
}

export interface KalshiOrderbookSnapshot {
  type: "orderbook_snapshot";
  sid: number;
  seq: number;
  msg: KalshiOrderbookSnapshotMsg;
}

export interface KalshiOrderbookDeltaMsg {
  market_ticker: string;
  market_id: string;
  price_dollars: string;
  delta_fp: string; // Positive = add, negative = remove
  side: "yes" | "no";
  client_order_id?: string;
  subaccount?: number;
  ts?: string; // RFC 3339
}

export interface KalshiOrderbookDelta {
  type: "orderbook_delta";
  sid: number;
  seq: number;
  msg: KalshiOrderbookDeltaMsg;
}

export type KalshiWsMessage = KalshiOrderbookSnapshot | KalshiOrderbookDelta;

// ─── View Mode ─────────────────────────────────────

export type ViewMode = "aggregated" | "polymarket" | "kalshi";
