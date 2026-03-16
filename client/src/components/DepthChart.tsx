import { useEffect, useMemo, useRef, useState } from "react";
import * as d3 from "d3";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useOrderBookStore } from "@/store/orderBookStore";
import type { AggregatedBook, AggregatedLevel, VenueContribution } from "@pma/shared";

const CHART_HEIGHT = 360;
const RENDER_THROTTLE_MS = 100;
const TRANSITION_MS = 200;
const PRICE_PADDING = 0.1;

type DepthPoint = {
  price: number;
  polyCum: number;
  kalshiCum: number;
  totalCum: number;
};

type HoverValues = {
  price: number;
  bid: DepthPoint | null;
  ask: DepthPoint | null;
  x: number;
  y: number;
};

function venueSize(venues: VenueContribution[], venue: "polymarket" | "kalshi"): number {
  return venues.find((v) => v.venue === venue)?.size ?? 0;
}

function buildBidPoints(levels: AggregatedLevel[]): DepthPoint[] {
  const bidsDesc = [...levels].sort((a, b) => b.price - a.price);
  let polyCum = 0;
  let kalshiCum = 0;
  const pointsDesc = bidsDesc.map((level) => {
    polyCum += venueSize(level.venues, "polymarket");
    kalshiCum += venueSize(level.venues, "kalshi");
    return {
      price: level.price,
      polyCum,
      kalshiCum,
      totalCum: polyCum + kalshiCum,
    };
  });
  // Plot in ascending x order.
  return pointsDesc.reverse();
}

function buildAskPoints(levels: AggregatedLevel[]): DepthPoint[] {
  const asksAsc = [...levels].sort((a, b) => a.price - b.price);
  let polyCum = 0;
  let kalshiCum = 0;
  return asksAsc.map((level) => {
    polyCum += venueSize(level.venues, "polymarket");
    kalshiCum += venueSize(level.venues, "kalshi");
    return {
      price: level.price,
      polyCum,
      kalshiCum,
      totalCum: polyCum + kalshiCum,
    };
  });
}

function formatPrice(price: number): string {
  return `${(price * 100).toFixed(1)}c`;
}

function formatSize(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return value.toFixed(0);
}

