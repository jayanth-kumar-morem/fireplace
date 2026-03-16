use crate::aggregator::{snapshot_message_from_state, SharedAppState};
use crate::config::HEARTBEAT_INTERVAL_MS;
use crate::feeds::now_ms;
use crate::types::ServerMessage;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch};

#[derive(Clone)]
pub struct AppContext {
    pub state: SharedAppState,
    pub broadcast_tx: broadcast::Sender<Arc<str>>,
    pub client_count: Arc<AtomicUsize>,
    pub started_at: Arc<Instant>,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(ctx): State<AppContext>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, ctx))
}

async fn handle_socket(mut socket: WebSocket, ctx: AppContext) {
    ctx.client_count.fetch_add(1, Ordering::SeqCst);

    let snapshot = {
        let guard = ctx.state.read().await;
        snapshot_message_from_state(&guard)
    };
    if let Ok(snapshot_text) = serde_json::to_string(&snapshot) {
        let _ = socket.send(Message::Text(snapshot_text.into())).await;
    }

    let mut rx = ctx.broadcast_tx.subscribe();
    loop {
        tokio::select! {
            outgoing = rx.recv() => {
                match outgoing {
                    Ok(payload) => {
                        if socket.send(Message::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if text.trim().eq_ignore_ascii_case("ping") {
                            let _ = socket.send(Message::Text("pong".into())).await;
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }

    ctx.client_count.fetch_sub(1, Ordering::SeqCst);
}

pub async fn health_handler(State(ctx): State<AppContext>) -> Json<serde_json::Value> {
    let guard = ctx.state.read().await;
    let uptime = ctx.started_at.elapsed().as_secs_f64();
    Json(json!({
        "status": "ok",
        "uptime": uptime,
        "connections": guard.connections,
        "clients": ctx.client_count.load(Ordering::SeqCst)
    }))
}

pub async fn snapshot_handler(State(ctx): State<AppContext>) -> Json<ServerMessage> {
    let guard = ctx.state.read().await;
    Json(snapshot_message_from_state(&guard))
}

pub async fn run_heartbeat(ctx: AppContext, shutdown_rx: watch::Receiver<bool>) {
    run_heartbeat_with_interval(ctx, shutdown_rx, Duration::from_millis(HEARTBEAT_INTERVAL_MS)).await;
}

async fn run_heartbeat_with_interval(
    ctx: AppContext,
    mut shutdown_rx: watch::Receiver<bool>,
    interval: Duration,
) {
    let mut tick = tokio::time::interval(interval);
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            _ = tick.tick() => {
                let connections = {
                    let guard = ctx.state.read().await;
                    guard.connections.clone()
                };
                let heartbeat = ServerMessage::Heartbeat {
                    timestamp: now_ms(),
                    connections,
                };
                if let Ok(text) = serde_json::to_string(&heartbeat) {
                    let _ = ctx.broadcast_tx.send(Arc::<str>::from(text.into_boxed_str()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregator::AppState;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn snapshot_handler_returns_snapshot_message() {
        let state = Arc::new(RwLock::new(AppState::default()));
        let (tx, _) = broadcast::channel(8);
        let ctx = AppContext {
            state,
            broadcast_tx: tx,
            client_count: Arc::new(AtomicUsize::new(0)),
            started_at: Arc::new(Instant::now()),
        };

        let Json(msg) = snapshot_handler(State(ctx)).await;
        match msg {
            ServerMessage::Snapshot { .. } => {}
            _ => panic!("expected snapshot message"),
        }
    }

    #[tokio::test]
    async fn heartbeat_task_broadcasts_heartbeat_payload() {
        let state = Arc::new(RwLock::new(AppState::default()));
        let (tx, mut rx) = broadcast::channel(8);
        let ctx = AppContext {
            state,
            broadcast_tx: tx,
            client_count: Arc::new(AtomicUsize::new(0)),
            started_at: Arc::new(Instant::now()),
        };
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let handle = tokio::spawn(run_heartbeat_with_interval(
            ctx,
            shutdown_rx,
            Duration::from_millis(20),
        ));

        let payload = tokio::time::timeout(Duration::from_millis(120), rx.recv())
            .await
            .expect("heartbeat should be emitted in time")
            .expect("broadcast payload should be available");
        assert!(payload.contains("\"type\":\"heartbeat\""));
        let _ = shutdown_tx.send(true);
        let _ = handle.await;
    }
}
