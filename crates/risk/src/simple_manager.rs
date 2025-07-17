// In crates/risk/src/simple_manager.rs

use crate::types::SimpleRiskSettings;
use crate::{Error, Result, RiskManager}; // Import our own trait and errors
use core_types::{OrderRequest, Position, Side, Signal, Kline};
use rust_decimal::Decimal;
use rust_decimal_macros::dec; // For creating decimals from literals
use num_traits::{FromPrimitive, ToPrimitive};

/// A simple risk manager that uses a fixed fractional position sizing model.
///
/// This manager implements two basic rules:
/// 1. Vetoes trades if signal confidence is below a configured threshold.
/// 2. Calculates position size based on a fixed percentage of portfolio value
///    and a pre-defined stop-loss distance.
#[derive(Debug)]
pub struct SimpleRiskManager {
    /// The configuration for this risk manager instance.
    settings: SimpleRiskSettings,
}

impl SimpleRiskManager {
    /// Creates a new `SimpleRiskManager` instance from its settings.
    pub fn new(settings: SimpleRiskSettings) -> Self {
        Self { settings }
    }
}

impl RiskManager for SimpleRiskManager {
    fn name(&self) -> &'static str {
        "SimpleRiskManager"
    }

    fn evaluate(
        &self,
        signal: &Signal,
        portfolio_value: Decimal,
        current_kline: &Kline,
        open_position: Option<&Position>,
    ) -> Result<Option<OrderRequest>> {
        // --- Veto & Early Exit Logic ---

        // Rule: If signal is Hold, do nothing.
        if matches!(signal, Signal::Hold) {
            return Ok(None);
        }

        // Rule: If signal is Close, construct a closing order if a position exists.
        if let Signal::Close = signal {
            return match open_position {
                Some(pos) => {
                    // Create a simple market order to close the position.
                    // The quantity will be the full size of the open position.
                    // The SL price is irrelevant for a closing order.
                    Ok(Some(OrderRequest {
                        symbol: pos.symbol.clone(),
                        side: if pos.side == Side::Long { Side::Short } else { Side::Long },
                        quantity: pos.quantity,
                        
                        // USE the position's leverage
                        leverage: pos.leverage,

                        sl_price: dec!(0), // Placeholder
                        originating_signal: *signal,
                    }))
                }
                None => Ok(None), // No position to close.
            };
        }

        // --- Entry Signal Logic ---

        // We are now dealing with a GoLong or GoShort signal.
        let (signal_side, confidence) = match signal {
            Signal::GoLong { confidence } => (Side::Long, *confidence),
            Signal::GoShort { confidence } => (Side::Short, *confidence),
            _ => unreachable!(), // We already handled Hold and Close.
        };

        // Rule: Veto if a position is already open. (No pyramiding in V1).
        if open_position.is_some() {
            return Err(Error::Vetoed {
                reason: "A position is already open for this symbol.".to_string(),
            });
        }

        // Rule: Veto if confidence is below the configured threshold.
        if confidence < self.settings.minimum_confidence_threshold {
            return Err(Error::Vetoed {
                reason: format!(
                    "Signal confidence ({:.2}) is below threshold ({:.2})",
                    confidence, self.settings.minimum_confidence_threshold
                ),
            });
        }

        // --- Position Sizing Logic ---

        // This assumes the `klines` data would be passed in to get the current price.
        // For now, we will use a placeholder price.
        // In a real implementation, this would come from the live data feed.
        let entry_price = current_kline.close;

        // Convert portfolio_value to Decimal
        // let portfolio_value = Decimal::from_f64(portfolio_value).unwrap(); // This line is removed as portfolio_value is now Decimal

        // Calculate stop-loss price
        let sl_price = if signal_side == Side::Long {
            entry_price * (dec!(1) - Decimal::from_f64(self.settings.stop_loss_percent).unwrap())
        } else {
            entry_price * (dec!(1) + Decimal::from_f64(self.settings.stop_loss_percent).unwrap())
        };

        // Calculate position size
        let risk_per_trade = Decimal::from_f64(self.settings.risk_per_trade_percent).unwrap();
        let amount_to_risk = portfolio_value * risk_per_trade;

        // Scale risk by confidence
        let scaled_amount_to_risk = amount_to_risk * Decimal::from_f64(confidence).unwrap();

        // Position size in quote asset (e.g., USDT)
        let position_size_quote = scaled_amount_to_risk / Decimal::from_f64(self.settings.stop_loss_percent).unwrap();
        
        // Convert to base asset quantity
        let quantity_base = position_size_quote / entry_price;

        // --- Construct the Order Request ---
        
        let order_request = OrderRequest {
            // TODO: Pass the symbol context into evaluate; Kline does not contain symbol.
            symbol: core_types::Symbol("BTCUSDT".to_string()), // Placeholder symbol
            side: signal_side,
            quantity: quantity_base,
            
            // USE the configured value
            leverage: self.settings.leverage,

            sl_price,
            originating_signal: *signal,
        };

        Ok(Some(order_request))
    }
}