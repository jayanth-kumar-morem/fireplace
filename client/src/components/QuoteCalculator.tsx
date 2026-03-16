import { useEffect, useMemo, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Table, TableBody, TableCell, TableFooter, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import { useOrderBookStore } from "@/store/orderBookStore";
import { useQuoteCalculator } from "@/hooks/useQuoteCalculator";

type Outcome = "yes" | "no";

function formatMoney(value: number): string {
  return `$${value.toFixed(2)}`;
}

function formatPrice(value: number): string {
  return `$${value.toFixed(4)}`;
}

function formatShares(value: number): string {
  return value.toFixed(2);
}

function formatPercent(value: number): string {
  return `${value.toFixed(2)}%`;
}

export function QuoteCalculator() {
  const aggregatedBook = useOrderBookStore((s) => s.aggregatedBook);
  const [amountText, setAmountText] = useState("");
  const [debouncedAmount, setDebouncedAmount] = useState(0);
  const [outcome, setOutcome] = useState<Outcome>("yes");

  useEffect(() => {
    const timeout = setTimeout(() => {
      const parsed = Number(amountText);
      setDebouncedAmount(Number.isFinite(parsed) && parsed > 0 ? parsed : 0);
    }, 200);
    return () => clearTimeout(timeout);
  }, [amountText]);

  const quote = useQuoteCalculator(debouncedAmount, outcome, aggregatedBook);

  const requestedAmount = useMemo(() => {
    const parsed = Number(amountText);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
  }, [amountText]);

  const hasAmount = requestedAmount > 0;
  const totalFillCost = quote ? requestedAmount - quote.unfilled : 0;
  const polySharePct = quote && quote.totalShares > 0 ? (quote.venueSplit.polymarket.shares / quote.totalShares) * 100 : 0;
  const kalshiSharePct = quote && quote.totalShares > 0 ? (quote.venueSplit.kalshi.shares / quote.totalShares) * 100 : 0;

  const polyAvgPrice =
    quote && quote.venueSplit.polymarket.shares > 0
      ? quote.venueSplit.polymarket.cost / quote.venueSplit.polymarket.shares
      : 0;
  const kalshiAvgPrice =
    quote && quote.venueSplit.kalshi.shares > 0 ? quote.venueSplit.kalshi.cost / quote.venueSplit.kalshi.shares : 0;

  return (
    <Card className="flex flex-col min-h-0 overflow-hidden">
      <CardHeader className="pb-3 flex-none">
        <CardTitle className="text-sm">Quote Calculator</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4 flex-1 overflow-y-auto">
        {!aggregatedBook ? (
          <div className="space-y-3">
            <Skeleton className="h-9 w-full" />
            <Skeleton className="h-6 w-1/2" />
            <Skeleton className="h-24 w-full" />
            <Skeleton className="h-40 w-full" />
          </div>
        ) : (
          <>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="quote-amount">Dollar amount</Label>
                <div className="flex items-center gap-2">
                  <span className="text-sm text-muted-foreground">$</span>
                  <Input
                    id="quote-amount"
                    type="number"
                    min="0"
                    step="0.01"
                    placeholder="100.00"
                    value={amountText}
                    onChange={(e) => setAmountText(e.target.value)}
                  />
                </div>
              </div>

              <div className="space-y-2">
                <Label>Outcome</Label>
                <RadioGroup
                  value={outcome}
                  onValueChange={(value) => setOutcome((value as Outcome) || "yes")}
                  className="grid grid-cols-2 gap-4"
                >
                  <Label htmlFor="quote-outcome-yes" className="gap-2 rounded-md border px-3 py-2">
                    <RadioGroupItem id="quote-outcome-yes" value="yes" />
                    YES
                  </Label>
                  <Label htmlFor="quote-outcome-no" className="gap-2 rounded-md border px-3 py-2">
                    <RadioGroupItem id="quote-outcome-no" value="no" />
                    NO
                  </Label>
                </RadioGroup>
              </div>
            </div>

            {!hasAmount || !quote || quote.totalShares <= 0 ? (
              <p className="text-sm text-muted-foreground">Enter an amount to see quote</p>
            ) : (
              <>
                <div className="rounded-lg border p-3">
                  <p className="text-sm">
                    You would receive <span className="font-semibold">{formatShares(quote.totalShares)} shares</span> at avg{" "}
                    <span className="font-semibold">{formatPrice(quote.avgPrice)}/share</span>
                  </p>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <Card size="sm">
                    <CardContent className="space-y-1">
                      <p className="text-xs text-muted-foreground">Implied Probability</p>
                      <p className="text-lg font-semibold">{formatPercent(quote.impliedProbability * 100)}</p>
                    </CardContent>
                  </Card>
                  <Card size="sm">
                    <CardContent className="space-y-1">
                      <p className="text-xs text-muted-foreground">Slippage</p>
                      <p className="text-lg font-semibold">{formatPercent(Math.max(0, quote.slippage))}</p>
                    </CardContent>
                  </Card>
                  <Card size="sm">
                    <CardContent className="space-y-1">
                      <p className="text-xs text-muted-foreground">Polymarket Fill %</p>
                      <p className="text-lg font-semibold">{formatPercent(polySharePct)}</p>
                    </CardContent>
                  </Card>
                  <Card size="sm">
                    <CardContent className="space-y-1">
                      <p className="text-xs text-muted-foreground">Kalshi Fill %</p>
                      <p className="text-lg font-semibold">{formatPercent(kalshiSharePct)}</p>
                    </CardContent>
                  </Card>
                </div>

                <div className="space-y-2">
                  <p className="text-xs text-muted-foreground">Venue Split</p>
                  <div className="h-7 rounded-md overflow-hidden border flex">
                    <div
                      className="h-full bg-polymarket text-[10px] text-white flex items-center justify-center"
                      style={{ width: `${polySharePct}%` }}
                    >
                      {polySharePct >= 10 ? `${polySharePct.toFixed(1)}%` : ""}
                    </div>
                    <div
                      className="h-full bg-kalshi text-[10px] text-black flex items-center justify-center"
                      style={{ width: `${kalshiSharePct}%` }}
                    >
                      {kalshiSharePct >= 10 ? `${kalshiSharePct.toFixed(1)}%` : ""}
                    </div>
                  </div>
                </div>

                <div className="rounded-lg border overflow-hidden">
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Venue</TableHead>
                        <TableHead>Shares</TableHead>
                        <TableHead>Avg Price</TableHead>
                        <TableHead className="text-right">Cost</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      <TableRow>
                        <TableCell>
                          <span className="inline-flex items-center gap-2">
                            <span className="h-2 w-2 rounded-full bg-polymarket" />
                            Polymarket
                          </span>
                        </TableCell>
                        <TableCell>{formatShares(quote.venueSplit.polymarket.shares)}</TableCell>
                        <TableCell>{formatPrice(polyAvgPrice)}</TableCell>
                        <TableCell className="text-right">{formatMoney(quote.venueSplit.polymarket.cost)}</TableCell>
                      </TableRow>
                      <TableRow>
                        <TableCell>
                          <span className="inline-flex items-center gap-2">
                            <span className="h-2 w-2 rounded-full bg-kalshi" />
                            Kalshi
                          </span>
                        </TableCell>
                        <TableCell>{formatShares(quote.venueSplit.kalshi.shares)}</TableCell>
                        <TableCell>{formatPrice(kalshiAvgPrice)}</TableCell>
                        <TableCell className="text-right">{formatMoney(quote.venueSplit.kalshi.cost)}</TableCell>
                      </TableRow>
                    </TableBody>
                    <TableFooter>
                      <TableRow>
                        <TableCell>Total</TableCell>
                        <TableCell>{formatShares(quote.totalShares)}</TableCell>
                        <TableCell>{formatPrice(quote.avgPrice)}</TableCell>
                        <TableCell className="text-right">{formatMoney(totalFillCost)}</TableCell>
                      </TableRow>
                    </TableFooter>
                  </Table>
                </div>

                {quote.unfilled > 0 && (
                  <Alert variant="destructive">
                    <AlertDescription>
                      Only {formatMoney(totalFillCost)} of {formatMoney(requestedAmount)} could be filled. Insufficient liquidity.
                    </AlertDescription>
                  </Alert>
                )}
              </>
            )}

            <p className="text-xs text-muted-foreground">Fees not included. Actual execution cost varies by venue.</p>
          </>
        )}
      </CardContent>
    </Card>
  );
}
