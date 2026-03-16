import { create } from "zustand";
import type {
  AggregatedBook,
  ConnectionStatus,
  NormalizedBook,
  MarketInfo,
  SnapshotMessage,
  BookUpdateMessage,
  Venue,
  ViewMode,
  BookChange,
} from "@pma/shared";
import { TICK_SIZE, MAX_BOOK_DEPTH } from "@pma/shared";
import { aggregate } from "@/lib/aggregator";

// ─── Helpers ───────────────────────────────────────

function roundToTick(value: number, tick: number): number {
  return Math.round(value / tick) * tick;
}

function applyChangesToBook(
  book: NormalizedBook,
  venue: Venue,
  changes: BookChange[]
): NormalizedBook {
  // Clone to avoid mutating the store's current reference
  const bids = [...book.bids];
  const asks = [...book.asks];

  for (const change of changes) {
    const levels = change.side === "bid" ? bids : asks;
    const price = roundToTick(change.price, TICK_SIZE);
    const idx = levels.findIndex((l) => l.price === price);

    if (change.size === 0) {
      if (idx !== -1) levels.splice(idx, 1);
    } else if (idx !== -1) {
      levels[idx] = { ...levels[idx], size: change.size };
    } else {
      levels.push({ price, size: change.size, venue });
    }
  }

  // T11.1 — enforce depth cap on every update path.
  bids.sort((a, b) => b.price - a.price);
  asks.sort((a, b) => a.price - b.price);
  if (bids.length > MAX_BOOK_DEPTH) bids.length = MAX_BOOK_DEPTH;
  if (asks.length > MAX_BOOK_DEPTH) asks.length = MAX_BOOK_DEPTH;

  return { bids, asks, lastUpdated: Date.now() };
}

function capBookDepth(book: NormalizedBook | null): NormalizedBook | null {
  if (!book) return null;
  return {
    ...book,
    bids: [...book.bids].sort((a, b) => b.price - a.price).slice(0, MAX_BOOK_DEPTH),
    asks: [...book.asks].sort((a, b) => a.price - b.price).slice(0, MAX_BOOK_DEPTH),
  };
}

// ─── Store ─────────────────────────────────────────

interface OrderBookState {
  // Market info
  market: MarketInfo | null;

  // Per-venue books
  polymarketBook: NormalizedBook | null;
  kalshiBook: NormalizedBook | null;

  // Aggregated book
  aggregatedBook: AggregatedBook | null;

  // Connection states
  connections: Record<Venue, ConnectionStatus>;

  // Client WS connection state
  clientStatus: "connecting" | "connected" | "reconnecting" | "disconnected";

  // View mode
  viewMode: ViewMode;

  // Last updated timestamps per venue
  lastUpdated: Record<Venue, number | null>;

  // Actions
  applySnapshot: (msg: SnapshotMessage) => void;
  applyBookUpdate: (msg: BookUpdateMessage) => void;
  updateConnectionStatus: (venue: Venue, status: ConnectionStatus) => void;
  setViewMode: (mode: ViewMode) => void;
  setClientStatus: (status: OrderBookState["clientStatus"]) => void;
}

export const useOrderBookStore = create<OrderBookState>((set, get) => ({
  // ─── Initial State ────────────────────────────

  market: null,
  polymarketBook: null,
  kalshiBook: null,
  aggregatedBook: null,
  connections: {
    polymarket: "disconnected",
    kalshi: "disconnected",
  },
  clientStatus: "connecting",
  viewMode: "aggregated",
  lastUpdated: {
    polymarket: null,
    kalshi: null,
  },

  // ─── T6.2: Apply Snapshot ─────────────────────

  applySnapshot: (msg) => {
    const { market, books, connections } = msg.data;
    const polymarketBook = capBookDepth(books.polymarket);
    const kalshiBook = capBookDepth(books.kalshi);
    const aggregatedBook = aggregate(polymarketBook, kalshiBook);

    set({
      market,
      polymarketBook,
      kalshiBook,
      aggregatedBook,
      connections,
      lastUpdated: {
        polymarket: polymarketBook?.lastUpdated ?? null,
        kalshi: kalshiBook?.lastUpdated ?? null,
      },
    });
  },

  // ─── T6.3: Apply Book Update ──────────────────

  applyBookUpdate: (msg) => {
    const state = get();
    const { venue, changes, timestamp } = msg;

    // Get the current venue book
    const bookKey = venue === "polymarket" ? "polymarketBook" : "kalshiBook";
    const currentBook = state[bookKey];

    if (!currentBook) return;

    // Apply changes
    const updatedBook = applyChangesToBook(currentBook, venue, changes);

    // Recompute aggregated book
    const polyBook = venue === "polymarket" ? updatedBook : state.polymarketBook;
    const kalshiBook = venue === "kalshi" ? updatedBook : state.kalshiBook;
    const aggregatedBook = aggregate(polyBook, kalshiBook);

    set({
      [bookKey]: updatedBook,
      aggregatedBook,
      lastUpdated: {
        ...state.lastUpdated,
        [venue]: timestamp,
      },
    });
  },

  // ─── Connection Status ────────────────────────

  updateConnectionStatus: (venue, status) => {
    set((state) => ({
      connections: {
        ...state.connections,
        [venue]: status,
      },
    }));
  },

  // ─── View Mode ────────────────────────────────

  setViewMode: (mode) => set({ viewMode: mode }),

  // ─── Client Status ────────────────────────────

  setClientStatus: (status) => set({ clientStatus: status }),
}));
