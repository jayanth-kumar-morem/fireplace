import { memo, useRef, useEffect } from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { AggregatedLevel, VenueContribution } from "@pma/shared";

// ─── Helpers ───────────────────────────────────────

function formatSize(size: number): string {
  if (size >= 1_000_000) return `${(size / 1_000_000).toFixed(1)}M`;
  if (size >= 1_000) return `${(size / 1_000).toFixed(1)}K`;
  return size.toFixed(0);
}

function formatPrice(price: number): string {
  return `${(price * 100).toFixed(1)}`;
}

function venueBarWidth(venues: VenueContribution[], maxSize: number): { poly: number; kalshi: number } {
  const polySize = venues.find((v) => v.venue === "polymarket")?.size ?? 0;
  const kalshiSize = venues.find((v) => v.venue === "kalshi")?.size ?? 0;
  const total = polySize + kalshiSize;
  if (maxSize === 0 || total === 0) return { poly: 0, kalshi: 0 };
  const pct = (total / maxSize) * 100;
  return {
    poly: (polySize / total) * pct,
    kalshi: (kalshiSize / total) * pct,
  };
}

// ─── Venue Tooltip Content (T8.6) ──────────────────

function VenueTooltip({ venues }: { venues: VenueContribution[] }) {
  return (
    <div className="text-xs space-y-0.5">
      {venues.map((v) => (
        <div key={v.venue} className="flex items-center gap-2">
          <span
            className={`h-2 w-2 rounded-full ${
              v.venue === "polymarket" ? "bg-polymarket" : "bg-kalshi"
            }`}
          />
          <span className="capitalize">{v.venue}</span>
          <span className="text-muted-foreground ml-auto pl-4">
            {formatSize(v.size)}
          </span>
        </div>
      ))}
    </div>
  );
}

// ─── Spread Row ────────────────────────────────────

export const SpreadRow = memo(function SpreadRow({
  spread,
  midpoint,
}: {
  spread: number;
  midpoint: number;
}) {
  return (
    <div className="h-8 flex items-center justify-center text-xs text-muted-foreground bg-spread/20 border-y border-spread/40 select-none">
      Spread: {formatPrice(spread)}¢ · Mid: {formatPrice(midpoint)}¢
    </div>
  );
});

// ─── Price Level Row (T8.2 + T8.4 + T8.6) ─────────

export const PriceLevelRow = memo(function PriceLevelRow({
  level,
  side,
  maxSize,
}: {
  level: AggregatedLevel;
  side: "bid" | "ask";
  maxSize: number;
}) {
  const { poly, kalshi } = venueBarWidth(level.venues, maxSize);

  // T8.4 — Flash on size change
  const prevSizeRef = useRef(level.totalSize);
  const flashRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const prev = prevSizeRef.current;
    prevSizeRef.current = level.totalSize;

    if (prev === level.totalSize || !flashRef.current) return;

    const el = flashRef.current;
    const color =
      level.totalSize > prev
        ? "rgba(0, 192, 135, 0.15)" // green flash
        : "rgba(239, 83, 80, 0.15)"; // red flash

    el.style.backgroundColor = color;
    const timeout = setTimeout(() => {
      el.style.backgroundColor = "transparent";
    }, 300);
    return () => clearTimeout(timeout);
  }, [level.totalSize]);

  return (
    <Tooltip>
      <TooltipTrigger
        className="w-full cursor-default"
        onClick={(e) => e.preventDefault()}
      >
        <div
          ref={flashRef}
          className="h-8 grid grid-cols-[1fr_80px_1fr] items-center px-2 text-xs font-mono transition-[background-color] duration-300"
        >
          {/* Bid side (left) */}
          <div className="relative flex items-center justify-end h-full overflow-hidden">
            {side === "bid" && (
              <>
                <div className="absolute inset-y-0 right-0 flex flex-row-reverse h-full">
                  {poly > 0 && (
                    <div
                      className="h-full bg-polymarket/25"
                      style={{ width: `${poly}%` }}
                    />
                  )}
                  {kalshi > 0 && (
                    <div
                      className="h-full bg-kalshi/25"
                      style={{ width: `${kalshi}%` }}
                    />
                  )}
                </div>
                <span className="relative z-10 text-bid">
                  {formatSize(level.totalSize)}
                </span>
              </>
            )}
          </div>

          {/* Price (center) */}
          <div className="text-center text-muted-foreground">
            {formatPrice(level.price)}¢
          </div>

          {/* Ask side (right) */}
          <div className="relative flex items-center justify-start h-full overflow-hidden">
            {side === "ask" && (
              <>
                <div className="absolute inset-y-0 left-0 flex flex-row h-full">
                  {poly > 0 && (
                    <div
                      className="h-full bg-polymarket/25"
                      style={{ width: `${poly}%` }}
                    />
                  )}
                  {kalshi > 0 && (
                    <div
                      className="h-full bg-kalshi/25"
                      style={{ width: `${kalshi}%` }}
                    />
                  )}
                </div>
                <span className="relative z-10 text-ask">
                  {formatSize(level.totalSize)}
                </span>
              </>
            )}
          </div>
        </div>
      </TooltipTrigger>
      <TooltipContent side="right" className="p-2">
        <div className="text-xs mb-1 font-semibold">
          {formatPrice(level.price)}¢ — {formatSize(level.totalSize)} total
        </div>
        <VenueTooltip venues={level.venues} />
      </TooltipContent>
    </Tooltip>
  );
});
