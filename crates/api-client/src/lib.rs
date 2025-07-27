// In crates/api-client/src/lib.rs (REPLACE ENTIRE FILE)

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use crate::types::FuturesAccountInfo;
use serde_json::Value;
use app_config::types::BinanceSettings;
use core_types::{Kline, Symbol, Side};
// Create a type alias for the HMAC-SHA256 implementation.
type HmacSha256 = Hmac<Sha256>;

pub mod error;
pub mod types;
pub mod live_connector;

// Re-export public types
pub use error::{Error, Result};
pub use types::*;
pub use live_connector::LiveConnector;

// We will add endpoint functions here later.

impl ApiClient {
    /// Constructs a new ApiClient from BinanceSettings.
    pub fn new(settings: &BinanceSettings) -> Result<Self> {
        let http_client = reqwest::Client::new();
        let api_key = settings.api_key.clone();
        let secret_key = settings.secret_key.clone();
        // The base_url is taken directly from the settings struct
        // that was populated from your .toml file.
        let base_url = settings.rest_base_url.clone(); // <-- IT IS READ HERE
        Ok(ApiClient {
            http_client,
            api_key,
            secret_key,
            base_url, // <-- AND STORED HERE
        })
    }

    /// Generates an HMAC-SHA256 signature for a given query string.
    ///
    /// # Arguments
    ///
    /// * `query_string`: The URL-encoded query string to be signed.
    ///
    /// # Returns
    ///
    /// A hexadecimal string representation of the signature.
    fn sign(&self, query_string: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query_string.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    /// Creates a signed query string including the timestamp and signature.
    ///
    /// # Arguments
    ///
    /// * `params`: A mutable reference to a string to which the parameters will be appended.
    ///
    /// # Returns
    ///
    /// The final signed query string.
    fn create_signed_query(&self, params: &mut String) {
        // Get the current timestamp in milliseconds.
        let timestamp = Utc::now().timestamp_millis();
        
        // Append the timestamp to the parameters.
        if !params.is_empty() {
            params.push('&');
        }
        params.push_str(&format!("timestamp={}", timestamp));
        
        // Sign the parameters.
        let signature = self.sign(params);
        
        // Append the signature to the parameters.
        params.push_str(&format!("&signature={}", signature));
    }

    /// Fetches the futures account balance and asset information.
    ///
    /// This corresponds to the `GET /fapi/v2/account` endpoint.
    pub async fn get_account_balance(&self) -> Result<FuturesAccountInfo> {
        let mut params = String::new();
        self.create_signed_query(&mut params);

        let url = format!("{}/fapi/v2/account?{}", self.base_url, params);

        let response = self
            .http_client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(Error::RequestFailed)?;

        let text = response.text().await.map_err(Error::RequestFailed)?;
        let value: Value = serde_json::from_str(&text).map_err(Error::DeserializationFailed)?;
        
        // Binance returns an error object on failure, so we check for that first.
        if let Some(code) = value.get("code").and_then(Value::as_i64) {
            if code != 0 {
                let msg = value.get("msg").and_then(Value::as_str).unwrap_or("Unknown error").to_string();
                return Err(Error::ApiError { code, msg });
            }
        }
        
        // If no error code, deserialize into our target struct.
        let account_info: FuturesAccountInfo = serde_json::from_value(value).map_err(Error::DeserializationFailed)?;

        Ok(account_info)
    }

    /// Fetches historical kline (candlestick) data.
    ///
    /// This corresponds to the `GET /fapi/v1/klines` endpoint.
    ///
    /// # Arguments
    ///
    /// * `symbol`: The symbol to fetch klines for.
    /// * `interval`: The kline interval (e.g., "1m", "5m", "1h").
    /// * `start_time`: Optional start time in milliseconds.
    /// * `limit`: Optional number of klines to return (max 1500, default 500).
    pub async fn get_historical_klines(
        &self,
        symbol: &Symbol,
        interval: &str,
        start_time: Option<i64>,
        limit: Option<u16>,
    ) -> Result<Vec<Kline>> {
        let mut params = format!("symbol={}&interval={}", symbol.0, interval);

        if let Some(st) = start_time {
            params.push_str(&format!("&startTime={}", st));
        }
        if let Some(l) = limit {
            params.push_str(&format!("&limit={}", l));
        }

        let url = format!("{}/fapi/v1/klines?{}", self.base_url, params);

        let response_body = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(Error::RequestFailed)?
            .text()
            .await
            .map_err(Error::RequestFailed)?;

        // Deserialize the raw response into a vector of RawKline.
        let raw_klines: Vec<RawKline> =
            serde_json::from_str(&response_body).map_err(|e| {
                // If deserialization fails, it might be a Binance error object.
                if let Ok(value) = serde_json::from_str::<Value>(&response_body) {
                    if let Some(code) = value.get("code").and_then(Value::as_i64) {
                        let msg = value.get("msg").and_then(Value::as_str).unwrap_or("").to_string();
                        return Error::ApiError { code, msg };
                    }
                }
                Error::DeserializationFailed(e)
            })?;

        // Convert the RawKlines into our clean, internal Kline type.
        let klines = raw_klines
            .into_iter()
            .map(|raw| Kline {
                open_time: raw.0,
                open: raw.1.parse().unwrap_or_default(),
                high: raw.2.parse().unwrap_or_default(),
                low: raw.3.parse().unwrap_or_default(),
                close: raw.4.parse().unwrap_or_default(),
                volume: raw.5.parse().unwrap_or_default(),
                close_time: raw.6,
            })
            .collect();

        Ok(klines)
    }
    pub async fn set_leverage(&self, symbol: &Symbol, leverage: u8) -> Result<()> {
        let mut params = format!("symbol={}&leverage={}", symbol.0, leverage);
        self.create_signed_query(&mut params);

        let url = format!("{}/fapi/v1/leverage", self.base_url);

        let response = self.http_client
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .body(params)
            .send()
            .await
            .map_err(Error::RequestFailed)?;

        let text = response.text().await.map_err(Error::RequestFailed)?;
        let value: Value = serde_json::from_str(&text).map_err(Error::DeserializationFailed)?;

        if let Some(code) = value.get("code") {
            // A successful leverage change doesn't have a code, but errors do.
            let msg = value.get("msg").and_then(Value::as_str).unwrap_or("Unknown error").to_string();
            return Err(Error::ApiError { code: code.as_i64().unwrap_or(-1), msg });
        }
        
        Ok(())
    }

    /// Places a new market order.
    /// Corresponds to `POST /fapi/v1/order`.
    pub async fn place_market_order(
        &self,
        symbol: &Symbol,
        side: &core_types::Side,
        quantity: rust_decimal::Decimal,
    ) -> Result<NewOrderResponse> {
        let side_str = match side {
            core_types::Side::Long => "BUY",
            core_types::Side::Short => "SELL",
        };

    // For Hedge Mode, we must specify the position side.
    // For One-way Mode, this parameter is ignored but doesn't hurt.
    // We will assume "BOTH" for now, as this works for One-Way and is a common setting.
    // Let's check the docs to be sure.
    // --- Correction based on docs ---
    // For Hedge mode, it must be LONG or SHORT.
    // Let's assume the user wants to trade one way for now and will set Hedge mode later.
    // The most robust solution is to ALWAYS send the positionSide.
    
    let position_side_str = match side {
        core_types::Side::Long => "LONG",
        core_types::Side::Short => "SHORT",
    };

    let quantity_str = format!("{:.3}", quantity);

    let mut params = format!(
        "symbol={}&side={}&type=MARKET&quantity={}&positionSide={}",
        symbol.0, side_str, quantity_str, position_side_str
    );
        self.create_signed_query(&mut params);

        let url = format!("{}/fapi/v1/order", self.base_url);

        let response = self.http_client
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .body(params)
            .send()
            .await
            .map_err(Error::RequestFailed)?;
            
        let text = response.text().await.map_err(Error::RequestFailed)?;
        let value: Value = serde_json::from_str(&text).map_err(Error::DeserializationFailed)?;
        
        if let Some(code) = value.get("code") {
            let msg = value.get("msg").and_then(Value::as_str).unwrap_or("Unknown error").to_string();
            return Err(Error::ApiError { code: code.as_i64().unwrap_or(-1), msg });
        }
        
        let order_response: NewOrderResponse = serde_json::from_value(value).map_err(Error::DeserializationFailed)?;

        Ok(order_response)
    }
}

// Free function to allow api_client::new usage
pub fn new(settings: &BinanceSettings) -> Result<ApiClient> {
    ApiClient::new(settings)
}

