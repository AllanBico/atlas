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

/// Represents a single open position as returned by the account endpoint.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PositionInfo {
    /// The trading pair symbol (e.g., "BTCUSDT").
    pub symbol: String,
    /// The quantity of the position (positive for long, negative for short).
    pub position_amt: Decimal,
    /// The average entry price of the position.
    pub entry_price: Decimal,
    /// The current mark price of the position.
    pub mark_price: Decimal,
    /// The unrealized profit/loss of the position.
    pub unrealized_profit: Decimal,
    /// The leverage used for the position (e.g., "10x").
    #[serde(rename = "leverage")]
    pub leverage: String,
    /// The side of the position ("LONG", "SHORT", or "BOTH").
    pub position_side: String,
}

/// Represents the overall futures account state.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    /// A list of assets in the futures account.
    pub assets: Vec<FuturesAsset>,
    /// A list of open positions in the account.
    pub positions: Vec<PositionInfo>,
    /// The total wallet balance in USDT.
    pub total_wallet_balance: Decimal,
    /// The total unrealized profit and loss in USDT.
    pub total_unrealized_profit: Decimal,
    /// The total margin balance in USDT.
    pub total_margin_balance: Decimal,
    /// The total available balance for new positions in USDT.
    pub total_available_balance: Option<Decimal>,
}

// Keep the old type for backward compatibility
#[deprecated(note = "Use AccountState instead")]
pub type FuturesAccountInfo = AccountState;

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
    #[serde(rename = "c")]
    pub close: Decimal,
    #[serde(rename = "h")]
    pub high: Decimal,
    #[serde(rename = "l")]
    pub low: Decimal,
    #[serde(rename = "v")]
    pub volume: Decimal,
    #[serde(rename = "x")]
    pub is_closed: bool, // Is this kline final?
}


#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewOrderResponse {
    pub symbol: String,
    pub side: String, // "BUY" or "SELL"
    pub r#type: String, // "MARKET", "LIMIT", etc.
    pub avg_price: Decimal, // The actual average fill price
    pub executed_qty: Decimal, // The actual filled quantity
    pub cum_quote: Decimal, // The cumulative quote asset transacted
}
