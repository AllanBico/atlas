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