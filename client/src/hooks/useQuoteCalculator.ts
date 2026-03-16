import { useMemo } from "react";
import type { AggregatedBook, AggregatedLevel, QuoteResult, Venue } from "@pma/shared";

type Outcome = "yes" | "no";

interface FlatLevel {
  price: number;
  size: number;
  venue: Venue;
}

function flattenYesAsks(asks: AggregatedLevel[]): FlatLevel[] {
  const sorted = [...asks].sort((a, b) => a.price - b.price);
  const levels: FlatLevel[] = [];
  for (const level of sorted) {
    for (const venue of level.venues) {
      levels.push({
        price: level.price,
        size: venue.size,
        venue: venue.venue,
      });
    }
  }
  return levels;
}

function flattenNoAsksFromYesBids(bids: AggregatedLevel[]): FlatLevel[] {
  const sorted = [...bids].sort((a, b) => b.price - a.price);
  const levels: FlatLevel[] = [];
  for (const level of sorted) {
    const noPrice = Math.max(0, Math.min(1, 1 - level.price));
    for (const venue of level.venues) {
      levels.push({
        price: noPrice,
        size: venue.size,
        venue: venue.venue,
      });
    }
  }
  return levels.sort((a, b) => a.price - b.price);
}

export function useQuoteCalculator(
  dollarAmount: number,
  outcome: Outcome,
  aggregatedBook: AggregatedBook | null
): QuoteResult | null {
  return useMemo(() => {
    if (!aggregatedBook || !Number.isFinite(dollarAmount) || dollarAmount <= 0) return null;

    const levels =
      outcome === "yes"
        ? flattenYesAsks(aggregatedBook.asks)
        : flattenNoAsksFromYesBids(aggregatedBook.bids);

    if (levels.length === 0) return null;

    let remaining = dollarAmount;
    let totalShares = 0;
    const fills: QuoteResult["fills"] = [];
    const venueSplit: QuoteResult["venueSplit"] = {
      polymarket: { shares: 0, cost: 0 },
      kalshi: { shares: 0, cost: 0 },
    };

    for (const level of levels) {
      if (remaining <= 0) break;
      if (level.price <= 0 || level.size <= 0) continue;

      const maxCost = level.price * level.size;
      if (maxCost <= remaining) {
        fills.push({
          price: level.price,
          shares: level.size,
          cost: maxCost,
          venue: level.venue,
        });
        totalShares += level.size;
        remaining -= maxCost;
        venueSplit[level.venue].shares += level.size;
        venueSplit[level.venue].cost += maxCost;
      } else {
        const partialShares = remaining / level.price;
        fills.push({
          price: level.price,
          shares: partialShares,
          cost: remaining,
          venue: level.venue,
        });
        totalShares += partialShares;
        venueSplit[level.venue].shares += partialShares;
        venueSplit[level.venue].cost += remaining;
        remaining = 0;
      }
    }

    const totalCost = dollarAmount - remaining;
    const avgPrice = totalShares > 0 ? totalCost / totalShares : 0;
    const bestPrice = levels[0]?.price ?? 0;
    const slippage = bestPrice > 0 ? ((avgPrice - bestPrice) / bestPrice) * 100 : 0;

    return {
      totalShares,
      avgPrice,
      fills,
      venueSplit,
      slippage,
      unfilled: remaining,
      impliedProbability: avgPrice,
    };
  }, [dollarAmount, outcome, aggregatedBook]);
}
