import { useEffect, useRef } from "react";
import type { ServerMessage } from "@pma/shared";
import { useOrderBookStore } from "@/store/orderBookStore";

const WS_URL =
  (window.location.protocol === "https:" ? "wss://" : "ws://") +
  window.location.host +
  "/ws";

const RECONNECT_BASE_MS = 1_000;
const RECONNECT_MAX_MS = 30_000;
const MAX_RECONNECT_ATTEMPTS = 10;
const MAX_BUFFER_SIZE = 1_000;

export function useWebSocket(): void {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectAttempt = useRef(0);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const bufferRef = useRef<ServerMessage[]>([]);
  const rafRef = useRef<number>(0);
  const isVisibleRef = useRef(true);
  const destroyedRef = useRef(false);

  // Pull store actions once (stable references)
  const applySnapshot = useOrderBookStore.getState().applySnapshot;
  const applyBookUpdate = useOrderBookStore.getState().applyBookUpdate;
  const updateConnectionStatus = useOrderBookStore.getState().updateConnectionStatus;
  const setClientStatus = useOrderBookStore.getState().setClientStatus;

  useEffect(() => {
    destroyedRef.current = false;

    // ─── Message Processing ──────────────────────

    function processMessage(msg: ServerMessage): void {
      switch (msg.type) {
        case "snapshot":
          applySnapshot(msg);
          break;
        case "book_update":
          applyBookUpdate(msg);
          break;
        case "connection_status":
          updateConnectionStatus(msg.venue, msg.status);
          break;
        case "heartbeat":
          // Update connection states from heartbeat
          for (const [venue, status] of Object.entries(msg.connections)) {
            updateConnectionStatus(
              venue as "polymarket" | "kalshi",
              status
            );
          }
          break;
      }
    }

    function flushBuffer(): void {
      const batch = bufferRef.current.splice(0);
      for (const msg of batch) {
        processMessage(msg);
      }
      rafRef.current = 0;
    }

    function scheduleFlush(): void {
      if (rafRef.current) return; // Already scheduled
      if (!isVisibleRef.current) return; // Paused while hidden

      rafRef.current = requestAnimationFrame(flushBuffer);
    }

    // ─── WebSocket Connection ────────────────────

    function connect(): void {
      if (destroyedRef.current) return;

      setClientStatus("connecting");
      const ws = new WebSocket(WS_URL);
      wsRef.current = ws;

      ws.onopen = () => {
        reconnectAttempt.current = 0;
        setClientStatus("connected");
      };

      ws.onmessage = (event) => {
        try {
          const msg: ServerMessage = JSON.parse(event.data);

          // Buffer the message
          if (bufferRef.current.length >= MAX_BUFFER_SIZE) {
            // Drop oldest to prevent unbounded growth
            bufferRef.current.shift();
          }
          bufferRef.current.push(msg);

          scheduleFlush();
        } catch {
          // Ignore non-JSON (e.g. "pong")
        }
      };

      ws.onclose = () => {
        if (!destroyedRef.current) {
          setClientStatus("reconnecting");
          scheduleReconnect();
        }
      };

      ws.onerror = () => {
        // onclose will fire after this
      };
    }

    // ─── Reconnection ────────────────────────────

    function scheduleReconnect(): void {
      if (destroyedRef.current) return;

      if (reconnectAttempt.current >= MAX_RECONNECT_ATTEMPTS) {
        setClientStatus("disconnected");
        // Slow retry after max attempts
        reconnectTimer.current = setTimeout(() => {
          reconnectAttempt.current = 0;
          connect();
        }, RECONNECT_MAX_MS * 2);
        return;
      }

      const delay = Math.min(
        RECONNECT_BASE_MS * Math.pow(2, reconnectAttempt.current) +
          Math.random() * 1000,
        RECONNECT_MAX_MS
      );
      reconnectAttempt.current++;

      reconnectTimer.current = setTimeout(connect, delay);
    }

    // ─── T6.6: Page Visibility API ───────────────

    function handleVisibility(): void {
      isVisibleRef.current = !document.hidden;

      if (isVisibleRef.current) {
        // Tab became visible — flush any buffered messages immediately
        if (bufferRef.current.length > 0) {
          flushBuffer();
        }
      }
      // When hidden: RAF callbacks won't fire (browser throttles them),
      // and scheduleFlush() will skip. Messages keep buffering in bufferRef.
    }

    document.addEventListener("visibilitychange", handleVisibility);

    // ─── Start ───────────────────────────────────

    connect();

    // ─── Cleanup ─────────────────────────────────

    return () => {
      destroyedRef.current = true;
      document.removeEventListener("visibilitychange", handleVisibility);

      if (reconnectTimer.current) {
        clearTimeout(reconnectTimer.current);
      }
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = 0;
      }
      if (wsRef.current) {
        wsRef.current.onclose = null; // Prevent reconnect on intentional close
        wsRef.current.close();
        wsRef.current = null;
      }

      bufferRef.current = [];
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
}
