use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Venue {
    Polymarket,
    Kalshi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    Connected,
    Reconnecting,
    Disconnected,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64,
    pub size: f64,
    pub venue: Venue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedBook {
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub last_updated: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueContribution {
    pub venue: Venue,
    pub size: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedLevel {
    pub price: f64,
    pub total_size: f64,
    pub venues: Vec<VenueContribution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_size: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedBook {
    pub bids: Vec<AggregatedLevel>,
    pub asks: Vec<AggregatedLevel>,
    pub spread: f64,
    pub midpoint: f64,
    pub best_bid: f64,
    pub best_ask: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketInfo {
    pub title: String,
    pub outcomes: [String; 2],
    pub polymarket_slug: String,
    pub kalshi_ticker: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polymarket_volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kalshi_volume: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteInput {
    #[serde(rename = "dollarAmount")]
    pub dollar_amount: f64,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub price: f64,
    pub shares: f64,
    pub cost: f64,
    pub venue: Venue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueFillSummary {
    pub shares: f64,
    pub cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VenueFillSplit {
    pub polymarket: VenueFillSummary,
    pub kalshi: VenueFillSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResult {
    pub total_shares: f64,
    pub avg_price: f64,
    pub fills: Vec<Fill>,
    pub venue_split: VenueFillSplit,
    pub slippage: f64,
    pub unfilled: f64,
    pub implied_probability: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotBooks {
    pub polymarket: Option<NormalizedBook>,
    pub kalshi: Option<NormalizedBook>,
    pub aggregated: Option<AggregatedBook>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueConnections {
    pub polymarket: ConnectionStatus,
    pub kalshi: ConnectionStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotData {
    pub market: MarketInfo,
    pub books: SnapshotBooks,
    pub connections: VenueConnections,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookChange {
    pub side: BookSide,
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BookSide {
    Bid,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Snapshot {
        data: SnapshotData,
    },
    BookUpdate {
        venue: Venue,
        changes: Vec<BookChange>,
        timestamp: u64,
    },
    ConnectionStatus {
        venue: Venue,
        status: ConnectionStatus,
        timestamp: u64,
    },
    Heartbeat {
        timestamp: u64,
        connections: VenueConnections,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolymarketRawLevel {
    pub price: String,
    pub size: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolymarketBookResponse {
    pub market: String,
    pub asset_id: String,
    pub bids: Vec<PolymarketRawLevel>,
    pub asks: Vec<PolymarketRawLevel>,
    pub timestamp: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolymarketPriceChangeEntry {
    pub asset_id: String,
    pub price: String,
    pub size: String,
    pub side: PolymarketSide,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_bid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_ask: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolymarketSide {
    BUY,
    SELL,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolymarketPriceChangeEvent {
    pub event_type: String,
    pub market: String,
    pub price_changes: Vec<PolymarketPriceChangeEntry>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolymarketBookEvent {
    pub event_type: String,
    pub asset_id: String,
    pub market: String,
    pub bids: Vec<PolymarketRawLevel>,
    pub asks: Vec<PolymarketRawLevel>,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolymarketWsEvent {
    Book(PolymarketBookEvent),
    PriceChange(PolymarketPriceChangeEvent),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KalshiOrderbookResponse {
    pub orderbook_fp: KalshiOrderbookFp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KalshiOrderbookFp {
    pub yes_dollars: Vec<(String, String)>,
    pub no_dollars: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KalshiOrderbookSnapshotMsg {
    pub market_ticker: String,
    pub market_id: String,
    pub yes_dollars_fp: Vec<(String, String)>,
    pub no_dollars_fp: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KalshiOrderbookSnapshot {
    #[serde(rename = "type")]
    pub message_type: String,
    pub sid: i64,
    pub seq: i64,
    pub msg: KalshiOrderbookSnapshotMsg,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KalshiOrderbookDeltaMsg {
    pub market_ticker: String,
    pub market_id: String,
    pub price_dollars: String,
    pub delta_fp: String,
    pub side: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subaccount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KalshiOrderbookDelta {
    #[serde(rename = "type")]
    pub message_type: String,
    pub sid: i64,
    pub seq: i64,
    pub msg: KalshiOrderbookDeltaMsg,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KalshiWsMessage {
    Snapshot(KalshiOrderbookSnapshot),
    Delta(KalshiOrderbookDelta),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    Aggregated,
    Polymarket,
    Kalshi,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    fn sample_market() -> MarketInfo {
        MarketInfo {
            title: "Will JD Vance win the 2028 US Presidential Election?".to_string(),
            outcomes: ["Yes".to_string(), "No".to_string()],
            polymarket_slug: "will-jd-vance-win-the-2028-us-presidential-election".to_string(),
            kalshi_ticker: "KXPRESPERSON-28-JVAN".to_string(),
            polymarket_volume: None,
            kalshi_volume: None,
        }
    }

    #[test]
    fn snapshot_serialization_matches_contract() {
        let message = ServerMessage::Snapshot {
            data: SnapshotData {
                market: sample_market(),
                books: SnapshotBooks {
                    polymarket: None,
                    kalshi: None,
                    aggregated: None,
                },
                connections: VenueConnections {
                    polymarket: ConnectionStatus::Connected,
                    kalshi: ConnectionStatus::Reconnecting,
                },
            },
        };

        let value = serde_json::to_value(message).expect("message should serialize");
        assert_eq!(
            value,
            json!({
                "type": "snapshot",
                "data": {
                    "market": {
                        "title": "Will JD Vance win the 2028 US Presidential Election?",
                        "outcomes": ["Yes", "No"],
                        "polymarketSlug": "will-jd-vance-win-the-2028-us-presidential-election",
                        "kalshiTicker": "KXPRESPERSON-28-JVAN"
                    },
                    "books": {
                        "polymarket": null,
                        "kalshi": null,
                        "aggregated": null
                    },
                    "connections": {
                        "polymarket": "connected",
                        "kalshi": "reconnecting"
                    }
                }
            })
        );
    }

    #[test]
    fn book_update_serialization_matches_contract() {
        let message = ServerMessage::BookUpdate {
            venue: Venue::Polymarket,
            changes: vec![BookChange {
                side: BookSide::Bid,
                price: 0.21,
                size: 150.0,
            }],
            timestamp: 1_742_200_001_000,
        };

        let value = serde_json::to_value(message).expect("message should serialize");
        assert_eq!(
            value,
            json!({
                "type": "book_update",
                "venue": "polymarket",
                "changes": [
                    { "side": "bid", "price": 0.21, "size": 150.0 }
                ],
                "timestamp": 1_742_200_001_000_u64
            })
        );
    }

    #[test]
    fn connection_status_and_heartbeat_serialization_matches_contract() {
        let status = ServerMessage::ConnectionStatus {
            venue: Venue::Kalshi,
            status: ConnectionStatus::Stale,
            timestamp: 1_742_200_005_000,
        };

        let status_value = serde_json::to_value(status).expect("status should serialize");
        assert_eq!(
            status_value,
            json!({
                "type": "connection_status",
                "venue": "kalshi",
                "status": "stale",
                "timestamp": 1_742_200_005_000_u64
            })
        );

        let heartbeat = ServerMessage::Heartbeat {
            timestamp: 1_742_200_015_000,
            connections: VenueConnections {
                polymarket: ConnectionStatus::Connected,
                kalshi: ConnectionStatus::Connected,
            },
        };

        let hb_value = serde_json::to_value(heartbeat).expect("heartbeat should serialize");
        assert_eq!(
            hb_value,
            json!({
                "type": "heartbeat",
                "timestamp": 1_742_200_015_000_u64,
                "connections": {
                    "polymarket": "connected",
                    "kalshi": "connected"
                }
            })
        );
    }

    #[test]
    fn raw_types_deserialize_shapes() {
        let poly: Value = json!({
            "market": "m",
            "asset_id": "a",
            "bids": [{"price":"0.20","size":"10"}],
            "asks": [{"price":"0.21","size":"20"}],
            "timestamp": "1742200000",
            "hash": "abc"
        });
        let _: PolymarketBookResponse =
            serde_json::from_value(poly).expect("polymarket response should parse");

        let kalshi: Value = json!({
            "orderbook_fp": {
                "yes_dollars": [["0.20","10.00"]],
                "no_dollars": [["0.80","5.00"]]
            }
        });
        let _: KalshiOrderbookResponse =
            serde_json::from_value(kalshi).expect("kalshi response should parse");
    }

    #[test]
    fn node_fixture_contract_parity_messages() {
        fn load_fixture(path: &str) -> Value {
            serde_json::from_str(path).expect("fixture JSON should parse")
        }

        let snapshot_fixture = load_fixture(include_str!("../tests/fixtures/node-snapshot.json"));
        let book_update_fixture = load_fixture(include_str!("../tests/fixtures/node-book-update.json"));
        let connection_status_fixture =
            load_fixture(include_str!("../tests/fixtures/node-connection-status.json"));
        let heartbeat_fixture = load_fixture(include_str!("../tests/fixtures/node-heartbeat.json"));

        let snapshot_actual = serde_json::to_value(ServerMessage::Snapshot {
            data: SnapshotData {
                market: sample_market(),
                books: SnapshotBooks {
                    polymarket: None,
                    kalshi: None,
                    aggregated: None,
                },
                connections: VenueConnections {
                    polymarket: ConnectionStatus::Connected,
                    kalshi: ConnectionStatus::Reconnecting,
                },
            },
        })
        .expect("snapshot should serialize");
        assert_eq!(snapshot_actual, snapshot_fixture);

        let book_update_actual = serde_json::to_value(ServerMessage::BookUpdate {
            venue: Venue::Polymarket,
            changes: vec![BookChange {
                side: BookSide::Bid,
                price: 0.21,
                size: 150.0,
            }],
            timestamp: 1_742_200_001_000,
        })
        .expect("book update should serialize");
        assert_eq!(book_update_actual, book_update_fixture);

        let connection_status_actual = serde_json::to_value(ServerMessage::ConnectionStatus {
            venue: Venue::Kalshi,
            status: ConnectionStatus::Stale,
            timestamp: 1_742_200_005_000,
        })
        .expect("connection status should serialize");
        assert_eq!(connection_status_actual, connection_status_fixture);

        let heartbeat_actual = serde_json::to_value(ServerMessage::Heartbeat {
            timestamp: 1_742_200_015_000,
            connections: VenueConnections {
                polymarket: ConnectionStatus::Connected,
                kalshi: ConnectionStatus::Connected,
            },
        })
        .expect("heartbeat should serialize");
        assert_eq!(heartbeat_actual, heartbeat_fixture);

        // Keep this map to make it explicit we validate all expected message fixture files.
        let covered: BTreeMap<&str, bool> = BTreeMap::from([
            ("snapshot", true),
            ("book_update", true),
            ("connection_status", true),
            ("heartbeat", true),
        ]);
        assert_eq!(covered.len(), 4);
    }
}
