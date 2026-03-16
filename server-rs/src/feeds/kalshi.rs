use crate::config::{
    KALSHI_API_BASE, KALSHI_MARKET_TICKER, KALSHI_POLL_INTERVAL_MS, KALSHI_WS_URL, MAX_BOOK_DEPTH,
    RECONNECT_BASE_DELAY_MS, RECONNECT_MAX_ATTEMPTS, RECONNECT_MAX_DELAY_MS, TICK_SIZE,
};
use crate::feeds::reconnect::ReconnectState;
use crate::feeds::{now_ms, round_to_tick, FeedEvent};
use crate::types::{
    BookChange, BookSide, ConnectionStatus, KalshiOrderbookDelta, KalshiOrderbookResponse,
    KalshiOrderbookSnapshot, NormalizedBook, Outcome, PriceLevel, Venue,
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use rsa::pkcs8::DecodePrivateKey;
use rsa::pss::BlindedSigningKey;
use rsa::rand_core::OsRng;
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use rsa::RsaPrivateKey;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderValue, Request};
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

#[derive(Debug)]
pub enum KalshiAuthError {
    InvalidKeyPath(String),
    ReadError(std::io::Error),
    ParseError(String),
}

impl Display for KalshiAuthError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            KalshiAuthError::InvalidKeyPath(path) => write!(f, "invalid private key path: {path}"),
            KalshiAuthError::ReadError(err) => write!(f, "failed to read private key: {err}"),
            KalshiAuthError::ParseError(err) => write!(f, "failed to parse private key: {err}"),
        }
    }
}

impl std::error::Error for KalshiAuthError {}

#[derive(Debug, Clone)]
pub struct KalshiState {
    pub status: ConnectionStatus,
    pub book: NormalizedBook,
    pub last_message_timestamp: u64,
    pub expected_seq: i64,
    pub subscription_id: u64,
    pub raw_yes_dollars: HashMap<String, f64>,
    pub raw_no_dollars: HashMap<String, f64>,
}

impl Default for KalshiState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            book: NormalizedBook {
                bids: vec![],
                asks: vec![],
                last_updated: 0,
            },
            last_message_timestamp: 0,
            expected_seq: 0,
            subscription_id: 0,
            raw_yes_dollars: HashMap::new(),
            raw_no_dollars: HashMap::new(),
        }
    }
}

pub async fn fetch_snapshot(
    client: &reqwest::Client,
) -> Result<KalshiOrderbookResponse, reqwest::Error> {
    let url = format!("{KALSHI_API_BASE}/markets/{KALSHI_MARKET_TICKER}/orderbook?depth=50");
    client.get(url).send().await?.error_for_status()?.json().await
}

pub fn normalize_kalshi_book(
    yes_dollars: &[(String, String)],
    no_dollars: &[(String, String)],
) -> (Vec<PriceLevel>, Vec<PriceLevel>) {
    let mut bids = Vec::new();
    for (price, qty) in yes_dollars {
        if let (Ok(price), Ok(size)) = (price.parse::<f64>(), qty.parse::<f64>()) {
            bids.push(PriceLevel {
                price: round_to_tick(price, TICK_SIZE),
                size,
                venue: Venue::Kalshi,
            });
        }
    }

    let mut asks = Vec::new();
    for (price, qty) in no_dollars {
        if let (Ok(price), Ok(size)) = (price.parse::<f64>(), qty.parse::<f64>()) {
            asks.push(PriceLevel {
                price: round_to_tick(1.0 - price, TICK_SIZE),
                size,
                venue: Venue::Kalshi,
            });
        }
    }

    bids.sort_by(|a, b| b.price.total_cmp(&a.price));
    asks.sort_by(|a, b| a.price.total_cmp(&b.price));
    bids.truncate(MAX_BOOK_DEPTH);
    asks.truncate(MAX_BOOK_DEPTH);
    (bids, asks)
}

pub fn load_private_key(path: &str) -> Result<RsaPrivateKey, KalshiAuthError> {
    if path.trim().is_empty() {
        return Err(KalshiAuthError::InvalidKeyPath(path.to_string()));
    }
    if !Path::new(path).exists() {
        return Err(KalshiAuthError::InvalidKeyPath(path.to_string()));
    }
    let pem = std::fs::read_to_string(path).map_err(KalshiAuthError::ReadError)?;
    RsaPrivateKey::from_pkcs8_pem(&pem).map_err(|err| KalshiAuthError::ParseError(err.to_string()))
}

