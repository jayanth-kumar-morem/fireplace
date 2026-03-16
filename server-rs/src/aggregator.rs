use crate::config::{
    KALSHI_MARKET_TICKER, POLYMARKET_SLUG, RECONCILIATION_INTERVAL_MS, STALE_THRESHOLD_MS,
    TICK_SIZE,
};
use crate::feeds::polymarket::{build_snapshot_book, fetch_snapshot};
use crate::feeds::{now_ms, price_key, FeedEvent};
use crate::types::{
    AggregatedBook, AggregatedLevel, BookChange, BookSide, ConnectionStatus, MarketInfo,
    NormalizedBook, ServerMessage, SnapshotBooks, SnapshotData, Venue, VenueConnections,
    VenueContribution,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, watch, RwLock};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct AppState {
    pub market: MarketInfo,
    pub polymarket_book: Option<NormalizedBook>,
    pub kalshi_book: Option<NormalizedBook>,
    pub aggregated_book: Option<AggregatedBook>,
    pub connections: VenueConnections,
    pub last_updated: HashMap<Venue, u64>,
}

impl Default for AppState {
    fn default() -> Self {
        let mut last_updated = HashMap::new();
        last_updated.insert(Venue::Polymarket, 0);
        last_updated.insert(Venue::Kalshi, 0);
        Self {
            market: default_market_info(),
            polymarket_book: None,
            kalshi_book: None,
            aggregated_book: None,
            connections: VenueConnections {
                polymarket: ConnectionStatus::Disconnected,
                kalshi: ConnectionStatus::Disconnected,
            },
            last_updated,
        }
    }
}

pub type SharedAppState = Arc<RwLock<AppState>>;

pub fn default_market_info() -> MarketInfo {
    MarketInfo {
        title: "Will JD Vance win the 2028 US Presidential Election?".to_string(),
        outcomes: ["Yes".to_string(), "No".to_string()],
        polymarket_slug: POLYMARKET_SLUG.to_string(),
        kalshi_ticker: KALSHI_MARKET_TICKER.to_string(),
        polymarket_volume: None,
        kalshi_volume: None,
    }
}

pub fn aggregate(books: &[Option<&NormalizedBook>]) -> AggregatedBook {
    let mut bid_map: HashMap<i64, AggregatedLevel> = HashMap::new();
    let mut ask_map: HashMap<i64, AggregatedLevel> = HashMap::new();

    for book in books.iter().flatten() {
        for level in &book.bids {
            add_level(&mut bid_map, level.price, level.size, level.venue);
        }
        for level in &book.asks {
            add_level(&mut ask_map, level.price, level.size, level.venue);
        }
    }

    let mut bids = bid_map.into_values().collect::<Vec<_>>();
    let mut asks = ask_map.into_values().collect::<Vec<_>>();
    bids.sort_by(|a, b| b.price.total_cmp(&a.price));
    asks.sort_by(|a, b| a.price.total_cmp(&b.price));

    let best_bid = bids.first().map(|l| l.price).unwrap_or(0.0);
    let best_ask = asks.first().map(|l| l.price).unwrap_or(1.0);
    let spread = ((best_ask - best_bid).max(0.0) / TICK_SIZE).round() * TICK_SIZE;
    let midpoint = ((best_bid + best_ask) / 2.0 / TICK_SIZE).round() * TICK_SIZE;

    AggregatedBook {
        bids,
        asks,
        spread,
        midpoint,
        best_bid,
        best_ask,
    }
}

fn add_level(map: &mut HashMap<i64, AggregatedLevel>, price: f64, size: f64, venue: Venue) {
    let key = price_key(price, TICK_SIZE);
    let normalized_price = key as f64 * TICK_SIZE;
    let entry = map.entry(key).or_insert(AggregatedLevel {
        price: normalized_price,
        total_size: 0.0,
        venues: vec![],
        cumulative_size: None,
    });
    entry.total_size += size;
    if let Some(v) = entry.venues.iter_mut().find(|v| v.venue == venue) {
        v.size += size;
    } else {
        entry.venues.push(VenueContribution { venue, size });
    }
}

fn apply_book_changes(book: &mut NormalizedBook, changes: &[BookChange], venue: Venue) {
    for change in changes {
        let levels = match change.side {
            BookSide::Bid => &mut book.bids,
            BookSide::Ask => &mut book.asks,
        };
        if let Some(existing) = levels.iter_mut().find(|l| l.price == change.price) {
            if change.size <= 0.0 {
                levels.retain(|l| l.price != change.price);
            } else {
                existing.size = change.size;
            }
        } else if change.size > 0.0 {
            levels.push(crate::types::PriceLevel {
                price: change.price,
                size: change.size,
                venue,
            });
        }
    }
    book.bids.sort_by(|a, b| b.price.total_cmp(&a.price));
    book.asks.sort_by(|a, b| a.price.total_cmp(&b.price));
    book.last_updated = now_ms();
}

