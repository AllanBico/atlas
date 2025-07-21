// In crates/execution/src/simulated.rs

use crate::types::{Portfolio, SimulationSettings};
use rust_decimal::Decimal;
use crate::{Error, Executor, Result};
use async_trait::async_trait;
use rust_decimal_macros::dec;
use core_types::{OrderRequest, Execution, Side, Position};
use num_traits::FromPrimitive;
use tokio::sync::broadcast;
use events::WsMessage;
/// A simulated executor that processes orders against an in-memory portfolio.
///
/// This executor is the core of the backtesting and paper trading engine.
/// It simulates market fills, slippage, and fees without sending any
/// real orders to an exchange.
#[derive(Debug)]
pub struct SimulatedExecutor {
    /// The configuration for the simulation.
    settings: SimulationSettings,
    
    /// The portfolio holding the current state (cash, positions).
    /// `Portfolio` is not `Clone`, so the executor must own it.
    portfolio: Portfolio,
    ws_tx: broadcast::Sender<WsMessage>,
}

impl SimulatedExecutor {
    /// Creates a new `SimulatedExecutor`.
    ///
    /// # Arguments
    ///
    /// * `settings`: The simulation settings (fees, slippage).
    /// * `initial_capital`: The starting cash balance for the portfolio.
    pub fn new(
        settings: SimulationSettings,
        initial_capital: Decimal,
        ws_tx: broadcast::Sender<WsMessage>,
    ) -> Self {
        Self {
            settings,
            portfolio: Portfolio::new(initial_capital),
            ws_tx,
        }
    }

    // We need to provide a way to get the portfolio state for our app to use.
    pub fn portfolio(&mut self) -> &mut Portfolio {
        &mut self.portfolio
    }

    fn create_portfolio_update(&self) -> events::WsPortfolioUpdate {
        let open_positions_str_keys = self.portfolio.open_positions
            .iter()
            .map(|(k, v)| (k.0.clone(), v.clone()))
            .collect();
        // TODO: Calculate total value (cash + position values)
        let total_value = self.portfolio.cash;
        events::WsPortfolioUpdate {
            cash: self.portfolio.cash,
            total_value,
            open_positions: open_positions_str_keys,
        }
    }

    /// Processes an entry order (opening a new long or short position).
    fn process_entry(&mut self, order: &OrderRequest, current_price: Decimal, current_time: i64) -> Result<(Execution, Option<Position>)> {
        // --- 1. Calculate Execution Price with Slippage ---
        let slippage_factor = Decimal::from_f64(self.settings.slippage_percent).unwrap();
        let execution_price = if order.side == Side::Long {
            // For a long entry, slippage makes the price worse (higher).
            current_price * (dec!(1) + slippage_factor)
        } else {
            // For a short entry, slippage also makes the price worse (lower).
            current_price * (dec!(1) - slippage_factor)
        };

        // --- 2. Calculate Costs ---
        let position_value = order.quantity * execution_price;
        let fee_rate = Decimal::from_f64(self.settings.taker_fee).unwrap(); // Entries are usually taker orders.
        let fee = position_value * fee_rate;

        // --- 3. Update Portfolio State ---
        // Veto if not enough cash to cover the fee. A real exchange would check margin.
        if self.portfolio.cash < fee {
            return Err(Error::ExecutionFailed {
                reason: "Insufficient cash for fees".to_string(),
            });
        }
        self.portfolio.cash -= fee;

        let new_position = Position {
            symbol: order.symbol.clone(),
            side: order.side,
            quantity: order.quantity,
            entry_price: execution_price,
            leverage: order.leverage,
            sl_price: order.sl_price,
            entry_time: current_time, // <-- Use the passed-in time
        };

        // Add the new position to our portfolio's open positions.
        self.portfolio.open_positions.insert(order.symbol.clone(), new_position);

        // --- 4. Return the Execution Result ---
        let execution = Execution {
            symbol: order.symbol.clone(),
            side: order.side,
            price: execution_price,
            quantity: order.quantity,
            fee,
            source_request: order.clone(),
        };
        let _ = self.ws_tx.send(events::WsMessage::TradeExecuted(execution.clone()));
        // Construct the full portfolio update
        let portfolio_update = self.create_portfolio_update();
        let _ = self.ws_tx.send(events::WsMessage::PortfolioUpdate(portfolio_update));
        Ok((execution, None))
    }

    /// Processes a closing order.
    fn process_close(&mut self, order: &OrderRequest, current_price: Decimal) -> Result<(Execution, Option<Position>)> {
        // --- 1. Find the Position to Close ---
        let open_position = self.portfolio.open_positions.remove(&order.symbol).ok_or_else(
            || Error::ExecutionFailed {
                reason: format!("No open position found for symbol {}", order.symbol.0),
            },
        )?;

        // --- 2. Calculate Execution Price with Slippage ---
        let slippage_factor = Decimal::from_f64(self.settings.slippage_percent).unwrap();
        let execution_price = if open_position.side == Side::Long {
            // To close a long, we sell. Slippage makes the price worse (lower).
            current_price * (dec!(1) - slippage_factor)
        } else {
            // To close a short, we buy. Slippage makes the price worse (higher).
            current_price * (dec!(1) + slippage_factor)
        };

        // --- 3. Calculate P&L and Costs ---
        let pnl = (execution_price - open_position.entry_price)
            * open_position.quantity
            * (if open_position.side == Side::Long { dec!(1) } else { dec!(-1) });
        
        let position_value = open_position.quantity * execution_price;
        let fee_rate = Decimal::from_f64(self.settings.taker_fee).unwrap();
        let fee = position_value * fee_rate;
        let net_pnl = pnl - fee;

        // --- 4. Update Portfolio State ---
        self.portfolio.cash += net_pnl;

        // --- 5. Return the Execution Result ---
        let execution = Execution {
            symbol: order.symbol.clone(),
            side: order.side, // The side of the *closing order*
            price: execution_price,
            quantity: open_position.quantity,
            fee,
            source_request: order.clone(),
        };
        let _ = self.ws_tx.send(WsMessage::TradeExecuted(execution.clone()));
        let _ = self.ws_tx.send(WsMessage::PortfolioUpdate(events::WsPortfolioUpdate {
            cash: Decimal::ZERO,
            total_value: Decimal::ZERO,
            open_positions: std::collections::HashMap::new(),
        }));
        Ok((execution, Some(open_position)))
    }
}

#[async_trait]
impl Executor for SimulatedExecutor {
    fn name(&self) -> &'static str {
        "SimulatedExecutor"
    }

    fn portfolio(&mut self) -> &mut Portfolio {
        &mut self.portfolio
    }

    /// The public method that fulfills the `Executor` trait contract.
    /// It acts as a router to the appropriate internal simulation logic.
    async fn execute(
        &mut self,
        order_request: &OrderRequest,
        current_price: rust_decimal::Decimal,
        current_time: i64,
    ) -> Result<(Execution, Option<Position>)> {
        let is_entry = !self.portfolio.open_positions.contains_key(&order_request.symbol);

        if is_entry {
            self.process_entry(order_request, current_price, current_time)
        } else {
            self.process_close(order_request, current_price)
        }
    }
}