import type {
  AggregatedBook,
  AggregatedLevel,
  NormalizedBook,
  PriceLevel,
} from "@pma/shared";
import { TICK_SIZE } from "@pma/shared";

function roundToTick(value: number, tick: number): number {
  return Math.round(value / tick) * tick;
}

function addToMap(
  map: Map<number, AggregatedLevel>,
  level: PriceLevel
): void {
  const key = roundToTick(level.price, TICK_SIZE);
  const existing = map.get(key);

  if (existing) {
    existing.totalSize += level.size;
    const venueEntry = existing.venues.find((v) => v.venue === level.venue);
    if (venueEntry) {
      venueEntry.size += level.size;
    } else {
      existing.venues.push({ venue: level.venue, size: level.size });
    }
  } else {
    map.set(key, {
      price: key,
      totalSize: level.size,
      venues: [{ venue: level.venue, size: level.size }],
    });
  }
}

export function aggregate(
  ...books: (NormalizedBook | null)[]
): AggregatedBook {
  const bidMap = new Map<number, AggregatedLevel>();
  const askMap = new Map<number, AggregatedLevel>();

  for (const book of books) {
    if (!book) continue;
    for (const level of book.bids) addToMap(bidMap, level);
    for (const level of book.asks) addToMap(askMap, level);
  }

  const bids = [...bidMap.values()].sort((a, b) => b.price - a.price);
  const asks = [...askMap.values()].sort((a, b) => a.price - b.price);

  const bestBid = bids[0]?.price ?? 0;
  const bestAsk = asks[0]?.price ?? 1;
  const spread = Math.max(0, roundToTick(bestAsk - bestBid, TICK_SIZE));
  const midpoint = roundToTick((bestBid + bestAsk) / 2, TICK_SIZE);

  return { bids, asks, spread, midpoint, bestBid, bestAsk };
}
