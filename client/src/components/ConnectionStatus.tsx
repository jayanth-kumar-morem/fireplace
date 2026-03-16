import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { useOrderBookStore } from "@/store/orderBookStore";
import type { ConnectionStatus as ConnStatus, Venue } from "@pma/shared";

function statusColor(s: ConnStatus): string {
  if (s === "connected") return "bg-bid";
  if (s === "reconnecting" || s === "stale") return "bg-warning";
  return "bg-ask";
}

function statusLabel(s: ConnStatus): string {
  if (s === "connected") return "Live";
  if (s === "reconnecting") return "Reconnecting";
  if (s === "stale") return "Stale";
  return "Offline";
}

function RelativeTime({ timestamp }: { timestamp: number | null }) {
  const [text, setText] = useState("");

  useEffect(() => {
    if (!timestamp) {
      setText("");
      return;
    }

    function update() {
      const elapsed = Math.round((Date.now() - timestamp!) / 1000);
      if (elapsed < 1) setText("just now");
      else if (elapsed < 60) setText(`${elapsed}s ago`);
      else setText(`${Math.round(elapsed / 60)}m ago`);
    }

    update();
    const id = setInterval(update, 1000);
    return () => clearInterval(id);
  }, [timestamp]);

  if (!text) return null;
  return <span className="text-muted-foreground">{text}</span>;
}

function VenueBadge({ venue, label }: { venue: Venue; label: string }) {
  const status = useOrderBookStore((s) => s.connections[venue]);
  const lastUpdated = useOrderBookStore((s) => s.lastUpdated[venue]);

  return (
    <Badge variant="outline" className="gap-1.5 text-xs">
      <span className={`h-2 w-2 rounded-full ${statusColor(status)}`} />
      {label}
      <span className="text-muted-foreground">·</span>
      {lastUpdated ? <RelativeTime timestamp={lastUpdated} /> : (
        <span className="text-muted-foreground">{statusLabel(status)}</span>
      )}
    </Badge>
  );
}

export function ConnectionStatus() {
  const connections = useOrderBookStore((s) => s.connections);
  const clientStatus = useOrderBookStore((s) => s.clientStatus);

  const degradedVenues: string[] = [];
  if (connections.polymarket !== "connected") degradedVenues.push("Polymarket");
  if (connections.kalshi !== "connected") degradedVenues.push("Kalshi");

  return (
    <>
      <header className="sticky top-0 z-50 border-b border-border bg-background/95 backdrop-blur px-4 py-2">
        <div className="max-w-7xl mx-auto flex items-center justify-between">
          <div className="flex items-center gap-4">
            <span className="text-sm font-semibold tracking-tight text-foreground">
              Prediction Market Aggregator
            </span>
            <div className="flex items-center gap-2">
              <VenueBadge venue="polymarket" label="Polymarket" />
              <VenueBadge venue="kalshi" label="Kalshi" />
            </div>
          </div>

          {clientStatus !== "connected" && (
            <Badge
              variant="secondary"
              className="text-xs gap-1.5"
            >
              <span className={`h-1.5 w-1.5 rounded-full ${
                clientStatus === "connecting" || clientStatus === "reconnecting"
                  ? "bg-warning animate-pulse"
                  : "bg-ask"
              }`} />
              {clientStatus === "connecting" ? "Connecting" :
               clientStatus === "reconnecting" ? "Reconnecting" : "Disconnected"}
            </Badge>
          )}
        </div>
      </header>

      {degradedVenues.length > 0 && (
        <div className="max-w-7xl mx-auto px-4 pt-2">
          <Alert variant="destructive" className="bg-warning/10 border-warning/30 text-warning">
            <AlertDescription className="text-sm">
              Data from {degradedVenues.join(" and ")} may be delayed.
            </AlertDescription>
          </Alert>
        </div>
      )}
    </>
  );
}