export function DepthChart() {
  const aggregatedBook = useOrderBookStore((s) => s.aggregatedBook);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  const [width, setWidth] = useState(0);
  const [hover, setHover] = useState<HoverValues | null>(null);

  const latestRenderRef = useRef<{ book: AggregatedBook; width: number } | null>(null);
  const throttleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverDataRef = useRef<{ bids: DepthPoint[]; asks: DepthPoint[] } | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;
    const observer = new ResizeObserver((entries) => {
      const next = entries[0]?.contentRect.width ?? 0;
      setWidth(next);
    });
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    return () => {
      if (throttleTimerRef.current) clearTimeout(throttleTimerRef.current);
      if (svgRef.current) {
        const svg = d3.select(svgRef.current);
        svg.selectAll("*").interrupt();
        svg.selectAll("*").remove();
      }
    };
  }, []);

  const ready = useMemo(
    () =>
      Boolean(
        aggregatedBook &&
          aggregatedBook.bids.length > 0 &&
          aggregatedBook.asks.length > 0 &&
          width > 0 &&
          svgRef.current
      ),
    [aggregatedBook, width]
  );

  useEffect(() => {
    if (!ready || !aggregatedBook || !svgRef.current) return;

    latestRenderRef.current = { book: aggregatedBook, width };
    if (throttleTimerRef.current) return;

    const render = () => {
      throttleTimerRef.current = null;
      if (!latestRenderRef.current || !svgRef.current) return;

      const { book, width: chartWidth } = latestRenderRef.current;
      const margins = { top: 16, right: 18, bottom: 30, left: 58 };
      const innerWidth = Math.max(0, chartWidth - margins.left - margins.right);
      const innerHeight = CHART_HEIGHT - margins.top - margins.bottom;

      const bids = buildBidPoints(book.bids);
      const asks = buildAskPoints(book.asks);
      hoverDataRef.current = { bids, asks };

      const xMin = Math.max(0, book.bestBid - PRICE_PADDING);
      const xMax = Math.min(1, book.bestAsk + PRICE_PADDING);
      const yMax = Math.max(1, d3.max([...bids, ...asks], (d) => d.totalCum) ?? 1);

      const x = d3.scaleLinear().domain([xMin, xMax]).range([0, innerWidth]).nice();
      const y = d3.scaleLinear().domain([0, yMax]).range([innerHeight, 0]).nice();

      const svg = d3.select(svgRef.current).attr("width", chartWidth).attr("height", CHART_HEIGHT);
      svg.selectAll("*").interrupt();
      const root = svg
        .selectAll<SVGGElement, null>("g.depth-root")
        .data([null])
        .join("g")
        .attr("class", "depth-root")
        .attr("transform", `translate(${margins.left},${margins.top})`);

      const axisX = d3.axisBottom(x).ticks(6).tickFormat((d) => `${(Number(d) * 100).toFixed(0)}c`);
      const axisY = d3.axisLeft(y).ticks(5).tickFormat((d) => formatSize(Number(d)));

      root
        .selectAll<SVGGElement, null>("g.axis-x")
        .data([null])
        .join("g")
        .attr("class", "axis-x")
        .attr("transform", `translate(0,${innerHeight})`)
        .transition()
        .duration(TRANSITION_MS)
        .ease(d3.easeCubicOut)
        .call(axisX);

      root
        .selectAll<SVGGElement, null>("g.axis-y")
        .data([null])
        .join("g")
        .attr("class", "axis-y")
        .transition()
        .duration(TRANSITION_MS)
        .ease(d3.easeCubicOut)
        .call(axisY);

      root
        .selectAll<SVGTextElement, null>("text.y-label")
        .data([null])
        .join("text")
        .attr("class", "y-label")
        .attr("x", -innerHeight / 2)
        .attr("y", -42)
        .attr("fill", "currentColor")
        .attr("font-size", 11)
        .attr("text-anchor", "middle")
        .attr("transform", "rotate(-90)")
        .style("opacity", 0.75)
        .text("Cumulative size");

      const areaPoly = d3
        .area<DepthPoint>()
        .x((d) => x(d.price))
        .y0(y(0))
        .y1((d) => y(d.polyCum))
        .curve(d3.curveStepAfter);

      const areaKalshi = d3
        .area<DepthPoint>()
        .x((d) => x(d.price))
        .y0((d) => y(d.polyCum))
        .y1((d) => y(d.totalCum))
        .curve(d3.curveStepAfter);

      const areaTotal = d3
        .area<DepthPoint>()
        .x((d) => x(d.price))
        .y0(y(0))
        .y1((d) => y(d.totalCum))
        .curve(d3.curveStepAfter);

      root
        .selectAll<SVGPathElement, DepthPoint[]>("path.bid-total")
        .data([bids])
        .join("path")
        .attr("class", "bid-total")
        .attr("fill", "rgba(0, 192, 135, 0.30)")
        .transition()
        .duration(TRANSITION_MS)
        .ease(d3.easeCubicOut)
        .attr("d", areaTotal);

      root
        .selectAll<SVGPathElement, DepthPoint[]>("path.ask-total")
        .data([asks])
        .join("path")
        .attr("class", "ask-total")
        .attr("fill", "rgba(239, 83, 80, 0.30)")
        .transition()
        .duration(TRANSITION_MS)
        .ease(d3.easeCubicOut)
        .attr("d", areaTotal);

      root
        .selectAll<SVGPathElement, DepthPoint[]>("path.bid-poly")
        .data([bids])
        .join("path")
        .attr("class", "bid-poly")
        .attr("fill", "rgba(74, 144, 217, 0.35)")
        .transition()
        .duration(TRANSITION_MS)
        .ease(d3.easeCubicOut)
        .attr("d", areaPoly);

      root
        .selectAll<SVGPathElement, DepthPoint[]>("path.bid-kalshi")
        .data([bids])
        .join("path")
        .attr("class", "bid-kalshi")
        .attr("fill", "rgba(245, 166, 35, 0.35)")
        .transition()
        .duration(TRANSITION_MS)
        .ease(d3.easeCubicOut)
        .attr("d", areaKalshi);

      root
        .selectAll<SVGPathElement, DepthPoint[]>("path.ask-poly")
        .data([asks])
        .join("path")
        .attr("class", "ask-poly")
        .attr("fill", "rgba(74, 144, 217, 0.35)")
        .transition()
        .duration(TRANSITION_MS)
        .ease(d3.easeCubicOut)
        .attr("d", areaPoly);

      root
        .selectAll<SVGPathElement, DepthPoint[]>("path.ask-kalshi")
        .data([asks])
        .join("path")
        .attr("class", "ask-kalshi")
        .attr("fill", "rgba(245, 166, 35, 0.35)")
        .transition()
        .duration(TRANSITION_MS)
        .ease(d3.easeCubicOut)
        .attr("d", areaKalshi);

      root
        .selectAll<SVGTextElement, null>("text.spread-label")
        .data([null])
        .join("text")
        .attr("class", "spread-label")
        .attr("x", x(book.midpoint))
        .attr("y", 12)
        .attr("text-anchor", "middle")
        .attr("font-size", 11)
        .attr("fill", "currentColor")
        .style("opacity", 0.8)
        .text(`Spread ${(book.spread * 100).toFixed(1)}c`);
    };

    throttleTimerRef.current = setTimeout(render, RENDER_THROTTLE_MS);
  }, [ready, aggregatedBook, width]);

  function handleMouseMove(event: React.MouseEvent<SVGSVGElement>) {
    if (!svgRef.current || !aggregatedBook || !hoverDataRef.current || width === 0) return;

    const margins = { top: 16, right: 18, bottom: 30, left: 58 };
    const innerWidth = Math.max(0, width - margins.left - margins.right);

    const [mx, my] = d3.pointer(event, svgRef.current);
    const clampedX = Math.max(margins.left, Math.min(width - margins.right, mx));
    const clampedY = Math.max(margins.top, Math.min(CHART_HEIGHT - margins.bottom, my));

    const xMin = Math.max(0, aggregatedBook.bestBid - PRICE_PADDING);
    const xMax = Math.min(1, aggregatedBook.bestAsk + PRICE_PADDING);
    const x = d3.scaleLinear().domain([xMin, xMax]).range([0, innerWidth]);
    const price = x.invert(clampedX - margins.left);

    const bids = hoverDataRef.current.bids;
    const asks = hoverDataRef.current.asks;
    const bisect = d3.bisector((d: DepthPoint) => d.price).center;

    const bidPoint = bids.length > 0 ? bids[Math.max(0, Math.min(bids.length - 1, bisect(bids, price)))] : null;
    const askPoint = asks.length > 0 ? asks[Math.max(0, Math.min(asks.length - 1, bisect(asks, price)))] : null;

    setHover({
      price,
      bid: bidPoint,
      ask: askPoint,
      x: clampedX,
      y: clampedY,
    });
  }

  return (
    <Card className="min-h-[400px]">
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">Depth Chart</CardTitle>
      </CardHeader>
      <CardContent>
        {!aggregatedBook ? (
          <div className="space-y-2">
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-4 w-1/2" />
          </div>
        ) : (
          <div ref={containerRef} className="relative w-full">
            <svg
              ref={svgRef}
              width="100%"
              height={CHART_HEIGHT}
              onMouseMove={handleMouseMove}
              onMouseLeave={() => setHover(null)}
            />

            {hover && (
              <>
                <div
                  className="absolute top-0 bottom-0 w-px bg-border pointer-events-none"
                  style={{ left: `${hover.x}px` }}
                />
                <div
                  className="absolute z-20 rounded-md border bg-popover text-popover-foreground p-2 shadow-md text-xs pointer-events-none min-w-[190px]"
                  style={{
                    left: `${Math.min(width - 200, hover.x + 10)}px`,
                    top: `${Math.max(8, hover.y - 84)}px`,
                  }}
                >
                  <div className="font-medium mb-1">Price: {formatPrice(hover.price)}</div>
                  <div className="text-bid">
                    Bid cum: {formatSize(hover.bid?.totalCum ?? 0)}
                    <span className="text-muted-foreground">
                      {" "}
                      (P {formatSize(hover.bid?.polyCum ?? 0)} / K {formatSize(hover.bid?.kalshiCum ?? 0)})
                    </span>
                  </div>
                  <div className="text-ask">
                    Ask cum: {formatSize(hover.ask?.totalCum ?? 0)}
                    <span className="text-muted-foreground">
                      {" "}
                      (P {formatSize(hover.ask?.polyCum ?? 0)} / K {formatSize(hover.ask?.kalshiCum ?? 0)})
                    </span>
                  </div>
                </div>
              </>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