pub fn sign_request(
    private_key: &RsaPrivateKey,
    timestamp_ms: &str,
    method: &str,
    path: &str,
) -> String {
    let msg = format!("{timestamp_ms}{method}{path}");
    let signing_key = BlindedSigningKey::<sha2::Sha256>::new(private_key.clone());
    let signature = signing_key.sign_with_rng(&mut OsRng, msg.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
}

fn apply_to_book_side(levels: &mut Vec<PriceLevel>, price: f64, size: f64, side: BookSide) {
    if let Some(existing) = levels.iter_mut().find(|l| l.price == price) {
        if size <= 0.0 {
            levels.retain(|l| l.price != price);
        } else {
            existing.size = size;
        }
    } else if size > 0.0 {
        levels.push(PriceLevel {
            price,
            size,
            venue: Venue::Kalshi,
        });
    }

    match side {
        BookSide::Bid => levels.sort_by(|a, b| b.price.total_cmp(&a.price)),
        BookSide::Ask => levels.sort_by(|a, b| a.price.total_cmp(&b.price)),
    }
    levels.truncate(MAX_BOOK_DEPTH);
}

fn delta_to_book_change(state: &mut KalshiState, side: Outcome, price_dollars: &str, delta: f64) -> Option<BookChange> {
    let raw_map = match side {
        Outcome::Yes => &mut state.raw_yes_dollars,
        Outcome::No => &mut state.raw_no_dollars,
    };
    let existing = *raw_map.get(price_dollars).unwrap_or(&0.0);
    let new_qty = existing + delta;
    if new_qty <= 0.0 {
        raw_map.remove(price_dollars);
    } else {
        raw_map.insert(price_dollars.to_string(), new_qty);
    }

    let raw_price = price_dollars.parse::<f64>().ok()?;
    match side {
        Outcome::Yes => {
            let normalized_price = round_to_tick(raw_price, TICK_SIZE);
            apply_to_book_side(&mut state.book.bids, normalized_price, new_qty, BookSide::Bid);
            Some(BookChange {
                side: BookSide::Bid,
                price: normalized_price,
                size: new_qty,
            })
        }
        Outcome::No => {
            let normalized_price = round_to_tick(1.0 - raw_price, TICK_SIZE);
            apply_to_book_side(&mut state.book.asks, normalized_price, new_qty, BookSide::Ask);
            Some(BookChange {
                side: BookSide::Ask,
                price: normalized_price,
                size: new_qty,
            })
        }
    }
}

fn apply_snapshot_to_state(state: &mut KalshiState, snapshot: KalshiOrderbookResponse) {
    state.raw_yes_dollars.clear();
    state.raw_no_dollars.clear();

    for (p, q) in &snapshot.orderbook_fp.yes_dollars {
        if let Ok(qty) = q.parse::<f64>() {
            state.raw_yes_dollars.insert(p.clone(), qty);
        }
    }
    for (p, q) in &snapshot.orderbook_fp.no_dollars {
        if let Ok(qty) = q.parse::<f64>() {
            state.raw_no_dollars.insert(p.clone(), qty);
        }
    }

    let (bids, asks) =
        normalize_kalshi_book(&snapshot.orderbook_fp.yes_dollars, &snapshot.orderbook_fp.no_dollars);
    state.book = NormalizedBook {
        bids,
        asks,
        last_updated: now_ms(),
    };
    state.last_message_timestamp = now_ms();
}

async fn send_status(
    tx: &mpsc::Sender<FeedEvent>,
    status: ConnectionStatus,
) -> Result<(), mpsc::error::SendError<FeedEvent>> {
    tx.send(FeedEvent::StatusChange {
        venue: Venue::Kalshi,
        status,
    })
    .await
}

async fn run_polling_mode(
    state: &mut KalshiState,
    tx: &mpsc::Sender<FeedEvent>,
    client: &reqwest::Client,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let first = fetch_snapshot(client).await?;
    apply_snapshot_to_state(state, first);
    tx.send(FeedEvent::Snapshot {
        venue: Venue::Kalshi,
        book: state.book.clone(),
    })
    .await?;
    send_status(tx, ConnectionStatus::Connected).await?;
    state.status = ConnectionStatus::Connected;

    let mut poll = tokio::time::interval(Duration::from_millis(KALSHI_POLL_INTERVAL_MS));
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    return Ok(());
                }
            }
            _ = poll.tick() => {
                let snapshot = fetch_snapshot(client).await?;
                apply_snapshot_to_state(state, snapshot);
                tx.send(FeedEvent::Snapshot {
                    venue: Venue::Kalshi,
                    book: state.book.clone(),
                }).await?;
            }
        }
    }
}

fn ws_request(
    api_key: &str,
    signature: &str,
    timestamp_ms: &str,
) -> Result<Request<()>, Box<dyn std::error::Error + Send + Sync>> {
    let mut req = KALSHI_WS_URL.into_client_request()?;
    req.headers_mut()
        .insert("KALSHI-ACCESS-KEY", HeaderValue::from_str(api_key)?);
    req.headers_mut().insert(
        "KALSHI-ACCESS-TIMESTAMP",
        HeaderValue::from_str(timestamp_ms)?,
    );
    req.headers_mut().insert(
        "KALSHI-ACCESS-SIGNATURE",
        HeaderValue::from_str(signature)?,
    );
    Ok(req)
}

