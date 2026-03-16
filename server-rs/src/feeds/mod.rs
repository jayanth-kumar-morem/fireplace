pub mod reconnect;
pub mod polymarket;
pub mod kalshi;

use crate::types::{BookChange, ConnectionStatus, NormalizedBook, Venue};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum FeedEvent {
    Snapshot {
        venue: Venue,
        book: NormalizedBook,
    },
    BookChange {
        venue: Venue,
        changes: Vec<BookChange>,
    },
    StatusChange {
        venue: Venue,
        status: ConnectionStatus,
    },
}

pub fn round_to_tick(value: f64, tick: f64) -> f64 {
    (value / tick).round() * tick
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn price_key(price: f64, tick: f64) -> i64 {
    (price / tick).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_to_tick() {
        assert!((round_to_tick(0.207, 0.01) - 0.21).abs() < f64::EPSILON);
    }

    #[test]
    fn computes_price_key() {
        assert_eq!(price_key(0.21, 0.01), 21);
    }
}