fn serialize_message(message: &ServerMessage) -> Option<Arc<str>> {
    serde_json::to_string(message)
        .ok()
        .map(|s| Arc::<str>::from(s.into_boxed_str()))
}

pub fn snapshot_message_from_state(state: &AppState) -> ServerMessage {
    ServerMessage::Snapshot {
        data: SnapshotData {
            market: state.market.clone(),
            books: SnapshotBooks {
                polymarket: state.polymarket_book.clone(),
                kalshi: state.kalshi_book.clone(),
                aggregated: state.aggregated_book.clone(),
            },
            connections: state.connections.clone(),
        },
    }
}

fn venue_status_mut(connections: &mut VenueConnections, venue: Venue) -> &mut ConnectionStatus {
    match venue {
        Venue::Polymarket => &mut connections.polymarket,
        Venue::Kalshi => &mut connections.kalshi,
    }
}

fn venue_status(connections: &VenueConnections, venue: Venue) -> ConnectionStatus {
    match venue {
        Venue::Polymarket => connections.polymarket,
        Venue::Kalshi => connections.kalshi,
    }
}

fn collect_stale_messages(state: &mut AppState, now: u64, stale_threshold_ms: u64) -> Vec<ServerMessage> {
    let mut stale_messages = vec![];
    for venue in [Venue::Polymarket, Venue::Kalshi] {
        let status = venue_status(&state.connections, venue);
        let last = *state.last_updated.get(&venue).unwrap_or(&0);
        if status == ConnectionStatus::Connected
            && last > 0
            && now.saturating_sub(last) > stale_threshold_ms
        {
            *venue_status_mut(&mut state.connections, venue) = ConnectionStatus::Stale;
            stale_messages.push(ServerMessage::ConnectionStatus {
                venue,
                status: ConnectionStatus::Stale,
                timestamp: now,
            });
        }
    }
    stale_messages
}

async fn recompute_and_publish(state: &SharedAppState, broadcast_tx: &broadcast::Sender<Arc<str>>) {
    let mut guard = state.write().await;
    let aggregated = aggregate(&[
        guard.polymarket_book.as_ref(),
        guard.kalshi_book.as_ref(),
    ]);
    guard.aggregated_book = Some(aggregated);
    let snapshot = snapshot_message_from_state(&guard);
    drop(guard);

    if let Some(serialized) = serialize_message(&snapshot) {
        let _ = broadcast_tx.send(serialized);
    }
}

pub async fn run_aggregator(
    feed_rx: mpsc::Receiver<FeedEvent>,
    state: SharedAppState,
    broadcast_tx: broadcast::Sender<Arc<str>>,
    http_client: reqwest::Client,
    shutdown_rx: watch::Receiver<bool>,
) {
    run_aggregator_internal(
        feed_rx,
        state,
        broadcast_tx,
        http_client,
        shutdown_rx,
        Duration::from_secs(2),
        Duration::from_millis(RECONCILIATION_INTERVAL_MS),
        STALE_THRESHOLD_MS,
        None,
    )
    .await;
}