async fn send_subscribe(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    subscription_id: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    write
        .send(Message::Text(
            serde_json::json!({
                "id": subscription_id,
                "cmd": "subscribe",
                "params": {
                    "channels": ["orderbook_delta"],
                    "market_tickers": [KALSHI_MARKET_TICKER]
                }
            })
            .to_string()
            .into(),
        ))
        .await?;
    Ok(())
}

async fn run_ws_mode(
    state: &mut KalshiState,
    tx: &mpsc::Sender<FeedEvent>,
    client: &reqwest::Client,
    api_key: &str,
    private_key: &RsaPrivateKey,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let first = fetch_snapshot(client).await?;
    apply_snapshot_to_state(state, first);
    tx.send(FeedEvent::Snapshot {
        venue: Venue::Kalshi,
        book: state.book.clone(),
    })
    .await?;

    let timestamp_ms = now_ms().to_string();
    let signature = sign_request(private_key, &timestamp_ms, "GET", "/trade-api/ws/v2");
    let req = ws_request(api_key, &signature, &timestamp_ms)?;
    let (stream, _) = connect_async(req).await?;
    let (mut write, mut read) = stream.split();

    state.subscription_id = state.subscription_id.saturating_add(1);
    send_subscribe(&mut write, state.subscription_id).await?;

    send_status(tx, ConnectionStatus::Connected).await?;
    state.status = ConnectionStatus::Connected;
    info!("kalshi websocket connected");

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    let _ = write.close().await;
                    return Ok(());
                }
            }
            maybe_msg = read.next() => {
                let Some(frame) = maybe_msg else {
                    return Err("kalshi websocket stream ended".into());
                };
                let frame = frame?;
                match frame {
                    Message::Text(text) => {
                        state.last_message_timestamp = now_ms();
                        handle_kalshi_text_message(state, tx, &mut write, text.as_ref()).await?;
                    }
                    Message::Binary(bytes) => {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            state.last_message_timestamp = now_ms();
                            handle_kalshi_text_message(state, tx, &mut write, &text).await?;
                        }
                    }
                    Message::Close(_) => return Err("kalshi websocket closed".into()),
                    _ => {}
                }
            }
        }
    }
}

async fn handle_kalshi_text_message(
    state: &mut KalshiState,
    tx: &mpsc::Sender<FeedEvent>,
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let msg_type = value.get("type").and_then(Value::as_str).unwrap_or_default();

    match msg_type {
        "orderbook_snapshot" => {
            let snapshot: KalshiOrderbookSnapshot = serde_json::from_value(value)?;
            state.expected_seq = snapshot.seq;
            let normalized = KalshiOrderbookResponse {
                orderbook_fp: crate::types::KalshiOrderbookFp {
                    yes_dollars: snapshot.msg.yes_dollars_fp,
                    no_dollars: snapshot.msg.no_dollars_fp,
                },
            };
            apply_snapshot_to_state(state, normalized);
            tx.send(FeedEvent::Snapshot {
                venue: Venue::Kalshi,
                book: state.book.clone(),
            })
            .await?;
        }
        "orderbook_delta" => {
            let delta: KalshiOrderbookDelta = serde_json::from_value(value)?;
            if delta.seq != state.expected_seq + 1 {
                warn!(
                    "kalshi sequence gap: expected {}, got {}. resubscribing",
                    state.expected_seq + 1,
                    delta.seq
                );
                state.subscription_id = state.subscription_id.saturating_add(1);
                send_subscribe(write, state.subscription_id).await?;
                return Ok(());
            }
            state.expected_seq = delta.seq;
            let delta_qty = match delta.msg.delta_fp.parse::<f64>() {
                Ok(v) => v,
                Err(_) => return Ok(()),
            };
            if let Some(change) = delta_to_book_change(state, delta.msg.side, &delta.msg.price_dollars, delta_qty) {
                state.book.last_updated = now_ms();
                tx.send(FeedEvent::BookChange {
                    venue: Venue::Kalshi,
                    changes: vec![change],
                })
                .await?;
            }
        }
        _ => {}
    }

    Ok(())
}

