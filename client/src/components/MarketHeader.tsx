import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { useOrderBookStore } from "@/store/orderBookStore";

function formatCents(price: number): string {
  return `${(price * 100).toFixed(1)}¢`;
}

export function MarketHeader() {
  const market = useOrderBookStore((s) => s.market);
  const book = useOrderBookStore((s) => s.aggregatedBook);

  if (!market || !book) {
    return (
      <Card>
        <CardHeader className="pb-3">
          <Skeleton className="h-6 w-3/4" />
        </CardHeader>
        <CardContent>
          <Skeleton className="h-4 w-1/2" />
        </CardContent>
      </Card>
    );
  }

  const yesPrice = book.bestBid;
  const noPrice = 1 - yesPrice;
  const yesPct = (yesPrice * 100).toFixed(1);

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-lg leading-snug">
          {market.title}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {/* Price badges */}
        <div className="flex items-center gap-3 flex-wrap">
          <Badge className="bg-bid/15 text-bid border-bid/30 hover:bg-bid/20 text-base px-3 py-1">
            YES {formatCents(yesPrice)}
          </Badge>
          <Badge className="bg-ask/15 text-ask border-ask/30 hover:bg-ask/20 text-base px-3 py-1">
            NO {formatCents(noPrice)}
          </Badge>
          <div className="flex items-center gap-3 text-sm text-muted-foreground ml-auto">
            <span>Spread: {formatCents(book.spread)}</span>
            <span>Mid: {formatCents(book.midpoint)}</span>
          </div>
        </div>

        {/* Probability bar */}
        <div className="relative h-2.5 rounded-full bg-muted overflow-hidden">
          <div
            className="absolute inset-y-0 left-0 bg-bid rounded-full transition-all duration-300"
            style={{ width: `${yesPct}%` }}
          />
          <div
            className="absolute inset-y-0 right-0 bg-ask rounded-full transition-all duration-300"
            style={{ width: `${(100 - parseFloat(yesPct))}%` }}
          />
        </div>
        <div className="flex justify-between text-xs text-muted-foreground">
          <span>YES {yesPct}%</span>
          <span>{(100 - parseFloat(yesPct)).toFixed(1)}% NO</span>
        </div>
      </CardContent>
    </Card>
  );
}
