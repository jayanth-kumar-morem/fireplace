use std::env;
use std::fmt::{Display, Formatter};

pub const SERVER_PORT: u16 = 3001;

pub const POLYMARKET_CLOB_BASE: &str = "https://clob.polymarket.com";
pub const POLYMARKET_WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
pub const POLYMARKET_CONDITION_ID: &str =
    "0x7ad403c3508f8e3912940fd1a913f227591145ca0614074208e0b962d5fcc422";
pub const POLYMARKET_YES_TOKEN_ID: &str =
    "16040015440196279900485035793550429453516625694844857319147506590755961451627";
pub const POLYMARKET_NO_TOKEN_ID: &str =
    "94476829201604408463453426454480212459887267917122244941405244686637914508323";
pub const POLYMARKET_SLUG: &str = "will-jd-vance-win-the-2028-us-presidential-election";
pub const POLYMARKET_PING_INTERVAL_MS: u64 = 10_000;

pub const KALSHI_API_BASE: &str = "https://api.elections.kalshi.com/trade-api/v2";
pub const KALSHI_WS_URL: &str = "wss://api.elections.kalshi.com/trade-api/ws/v2";
pub const KALSHI_MARKET_TICKER: &str = "KXPRESPERSON-28-JVAN";
pub const KALSHI_EVENT_TICKER: &str = "KXPRESPERSON-28";
pub const KALSHI_POLL_INTERVAL_MS: u64 = 2_000;

pub const TICK_SIZE: f64 = 0.01;
pub const MAX_BOOK_DEPTH: usize = 200;
pub const RECONNECT_BASE_DELAY_MS: u64 = 1_000;
pub const RECONNECT_MAX_DELAY_MS: u64 = 30_000;
pub const RECONNECT_MAX_ATTEMPTS: u32 = 10;
pub const STALE_THRESHOLD_MS: u64 = 10_000;
pub const HEARTBEAT_INTERVAL_MS: u64 = 15_000;
pub const RECONCILIATION_INTERVAL_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub port: u16,
    pub kalshi_api_key: Option<String>,
    pub kalshi_private_key_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppConfigError {
    InvalidPort(String),
}

impl Display for AppConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppConfigError::InvalidPort(value) => {
                write!(f, "invalid PORT value '{value}', expected an integer in 1..=65535")
            }
        }
    }
}

impl std::error::Error for AppConfigError {}

impl AppConfig {
    pub fn from_env() -> Result<Self, AppConfigError> {
        let port = match env::var("PORT") {
            Ok(raw) => raw
                .parse::<u16>()
                .map_err(|_| AppConfigError::InvalidPort(raw))?,
            Err(_) => SERVER_PORT,
        };

        let kalshi_api_key = env::var("KALSHI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let kalshi_private_key_path = env::var("KALSHI_PRIVATE_KEY_PATH")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Ok(Self {
            port,
            kalshi_api_key,
            kalshi_private_key_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn from_env_uses_defaults() {
        let _guard = env_lock().lock().expect("lock should not be poisoned");
        env::remove_var("PORT");
        env::remove_var("KALSHI_API_KEY");
        env::remove_var("KALSHI_PRIVATE_KEY_PATH");

        let cfg = AppConfig::from_env().expect("config should parse");
        assert_eq!(cfg.port, SERVER_PORT);
        assert_eq!(cfg.kalshi_api_key, None);
        assert_eq!(cfg.kalshi_private_key_path, None);
    }
}
