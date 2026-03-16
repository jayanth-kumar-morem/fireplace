use crate::config::{
    MAX_BOOK_DEPTH, POLYMARKET_CLOB_BASE, POLYMARKET_PING_INTERVAL_MS, POLYMARKET_WS_URL,
    POLYMARKET_YES_TOKEN_ID, RECONNECT_BASE_DELAY_MS, RECONNECT_MAX_ATTEMPTS,
    RECONNECT_MAX_DELAY_MS, TICK_SIZE,
};
use crate::feeds::reconnect::ReconnectState;
use crate::feeds::{now_ms, round_to_tick, FeedEvent};
use crate::types::{
    BookChange, BookSide, ConnectionStatus, NormalizedBook, PolymarketBookEvent,
    PolymarketBookResponse, PolymarketPriceChangeEvent, PolymarketRawLevel, PriceLevel, Venue,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct PolymarketState {
    pub status: ConnectionStatus,
    pub book: NormalizedBook,
    pub last_message_timestamp: u64,
    pub last_snapshot_hash: Option<String>,
}

impl Default for PolymarketState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            book: NormalizedBook {
                bids: vec![],
                asks: vec![],
                last_updated: 0,
            },
            last_message_timestamp: 0,
            last_snapshot_hash: None,
        }
    }
}

pub async fn fetch_snapshot(
    client: &reqwest::Client,
) -> Result<PolymarketBookResponse, reqwest::Error> {
    let url = format!("{POLYMARKET_CLOB_BASE}/book?token_id={POLYMARKET_YES_TOKEN_ID}");
    client.get(url).send().await?.error_for_status()?.json().await
}

pub fn parse_raw_levels(raw: &[PolymarketRawLevel]) -> Vec<PriceLevel> {
    raw.iter()
        .filter_map(|l| {
            let price = l.price.parse::<f64>().ok()?;
            let size = l.size.parse::<f64>().ok()?;
            Some(PriceLevel {
                price: round_to_tick(price, TICK_SIZE),
                size,
                venue: Venue::Polymarket,
            })
        })
        .collect()
}

fn sort_and_cap(levels: &mut [PriceLevel], side: BookSide) {
    match side {
        BookSide::Bid => levels.sort_by(|a, b| b.price.total_cmp(&a.price)),
        BookSide::Ask => levels.sort_by(|a, b| a.price.total_cmp(&b.price)),
    }
}

pub fn build_snapshot_book(snapshot: &PolymarketBookResponse) -> NormalizedBook {
    let mut bids = parse_raw_levels(&snapshot.bids);
    let mut asks = parse_raw_levels(&snapshot.asks);
    sort_and_cap(&mut bids, BookSide::Bid);
    sort_and_cap(&mut asks, BookSide::Ask);
    bids.truncate(MAX_BOOK_DEPTH);
    asks.truncate(MAX_BOOK_DEPTH);

    NormalizedBook {
        bids,
        asks,
        last_updated: now_ms(),
    }
}

pub fn apply_level_update(
    levels: &mut Vec<PriceLevel>,
    price: f64,
    size: f64,
    side: BookSide,
) -> BookChange {
    let rounded = round_to_tick(price, TICK_SIZE);
    if let Some(existing) = levels.iter_mut().find(|l| l.price == rounded) {
        if size <= 0.0 {
            levels.retain(|l| l.price != rounded);
        } else {
            existing.size = size;
        }
    } else if size > 0.0 {
        levels.push(PriceLevel {
            price: rounded,
            size,
            venue: Venue::Polymarket,
        });
    }

    sort_and_cap(levels, side);
    levels.truncate(MAX_BOOK_DEPTH);

    BookChange {
        side,
        price: rounded,
        size,
    }
}

pub fn reconcile_with_snapshot_data(
    state: &mut PolymarketState,
    snapshot: PolymarketBookResponse,
) -> bool {
    if state.last_snapshot_hash.as_deref() == Some(snapshot.hash.as_str()) {
        return false;
    }

    state.book = build_snapshot_book(&snapshot);
    state.last_snapshot_hash = Some(snapshot.hash);
    state.last_message_timestamp = now_ms();
    true
}

pub async fn reconcile_with_rest(
    state: &mut PolymarketState,
    client: &reqwest::Client,
) -> Result<bool, reqwest::Error> {
    let snapshot = fetch_snapshot(client).await?;
    Ok(reconcile_with_snapshot_data(state, snapshot))
}

async fn send_status(
    tx: &mpsc::Sender<FeedEvent>,
    status: ConnectionStatus,
) -> Result<(), mpsc::error::SendError<FeedEvent>> {
    tx.send(FeedEvent::StatusChange {
        venue: Venue::Polymarket,
        status,
    })
    .await
}

