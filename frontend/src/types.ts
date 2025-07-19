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