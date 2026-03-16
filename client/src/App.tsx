import { TooltipProvider } from "@/components/ui/tooltip";
import { useWebSocket } from "@/hooks/useWebSocket";
import { ConnectionStatus } from "@/components/ConnectionStatus";
import { MarketHeader } from "@/components/MarketHeader";
import { OrderBookLadder } from "@/components/OrderBookLadder";

import { QuoteCalculator } from "@/components/QuoteCalculator";

function App() {
  useWebSocket();

  return (
    <TooltipProvider>
      <div className="h-screen flex flex-col bg-background text-foreground overflow-hidden">
        <ConnectionStatus />

        <main className="flex-1 max-w-7xl w-full mx-auto p-4 flex flex-col gap-4 min-h-0">
          <MarketHeader />

          <div className="flex-1 grid grid-cols-1 lg:grid-cols-[1fr_1fr] gap-4 min-h-0">
            <OrderBookLadder />
            <QuoteCalculator />
          </div>
        </main>
      </div>
    </TooltipProvider>
  );
}

export default App;