async fn connect_once(
    state: &mut PolymarketState,
    tx: &mpsc::Sender<FeedEvent>,
    client: &reqwest::Client,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let snapshot = fetch_snapshot(client).await?;
    state.book = build_snapshot_book(&snapshot);
    state.last_snapshot_hash = Some(snapshot.hash);
    state.last_message_timestamp = now_ms();

    tx.send(FeedEvent::Snapshot {
        venue: Venue::Polymarket,
        book: state.book.clone(),
    })
    .await?;

    let (stream, _response) = connect_async(POLYMARKET_WS_URL).await?;
    let (mut write, mut read) = stream.split();

    write
        .send(Message::Text(
            serde_json::json!({
                "assets_ids": [POLYMARKET_YES_TOKEN_ID],
                "type": "market"
            })
            .to_string()
            .into(),
        ))
        .await?;

    state.status = ConnectionStatus::Connected;
    send_status(tx, ConnectionStatus::Connected).await?;
    info!("polymarket connected");

    let mut ping_interval = tokio::time::interval(Duration::from_millis(POLYMARKET_PING_INTERVAL_MS));

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    let _ = write.close().await;
                    return Ok(());
                }
            }
            _ = ping_interval.tick() => {
                let _ = write.send(Message::Text("PING".into())).await;
            }
            maybe_message = read.next() => {
                let Some(message) = maybe_message else {
                    return Err("polymarket websocket stream ended".into());
                };
                match message {
                    Ok(Message::Text(text)) => {
                        if text == "PONG" {
                            continue;
                        }
                        state.last_message_timestamp = now_ms();
                        handle_polymarket_message(state, tx, text.as_ref()).await?;
                    }
                    Ok(Message::Binary(bytes)) => {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            state.last_message_timestamp = now_ms();
                            handle_polymarket_message(state, tx, &text).await?;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        return Err("polymarket websocket closed".into());
                    }
                    Ok(_) => {}
                    Err(err) => return Err(Box::new(err)),
                }
            }
        }
    }
}

async fn handle_polymarket_message(
    state: &mut PolymarketState,
    tx: &mpsc::Sender<FeedEvent>,
    text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let event_type = value.get("event_type").and_then(Value::as_str).unwrap_or_default();

    match event_type {
        "book" => {
            let event: PolymarketBookEvent = serde_json::from_value(value)?;
            let snapshot = PolymarketBookResponse {
                market: event.market,
                asset_id: event.asset_id,
                bids: event.bids,
                asks: event.asks,
                timestamp: event.timestamp,
                hash: event.hash.unwrap_or_default(),
            };
            state.book = build_snapshot_book(&snapshot);
            if !snapshot.hash.is_empty() {
                state.last_snapshot_hash = Some(snapshot.hash);
            }
            tx.send(FeedEvent::Snapshot {
                venue: Venue::Polymarket,
                book: state.book.clone(),
            })
            .await?;
        }
        "price_change" => {
            let event: PolymarketPriceChangeEvent = serde_json::from_value(value)?;
            let mut changes = Vec::new();
            for entry in event.price_changes {
                if entry.asset_id != POLYMARKET_YES_TOKEN_ID {
                    continue;
                }
                let price = match entry.price.parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let size = match entry.size.parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                match entry.side {
                    crate::types::PolymarketSide::BUY => {
                        changes.push(apply_level_update(
                            &mut state.book.bids,
                            price,
                            size,
                            BookSide::Bid,
                        ));
                    }
                    crate::types::PolymarketSide::SELL => {
                        changes.push(apply_level_update(
                            &mut state.book.asks,
                            price,
                            size,
                            BookSide::Ask,
                        ));
                    }
                }
            }
            if !changes.is_empty() {
                state.book.last_updated = now_ms();
                tx.send(FeedEvent::BookChange {
                    venue: Venue::Polymarket,
                    changes,
                })
                .await?;
            }
        }
        _ => {}
    }

    Ok(())
}

