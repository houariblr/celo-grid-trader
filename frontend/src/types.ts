// ============================================================
//  types.ts — Shared TypeScript types for Grid Keeper V2 UI
// ============================================================

export interface PriceUpdate {
  price: number;
  sources: number;
  sources_used?: string[];
  deviation_warning: boolean;
  timestamp: string;
}

export interface CircuitBreakerUpdate {
  reason: string;
  is_active: boolean;
  cb_price: number;
  ratio: number;
}

export interface GridLevel {
  index: number;
  price: number;
  filled: boolean;
  is_buy: boolean;
}

export interface GridStatus {
  grid_id: number;
  lower_price: number;
  upper_price: number;
  active: boolean;
  level_count: number;
  levels: GridLevel[];
  quote_balance?: number;
  base_balance?: number;
  amount_per_grid?: number;
  yield_enabled?: boolean;
  slippage_bps?: number;
  created_at?: number;
}

export interface TransactionUpdate {
  id: string;
  hash: string;
  type: 'BUY' | 'SELL';
  price: number;
  status: 'PENDING' | 'SUCCESS' | 'FAILED';
  timestamp: string;
  gas_used?: number;
  grid_id?: number;
  level_index?: number;
}

export interface ChainClientHealth {
  rpc_status: string;
  rpc_current_url?: string;
  total_requests: number;
  success_rate: number;
  avg_response_time_ms: number;
  consecutive_failures: number;
  simulation_enabled?: boolean;
  min_profit_threshold_usd?: number;
}

export interface HealthUpdate {
  chain_client: ChainClientHealth;
  last_execution_seconds_ago: number;
  ohlc_fresh: boolean;
  timestamp: string;
}

export type WsMessageType =
  | 'PRICE_UPDATE'
  | 'CIRCUIT_BREAKER'
  | 'GRID_STATUS'
  | 'TRANSACTION'
  | 'HEALTH'
  | 'ERROR'
  | 'SHUTDOWN';

export interface WsMessage {
  type: WsMessageType;
  data:
    | PriceUpdate
    | CircuitBreakerUpdate
    | GridStatus
    | TransactionUpdate
    | HealthUpdate
    | { message: string };
  timestamp?: string;
}

export interface PricePoint {
  price: number;
  timestamp: string;
}

export interface CreateGridFormData {
  lower:        bigint;
  upper:        bigint;
  levels:       bigint;
  amount:       bigint;
  yieldEnabled: boolean;
  slippageBps:  bigint;
}