pub async fn run_kalshi_feed(
    feed_tx: mpsc::Sender<FeedEvent>,
    http_client: reqwest::Client,
    kalshi_api_key: Option<String>,
    kalshi_private_key_path: Option<String>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut state = KalshiState::default();
    let mut reconnect = ReconnectState::new(
        RECONNECT_MAX_ATTEMPTS,
        Duration::from_millis(RECONNECT_BASE_DELAY_MS),
        Duration::from_millis(RECONNECT_MAX_DELAY_MS),
    );

    let private_key = kalshi_private_key_path
        .as_deref()
        .and_then(|path| match load_private_key(path) {
            Ok(k) => Some(k),
            Err(err) => {
                warn!("kalshi private key load failed; falling back to polling mode: {err}");
                None
            }
        });
    let use_ws = kalshi_api_key.is_some() && private_key.is_some();

    let _ = send_status(&feed_tx, ConnectionStatus::Reconnecting).await;
    state.status = ConnectionStatus::Reconnecting;

    loop {
        if *shutdown_rx.borrow() {
            let _ = send_status(&feed_tx, ConnectionStatus::Disconnected).await;
            return;
        }

        let run_result = if use_ws {
            run_ws_mode(
                &mut state,
                &feed_tx,
                &http_client,
                kalshi_api_key.as_deref().unwrap_or_default(),
                private_key.as_ref().expect("ws mode requires private key"),
                &mut shutdown_rx,
            )
            .await
        } else {
            run_polling_mode(&mut state, &feed_tx, &http_client, &mut shutdown_rx).await
        };

        match run_result {
            Ok(_) => {
                if *shutdown_rx.borrow() {
                    let _ = send_status(&feed_tx, ConnectionStatus::Disconnected).await;
                    return;
                }
            }
            Err(err) => warn!("kalshi connection cycle ended: {err}"),
        }

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
            error!("kalshi max reconnect attempts reached; entering slow retry mode");
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
    use rsa::pkcs8::EncodePrivateKey;
    use rsa::traits::PublicKeyParts;

    fn mk_pair(price: &str, qty: &str) -> (String, String) {
        (price.to_string(), qty.to_string())
    }

    #[test]
    fn normalize_kalshi_inversion_sorting_and_cap() {
        let yes = vec![
            mk_pair("0.25", "100.00"),
            mk_pair("0.30", "50.00"),
            mk_pair("0.10", "10.00"),
        ];
        let no = vec![mk_pair("0.80", "25.00"), mk_pair("0.75", "40.00")];
        let (bids, asks) = normalize_kalshi_book(&yes, &no);

        assert_eq!(bids[0].price, 0.30);
        assert_eq!(bids[1].price, 0.25);
        assert_eq!(asks[0].price, 0.20); // 1 - 0.80
        assert_eq!(asks[1].price, 0.25); // 1 - 0.75
        assert!(bids.len() <= MAX_BOOK_DEPTH);
        assert!(asks.len() <= MAX_BOOK_DEPTH);
    }

    #[test]
    fn sign_request_produces_verifiable_signature() {
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("key should generate");
        let sig = sign_request(&private_key, "1700000000000", "GET", "/trade-api/ws/v2");
        assert!(!sig.is_empty());
    }

    #[test]
    fn load_private_key_returns_explicit_error_for_invalid_path() {
        let result = load_private_key("/definitely/not/here.pem");
        assert!(matches!(result, Err(KalshiAuthError::InvalidKeyPath(_))));
    }

    #[test]
    fn load_private_key_reads_pkcs8_pem() {
        let mut rng = OsRng;
        let key = RsaPrivateKey::new(&mut rng, 2048).expect("key should generate");
        let pem = key
            .to_pkcs8_pem(Default::default())
            .expect("pem encoding should work")
            .to_string();
        let mut path = std::env::temp_dir();
        path.push(format!("kalshi-test-key-{}.pem", now_ms()));
        std::fs::write(&path, pem).expect("file write should work");

        let loaded = load_private_key(path.to_str().expect("valid path")).expect("key should load");
        assert_eq!(loaded.n(), key.n());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn delta_conversion_handles_yes_and_no_and_sequence_gap_behavior() {
        let mut state = KalshiState::default();
        state.expected_seq = 10;
        state.raw_yes_dollars.insert("0.20".into(), 5.0);
        state.raw_no_dollars.insert("0.80".into(), 4.0);

        let yes_change = delta_to_book_change(&mut state, Outcome::Yes, "0.20", 3.0)
            .expect("yes delta should produce change");
        assert_eq!(yes_change.side, BookSide::Bid);
        assert_eq!(yes_change.price, 0.20);
        assert_eq!(yes_change.size, 8.0);

        let no_change =
            delta_to_book_change(&mut state, Outcome::No, "0.80", -1.0).expect("no delta change");
        assert_eq!(no_change.side, BookSide::Ask);
        assert_eq!(no_change.price, 0.20);
        assert_eq!(no_change.size, 3.0);

        // Gap behavior contract: expected sequence for next message is current + 1.
        assert_eq!(state.expected_seq + 1, 11);
    }
}