pub async fn run_polymarket_feed(
    feed_tx: mpsc::Sender<FeedEvent>,
    http_client: reqwest::Client,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut state = PolymarketState::default();
    let mut reconnect = ReconnectState::new(
        RECONNECT_MAX_ATTEMPTS,
        Duration::from_millis(RECONNECT_BASE_DELAY_MS),
        Duration::from_millis(RECONNECT_MAX_DELAY_MS),
    );

    let _ = send_status(&feed_tx, ConnectionStatus::Reconnecting).await;
    state.status = ConnectionStatus::Reconnecting;

    loop {
        if *shutdown_rx.borrow() {
            let _ = send_status(&feed_tx, ConnectionStatus::Disconnected).await;
            return;
        }

        match connect_once(&mut state, &feed_tx, &http_client, &mut shutdown_rx).await {
            Ok(_) => {
                if *shutdown_rx.borrow() {
                    let _ = send_status(&feed_tx, ConnectionStatus::Disconnected).await;
                    return;
                }
            }
            Err(err) => {
                warn!("polymarket connection cycle ended: {err}");
            }
        }

        state.status = ConnectionStatus::Reconnecting;
        let _ = send_status(&feed_tx, ConnectionStatus::Reconnecting).await;

        if let Some(delay) = reconnect.next_delay() {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        let _ = send_status(&feed_tx, ConnectionStatus::Disconnected).await;
                        return;
                    }
                }
                _ = tokio::time::sleep(delay) => {}
            }
        } else {
            error!("polymarket max reconnect attempts reached; entering slow retry mode");
            let _ = send_status(&feed_tx, ConnectionStatus::Disconnected).await;
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        return;
                    }
                }
                _ = tokio::time::sleep(reconnect.slow_retry_delay()) => {
                    reconnect.reset();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(price: &str, size: &str) -> PolymarketRawLevel {
        PolymarketRawLevel {
            price: price.to_string(),
            size: size.to_string(),
        }
    }

    #[test]
    fn snapshot_normalization_is_sorted_and_capped() {
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        for i in 0..250 {
            let bid_price = format!("{:.3}", 0.10 + (i as f64) * 0.001);
            let ask_price = format!("{:.3}", 0.20 + (i as f64) * 0.001);
            bids.push(raw(&bid_price, "1"));
            asks.push(raw(&ask_price, "1"));
        }

        let snapshot = PolymarketBookResponse {
            market: "m".into(),
            asset_id: "a".into(),
            bids,
            asks,
            timestamp: "1".into(),
            hash: "h1".into(),
        };

        let book = build_snapshot_book(&snapshot);
        assert_eq!(book.bids.len(), MAX_BOOK_DEPTH);
        assert_eq!(book.asks.len(), MAX_BOOK_DEPTH);
        assert!(book.bids.windows(2).all(|w| w[0].price >= w[1].price));
        assert!(book.asks.windows(2).all(|w| w[0].price <= w[1].price));
    }

    #[test]
    fn apply_level_update_insert_update_remove_bid_and_ask() {
        let mut bids = vec![
            PriceLevel {
                price: 0.20,
                size: 10.0,
                venue: Venue::Polymarket,
            },
            PriceLevel {
                price: 0.19,
                size: 5.0,
                venue: Venue::Polymarket,
            },
        ];
        let mut asks = vec![
            PriceLevel {
                price: 0.21,
                size: 7.0,
                venue: Venue::Polymarket,
            },
            PriceLevel {
                price: 0.22,
                size: 8.0,
                venue: Venue::Polymarket,
            },
        ];

        let _ = apply_level_update(&mut bids, 0.205, 12.0, BookSide::Bid);
        assert_eq!(bids[0].price, 0.21);
        let _ = apply_level_update(&mut bids, 0.19, 9.0, BookSide::Bid);
        assert_eq!(bids.iter().find(|l| l.price == 0.19).map(|l| l.size), Some(9.0));
        let _ = apply_level_update(&mut bids, 0.20, 0.0, BookSide::Bid);
        assert!(bids.iter().all(|l| l.price != 0.20));

        let _ = apply_level_update(&mut asks, 0.205, 11.0, BookSide::Ask);
        assert_eq!(asks[0].price, 0.21);
        let _ = apply_level_update(&mut asks, 0.22, 4.0, BookSide::Ask);
        assert_eq!(asks.iter().find(|l| l.price == 0.22).map(|l| l.size), Some(4.0));
        let _ = apply_level_update(&mut asks, 0.21, 0.0, BookSide::Ask);
        assert!(asks.iter().all(|l| l.price != 0.21));
    }

    #[test]
    fn apply_level_update_maintains_cap_after_repeated_updates() {
        let mut bids: Vec<PriceLevel> = Vec::new();
        for i in 0..500 {
            let price = 0.01 + i as f64 * 0.01;
            let _ = apply_level_update(&mut bids, price, 1.0, BookSide::Bid);
        }
        assert_eq!(bids.len(), MAX_BOOK_DEPTH);
        assert!(bids.windows(2).all(|w| w[0].price >= w[1].price));
    }

    #[test]
    fn reconciliation_replaces_on_hash_mismatch() {
        let mut state = PolymarketState::default();
        state.last_snapshot_hash = Some("old".into());

        let snapshot = PolymarketBookResponse {
            market: "m".into(),
            asset_id: "a".into(),
            bids: vec![raw("0.20", "10")],
            asks: vec![raw("0.21", "11")],
            timestamp: "1".into(),
            hash: "new".into(),
        };

        let replaced = reconcile_with_snapshot_data(&mut state, snapshot);
        assert!(replaced);
        assert_eq!(state.last_snapshot_hash.as_deref(), Some("new"));
        assert_eq!(state.book.bids.len(), 1);
    }

    #[test]
    fn reconciliation_skips_when_hash_matches() {
        let mut state = PolymarketState::default();
        state.last_snapshot_hash = Some("same".into());
        state.book = NormalizedBook {
            bids: vec![PriceLevel {
                price: 0.10,
                size: 1.0,
                venue: Venue::Polymarket,
            }],
            asks: vec![],
            last_updated: 1,
        };

        let snapshot = PolymarketBookResponse {
            market: "m".into(),
            asset_id: "a".into(),
            bids: vec![raw("0.20", "10")],
            asks: vec![raw("0.21", "11")],
            timestamp: "2".into(),
            hash: "same".into(),
        };

        let replaced = reconcile_with_snapshot_data(&mut state, snapshot);
        assert!(!replaced);
        assert_eq!(state.book.bids[0].price, 0.10);
    }
}