async fn run_aggregator_internal(
    mut feed_rx: mpsc::Receiver<FeedEvent>,
    state: SharedAppState,
    broadcast_tx: broadcast::Sender<Arc<str>>,
    http_client: reqwest::Client,
    mut shutdown_rx: watch::Receiver<bool>,
    stale_interval: Duration,
    reconciliation_interval: Duration,
    stale_threshold_ms: u64,
    mut reconciliation_rx: Option<mpsc::Receiver<crate::types::PolymarketBookResponse>>,
) {
    let mut stale_tick = tokio::time::interval(stale_interval);
    let mut reconcile_tick = tokio::time::interval(reconciliation_interval);
    let mut last_reconciled_hash: Option<String> = None;

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            maybe_event = feed_rx.recv() => {
                let Some(event) = maybe_event else { break; };
                match event {
                    FeedEvent::Snapshot { venue, book } => {
                        {
                            let mut guard = state.write().await;
                            match venue {
                                Venue::Polymarket => guard.polymarket_book = Some(book),
                                Venue::Kalshi => guard.kalshi_book = Some(book),
                            }
                            guard.last_updated.insert(venue, now_ms());
                        }
                        recompute_and_publish(&state, &broadcast_tx).await;
                    }
                    FeedEvent::BookChange { venue, changes } => {
                        {
                            let mut guard = state.write().await;
                            let target = match venue {
                                Venue::Polymarket => guard.polymarket_book.get_or_insert(NormalizedBook{
                                    bids: vec![],
                                    asks: vec![],
                                    last_updated: now_ms(),
                                }),
                                Venue::Kalshi => guard.kalshi_book.get_or_insert(NormalizedBook{
                                    bids: vec![],
                                    asks: vec![],
                                    last_updated: now_ms(),
                                }),
                            };
                            apply_book_changes(target, &changes, venue);
                            guard.last_updated.insert(venue, now_ms());
                        }
                        recompute_and_publish(&state, &broadcast_tx).await;
                    }
                    FeedEvent::StatusChange { venue, status } => {
                        {
                            let mut guard = state.write().await;
                            *venue_status_mut(&mut guard.connections, venue) = status;
                            if status == ConnectionStatus::Connected {
                                guard.last_updated.insert(venue, now_ms());
                            }
                        }
                        let message = ServerMessage::ConnectionStatus {
                            venue,
                            status,
                            timestamp: now_ms(),
                        };
                        if let Some(serialized) = serialize_message(&message) {
                            let _ = broadcast_tx.send(serialized);
                        }
                    }
                }
            }
            _ = stale_tick.tick() => {
                let now = now_ms();
                let stale_messages = {
                    let mut guard = state.write().await;
                    collect_stale_messages(&mut guard, now, stale_threshold_ms)
                };
                for msg in stale_messages {
                    if let Some(serialized) = serialize_message(&msg) {
                        let _ = broadcast_tx.send(serialized);
                    }
                }
            }
            _ = reconcile_tick.tick() => {
                let maybe_snapshot = if let Some(rx) = reconciliation_rx.as_mut() {
                    rx.try_recv().ok()
                } else {
                    fetch_snapshot(&http_client).await.ok()
                };

                if let Some(snapshot) = maybe_snapshot {
                    let incoming_hash = snapshot.hash.clone();
                    if last_reconciled_hash.as_deref() != Some(incoming_hash.as_str()) {
                        last_reconciled_hash = Some(incoming_hash);
                        {
                            let mut guard = state.write().await;
                            guard.polymarket_book = Some(build_snapshot_book(&snapshot));
                            guard.last_updated.insert(Venue::Polymarket, now_ms());
                        }
                        warn!("polymarket reconciliation mismatch -> replacing local book");
                        recompute_and_publish(&state, &broadcast_tx).await;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PriceLevel, Venue};

    fn mk_book(bids: Vec<(f64, f64, Venue)>, asks: Vec<(f64, f64, Venue)>) -> NormalizedBook {
        NormalizedBook {
            bids: bids
                .into_iter()
                .map(|(price, size, venue)| PriceLevel { price, size, venue })
                .collect(),
            asks: asks
                .into_iter()
                .map(|(price, size, venue)| PriceLevel { price, size, venue })
                .collect(),
            last_updated: now_ms(),
        }
    }

    #[test]
    fn aggregate_two_venue_merge_and_consolidation() {
        let poly = mk_book(
            vec![(0.20, 100.0, Venue::Polymarket)],
            vec![(0.22, 80.0, Venue::Polymarket)],
        );
        let kalshi = mk_book(
            vec![(0.20, 50.0, Venue::Kalshi), (0.19, 40.0, Venue::Kalshi)],
            vec![(0.23, 30.0, Venue::Kalshi)],
        );
        let out = aggregate(&[Some(&poly), Some(&kalshi)]);
        assert_eq!(out.bids[0].price, 0.20);
        assert_eq!(out.bids[0].total_size, 150.0);
        assert_eq!(out.bids[0].venues.len(), 2);
        assert_eq!(out.best_bid, 0.20);
        assert_eq!(out.best_ask, 0.22);
    }

    #[test]
    fn aggregate_single_venue_and_empty_books() {
        let poly = mk_book(vec![(0.18, 10.0, Venue::Polymarket)], vec![]);
        let out_one = aggregate(&[Some(&poly), None]);
        assert_eq!(out_one.bids.len(), 1);
        let out_empty = aggregate(&[None, None]);
        assert!(out_empty.bids.is_empty());
        assert!(out_empty.asks.is_empty());
        assert_eq!(out_empty.best_bid, 0.0);
        assert_eq!(out_empty.best_ask, 1.0);
    }

    #[test]
    fn stale_detector_emits_status_change_deterministically() {
        let mut state = AppState::default();
        state.connections.polymarket = ConnectionStatus::Connected;
        state
            .last_updated
            .insert(Venue::Polymarket, 1_000);

        let stale = collect_stale_messages(&mut state, 1_500, 200);
        assert_eq!(stale.len(), 1);
        match &stale[0] {
            ServerMessage::ConnectionStatus { venue, status, .. } => {
                assert_eq!(*venue, Venue::Polymarket);
                assert_eq!(*status, ConnectionStatus::Stale);
            }
            _ => panic!("expected connection status message"),
        }
    }

    #[tokio::test]
    async fn status_change_recovery_from_stale_to_connected_is_broadcast() {
        let (feed_tx, feed_rx) = mpsc::channel(16);
        let state = Arc::new(RwLock::new(AppState::default()));
        let (broadcast_tx, mut broadcast_rx) = broadcast::channel::<Arc<str>>(64);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let client = reqwest::Client::new();

        tokio::spawn(run_aggregator_internal(
            feed_rx,
            state.clone(),
            broadcast_tx.clone(),
            client,
            shutdown_rx,
            Duration::from_secs(3600),
            Duration::from_secs(3600),
            STALE_THRESHOLD_MS,
            None,
        ));

        let _ = feed_tx
            .send(FeedEvent::StatusChange {
                venue: Venue::Polymarket,
                status: ConnectionStatus::Stale,
            })
            .await;
        let _ = feed_tx
            .send(FeedEvent::StatusChange {
                venue: Venue::Polymarket,
                status: ConnectionStatus::Connected,
            })
            .await;
        let _ = feed_tx
            .send(FeedEvent::StatusChange {
                venue: Venue::Kalshi,
                status: ConnectionStatus::Connected,
            })
            .await;

        let mut saw_poly_recovered = false;
        let mut saw_kalshi_connected = false;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        while tokio::time::Instant::now() < deadline {
            if let Ok(msg) =
                tokio::time::timeout(Duration::from_millis(40), broadcast_rx.recv()).await
            {
                if let Ok(payload) = msg {
                    if payload.contains("\"type\":\"connection_status\"")
                        && payload.contains("\"venue\":\"polymarket\"")
                        && payload.contains("\"status\":\"connected\"")
                    {
                        saw_poly_recovered = true;
                    }
                    if payload.contains("\"type\":\"connection_status\"")
                        && payload.contains("\"venue\":\"kalshi\"")
                        && payload.contains("\"status\":\"connected\"")
                    {
                        saw_kalshi_connected = true;
                    }
                }
            }
            if saw_poly_recovered && saw_kalshi_connected {
                break;
            }
        }

        assert!(saw_poly_recovered);
        assert!(saw_kalshi_connected);
    }

    #[tokio::test]
    async fn reconciliation_mismatch_updates_and_broadcasts() {
        let (feed_tx, feed_rx) = mpsc::channel(16);
        let state = Arc::new(RwLock::new(AppState::default()));
        let (broadcast_tx, mut broadcast_rx) = broadcast::channel::<Arc<str>>(64);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let client = reqwest::Client::new();
        let (recon_tx, recon_rx) = mpsc::channel(4);

        tokio::spawn(run_aggregator_internal(
            feed_rx,
            state.clone(),
            broadcast_tx.clone(),
            client,
            shutdown_rx,
            Duration::from_secs(3600),
            Duration::from_millis(20),
            STALE_THRESHOLD_MS,
            Some(recon_rx),
        ));

        let _ = feed_tx
            .send(FeedEvent::Snapshot {
                venue: Venue::Polymarket,
                book: mk_book(vec![(0.10, 1.0, Venue::Polymarket)], vec![]),
            })
            .await;

        let _ = recon_tx
            .send(crate::types::PolymarketBookResponse {
                market: "m".into(),
                asset_id: "a".into(),
                bids: vec![crate::types::PolymarketRawLevel {
                    price: "0.31".into(),
                    size: "10".into(),
                }],
                asks: vec![crate::types::PolymarketRawLevel {
                    price: "0.32".into(),
                    size: "12".into(),
                }],
                timestamp: "1".into(),
                hash: "h-new".into(),
            })
            .await;

        let mut saw_snapshot = false;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(350);
        while tokio::time::Instant::now() < deadline {
            if let Ok(msg) = tokio::time::timeout(Duration::from_millis(40), broadcast_rx.recv()).await {
                if let Ok(payload) = msg {
                    if payload.contains("\"type\":\"snapshot\"")
                        && payload.contains("\"price\":0.31")
                    {
                        saw_snapshot = true;
                        break;
                    }
                }
            }
        }
        assert!(saw_snapshot);
    }
}
