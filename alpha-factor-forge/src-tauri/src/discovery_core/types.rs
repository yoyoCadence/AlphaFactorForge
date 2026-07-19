use serde::{Deserialize, Serialize};

pub const CANDLE_CONTRACT_VERSION: &str = "ohlcv-candle-v1";

/// One immutable OHLCV bar consumed by the pure discovery engine.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
