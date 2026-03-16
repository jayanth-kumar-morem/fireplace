import { useMemo, useCallback, useRef, useEffect } from "react";
import { List, useListRef } from "react-window";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Skeleton } from "@/components/ui/skeleton";
import { useOrderBookStore } from "@/store/orderBookStore";
import { aggregate } from "@/lib/aggregator";
import { PriceLevelRow, SpreadRow } from "./PriceLevel";
import type { AggregatedBook, AggregatedLevel, ViewMode } from "@pma/shared";
import type { CSSProperties } from "react";

const ROW_HEIGHT = 32;
const MAX_LEVELS = 50;

function Header() {
  return (
    <div className="grid grid-cols-[1fr_80px_1fr] items-center px-2 h-7 text-[10px] uppercase tracking-wider text-muted-foreground border-b border-border select-none">
      <div className="text-right">Bid Size</div>
      <div className="text-center">Price</div>
      <div className="text-left">Ask Size</div>
    </div>
  );
}

// T8.5
function useDisplayBook(): AggregatedBook | null {
  const viewMode = useOrderBookStore((s) => s.viewMode);
  const aggregatedBook = useOrderBookStore((s) => s.aggregatedBook);
  const polymarketBook = useOrderBookStore((s) => s.polymarketBook);
  const kalshiBook = useOrderBookStore((s) => s.kalshiBook);

  return useMemo(() => {
    if (viewMode === "aggregated") return aggregatedBook;
    if (viewMode === "polymarket" && polymarketBook) return aggregate(polymarketBook);
    if (viewMode === "kalshi" && kalshiBook) return aggregate(kalshiBook);
    return aggregatedBook;
  }, [viewMode, aggregatedBook, polymarketBook, kalshiBook]);
}

interface RowItem {
  type: "bid" | "ask" | "spread";
  level?: AggregatedLevel;
  spread?: number;
  midpoint?: number;
}

function useRows(book: AggregatedBook | null) {
  return useMemo(() => {
    if (!book) return { rows: [] as RowItem[], maxSize: 0, spreadIndex: -1 };

    const topAsks = book.asks.slice(0, MAX_LEVELS).reverse();
    const topBids = book.bids.slice(0, MAX_LEVELS);

    const askRows: RowItem[] = topAsks.map((level) => ({ type: "ask", level }));
    const spreadRow: RowItem = { type: "spread", spread: book.spread, midpoint: book.midpoint };
    const bidRows: RowItem[] = topBids.map((level) => ({ type: "bid", level }));

    const rows = [...askRows, spreadRow, ...bidRows];
    const spreadIndex = askRows.length;

    let maxSize = 0;
    for (const b of topBids) if (b.totalSize > maxSize) maxSize = b.totalSize;
    for (const a of topAsks) if (a.totalSize > maxSize) maxSize = a.totalSize;

    return { rows, maxSize, spreadIndex };
  }, [book]);
}

// RowProps type — only custom props, not index/style/ariaAttributes
interface RowExtraProps {
  rows: RowItem[];
  maxSize: number;
}

function RowComponent(props: {
  index: number;
  style: CSSProperties;
  ariaAttributes: Record<string, unknown>;
} & RowExtraProps) {
  const row = props.rows[props.index];
  return (
    <div style={props.style}>
      {row.type === "spread" ? (
        <SpreadRow spread={row.spread!} midpoint={row.midpoint!} />
      ) : (
        <PriceLevelRow level={row.level!} side={row.type} maxSize={props.maxSize} />
      )}
    </div>
  );
}

export function OrderBookLadder() {
  const viewMode = useOrderBookStore((s) => s.viewMode);
  const setViewMode = useOrderBookStore((s) => s.setViewMode);
  const book = useDisplayBook();
  const { rows, maxSize, spreadIndex } = useRows(book);
  const listRef = useListRef(null);

  const hasScrolled = useRef(false);
  useEffect(() => {
    if (spreadIndex > 0 && !hasScrolled.current && listRef.current) {
      listRef.current.scrollToRow({ index: Math.max(0, spreadIndex - 5), align: "start" });
      hasScrolled.current = true;
    }
  }, [spreadIndex, listRef]);

  const handleViewChange = useCallback(
    (value: string[]) => {
      if (value.length > 0) setViewMode(value[0] as ViewMode);
    },
    [setViewMode]
  );

  const listHeight = Math.min(rows.length * ROW_HEIGHT, 25 * ROW_HEIGHT);

  return (
    <Card className="flex flex-col min-h-0 overflow-hidden">
      <CardHeader className="pb-2 flex-none">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">Order Book</CardTitle>
          <ToggleGroup
            value={[viewMode]}
            onValueChange={handleViewChange}
            className="h-7"
          >
            <ToggleGroupItem value="aggregated" className="text-[10px] px-2 h-6">
              Combined
            </ToggleGroupItem>
            <ToggleGroupItem value="polymarket" className="text-[10px] px-2 h-6">
              <span className="h-1.5 w-1.5 rounded-full bg-polymarket mr-1" />
              Poly
            </ToggleGroupItem>
            <ToggleGroupItem value="kalshi" className="text-[10px] px-2 h-6">
              <span className="h-1.5 w-1.5 rounded-full bg-kalshi mr-1" />
              Kalshi
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
      </CardHeader>
      <CardContent className="flex-1 p-0 overflow-hidden">
        {rows.length === 0 ? (
          <div className="p-4 space-y-2">
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-4 w-1/2" />
          </div>
        ) : (
          <>
            <Header />
            <List<RowExtraProps>
              listRef={listRef}
              defaultHeight={listHeight}
              rowCount={rows.length}
              rowHeight={ROW_HEIGHT}
              rowComponent={RowComponent}
              rowProps={{ rows, maxSize }}
              overscanCount={5}
              style={{ width: "100%", height: listHeight }}
            />
          </>
        )}
      </CardContent>
    </Card>
  );
}
