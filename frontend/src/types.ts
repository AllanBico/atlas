// In frontend/src/types.ts
// These types MUST match the `WsMessage` and payload structs in the Rust backend.

// --- Core Data Types ---
export interface Position {
    symbol: { "0": string }; // The Symbol newtype
    side: 'Long' | 'Short';
    quantity: string; // Decimals are sent as strings
    entry_price: string;
    leverage: number;
    sl_price: string;
    entry_time: number;
  }
  
  export interface Execution {
    symbol: { "0": string };
    side: 'Long' | 'Short';
    price: string;
    quantity: string;
    fee: string;
    // ... we can add `source_request` if needed later
  }
  
  // --- WebSocket Payloads ---
  export interface WsLogPayload {
    timestamp: string; // ISO 8601 date string
    level: 'INFO' | 'WARN' | 'ERROR' | 'DEBUG';
    message: string;
  }
  
  export interface WsPortfolioUpdatePayload {
    cash: string;
    total_value: string;
    open_positions: Record<string, Position>; // A map of symbol strings to Position objects
  }
  
  // --- Top-Level WebSocket Message ---
  // This uses a discriminated union for excellent type safety in TypeScript.
  export type WsMessage =
    | { type: 'Log'; payload: WsLogPayload }
    | { type: 'PortfolioUpdate'; payload: WsPortfolioUpdatePayload }
    | { type: 'TradeExecuted'; payload: Execution };

// In frontend/src/types.ts

// Represents the strategy parameters (will be a JSON object)
export type StrategyParameters = Record<string, any>;

// Represents the full performance report for a single run
export interface PerformanceReport {
  net_pnl_absolute: string;
  net_pnl_percentage: number;
  max_drawdown_percentage: number;
  sharpe_ratio: number;
  total_trades: number;
  profit_factor: number;
  // We can add all 16 metrics here if we want to display them in the table
}

// Represents a single ranked backtest run within an optimization job
export interface RankedRun {
  score: number;
  report: {
    parameters: StrategyParameters;
    report: PerformanceReport & { run_id: number }; // Add run_id for linking
  };
}

export interface ApiTrade {
  symbol: string;
  side: 'Long' | 'Short';
  entry_time: string;
  exit_time: string;
  entry_price: string;
  exit_price: string;
  pnl: string;
  fees: string;
  signal_confidence: number;
  leverage: number;
}

export interface EquityPoint {
  timestamp: string;
  value: string;
}

// Define the full PerformanceReport type
export interface FullPerformanceReport extends PerformanceReport {
  max_drawdown_absolute: string;
  sortino_ratio: number;
  calmar_ratio: number;
  avg_trade_duration_secs: number;
  expectancy: string;
  larom: number;
  funding_pnl: string;
  drawdown_duration_secs: number;
  win_rate: number;
  // Add other fields as needed
}