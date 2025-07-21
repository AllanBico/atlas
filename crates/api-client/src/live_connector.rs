// In crates/api-client/src/live_connector.rs

use crate::Result;
use crate::types::{ WsKlineEvent};
use async_stream::stream;
use core_types::{Kline, Symbol};
use futures::Stream;
use futures_util::StreamExt;
use tokio_tungstenite::connect_async;

/// A connector for receiving live data streams from Binance.
#[derive(Clone)]
pub struct LiveConnector;

impl LiveConnector {
    pub fn new() -> Self {
        Self
    }

    /// Subscribes to a kline stream and returns an asynchronous stream of `Kline` data.
    ///
    /// The returned stream will only yield klines when they are marked as "closed".
    pub fn subscribe_to_klines(
        &self,
        symbol: &Symbol,
        interval: &str,
        base_url: &str,
    ) -> impl Stream<Item = Result<Kline>> {
        let stream_name = format!("{}@kline_{}", symbol.0.to_lowercase(), interval);
        let url = format!("{}/{}", base_url, stream_name);

        stream! {
            loop {
                tracing::info!(url = %url, "Connecting to WebSocket stream...");
                let (ws_stream, _) = match connect_async(&url).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(error = %e, "WebSocket connection failed. Retrying in 5s...");
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                };
                tracing::info!("WebSocket connection successful.");

                let mut read = ws_stream.fuse();

                while let Some(message) = read.next().await {
                    match message {
                        Ok(msg) => {
                            // tracing::info!(?msg, "Raw WebSocket message received");
                            if let Ok(text) = msg.to_text() {
                                if let Ok(event) = serde_json::from_str::<WsKlineEvent>(text) {
                                    // Only yield the kline if it's the final update for that bar.
                                    if event.kline.is_closed {
                                        tracing::info!("Closed kline received: {:?}", event.kline);
                                        yield Ok(Kline {
                                            open_time: event.kline.open_time,
                                            open: event.kline.open,
                                            high: event.kline.high,
                                            low: event.kline.low,
                                            close: event.kline.close,
                                            volume: event.kline.volume,
                                            close_time: event.kline.close_time,
                                        });
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Error reading from WebSocket. Reconnecting...");
                            // Break the inner loop to trigger a reconnection.
                            break;
                        }
                    }
                }
            }
        }
    }
}