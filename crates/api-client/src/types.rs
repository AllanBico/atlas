// In crates/api-client/src/types.rs

use reqwest::Client;
use serde::Deserialize;
use rust_decimal::Decimal;

/// The main client for interacting with the Binance Futures API.
#[derive(Debug, Clone)]
pub struct ApiClient {
    /// The persistent HTTP client.
    pub http_client: Client,
    /// The user's Binance API key.
    pub api_key: String,
    /// The user's Binance secret key.
    pub secret_key: String,
    /// The base URL for the Binance Futures API.
    pub base_url: String,
}

/// Represents a single asset's balance in the futures account.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FuturesAsset {
    /// The asset's symbol (e.g., "USDT").
    pub asset: String,
    /// The wallet balance of the asset.
    pub wallet_balance: Decimal,
    /// The unrealized profit and loss.
    pub unrealized_profit: Decimal,
    /// The margin balance.
    pub margin_balance: Decimal,
    /// The available balance for new positions.
    pub available_balance: Decimal,
}

/// Represents the overall futures account information.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FuturesAccountInfo {
    /// A list of assets in the futures account.
    pub assets: Vec<FuturesAsset>,
    /// The total wallet balance in USDT.
    pub total_wallet_balance: Decimal,
    /// The total unrealized profit and loss in USDT.
    pub total_unrealized_profit: Decimal,
    /// The total margin balance in USDT.
    pub total_margin_balance: Decimal,
    /// The total available balance for new positions in USDT.
    pub total_available_balance: Option<Decimal>,
}

/// Temporary struct to deserialize the kline response from Binance,
/// which is a JSON array of mixed types.
#[derive(Debug, Deserialize)]
pub struct RawKline(
    pub i64,         // 0: Open time
    pub String,      // 1: Open
    pub String,      // 2: High
    pub String,      // 3: Low
    pub String,      // 4: Close
    pub String,      // 5: Volume
    pub i64,         // 6: Close time
    pub String,      // 7: Quote asset volume
    pub i64,         // 8: Number of trades
    pub String,      // 9: Taker buy base asset volume
    pub String,      // 10: Taker buy quote asset volume
    pub String,      // 11: Ignore
);

/// Represents a single kline event from a WebSocket stream.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsKlineEvent {
    #[serde(rename = "e")]
    pub event_type: String, // "kline"
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: WsKline,
}

/// Represents the kline data within a WebSocket event.
#[derive(Debug, Deserialize, Clone)]
pub struct WsKline {
    #[serde(rename = "t")]
    pub open_time: i64,
    #[serde(rename = "T")]
    pub close_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "o")]
    pub open: Decimal,
    #[serde(rename = "l")]
    pub low: Decimal,
    #[serde(rename = "c")]
    pub close: Decimal,
    #[serde(rename = "h")]
    pub high: Decimal,
    #[serde(rename = "v")]
    pub volume: Decimal,
    #[serde(rename = "x")]
    pub is_closed: bool, // Is this kline final?
}

/// The response from a successful `set_leverage` call.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LeverageInfo {
    pub symbol: String,
    pub leverage: u8,
    pub max_notional_value: String,
}

/// The response from a successful `place_order` call.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub order_id: i64,
    pub symbol: String,
    pub side: String, // "BUY" or "SELL"
    #[serde(rename = "type")]
    pub order_type: String, // "MARKET", "LIMIT", etc.
    pub cum_quote: Decimal, // The cumulative quote asset transacted quantity
    pub executed_qty: Decimal,
    pub avg_price: Decimal,
    pub status: String, // "FILLED", "NEW", etc.
}

/// The response from a successful `cancel_all_orders` call.
#[derive(Debug, Deserialize, Clone)]
pub struct CancelAllResponse {
    pub code: String, // "200"
    pub msg: String,  // "The operation of cancel all open order is done."
}