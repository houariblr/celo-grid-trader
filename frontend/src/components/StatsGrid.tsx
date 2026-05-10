import React from 'react';
import { Shield, ShieldAlert, Activity, Signal, Zap } from 'lucide-react';
import { StatCard } from './StatCard';
import type { PriceUpdate, CircuitBreakerUpdate, HealthUpdate } from '../types';
import { formatCurrency, cn } from '../lib/utils';

interface StatsGridProps {
  price?:  PriceUpdate;
  cb?:     CircuitBreakerUpdate;
  health?: HealthUpdate;
}

export const StatsGrid: React.FC<StatsGridProps> = ({ price, cb, health }) => {
  // rpc_current_url is optional — guard against undefined
  const rpcLabel = health?.chain_client.rpc_current_url
    ? health.chain_client.rpc_current_url.replace(/^https?:\/\//, '').slice(0, 30)
    : 'Waiting for RPC…';

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 w-full">
      <StatCard
        title="CELO / USDT"
        value={price ? formatCurrency(price.price) : '$0.0000'}
        subtitle={`Median from ${price?.sources ?? 0} sources — Binance · Gate.io · MEXC · CoinGecko`}
        icon={<Activity className="w-5 h-5" />}
        trend={price?.deviation_warning ? 'down' : 'neutral'}
      />

      <StatCard
        title="Circuit Breaker"
        value={cb?.is_active ? 'TRIPPED' : 'ARMED'}
        subtitle={
          cb?.is_active
            ? `Suspended — ${cb.reason}`
            : 'Monitoring 0.786 Fibonacci level'
        }
        className={cn(
          cb?.is_active
            ? 'border-red-500/50 bg-red-500/5'
            : 'border-military-green/30'
        )}
        icon={
          cb?.is_active
            ? <ShieldAlert className="w-5 h-5 text-red-500" />
            : <Shield className="w-5 h-5 text-military-green" />
        }
      />

      <StatCard
        title="RPC Success Rate"
        value={health ? `${health.chain_client.success_rate.toFixed(1)}%` : '0.0%'}
        subtitle={rpcLabel}
        icon={<Signal className="w-5 h-5" />}
      />

      <StatCard
        title="Last Execution"
        value={health ? `${health.last_execution_seconds_ago}s ago` : '—'}
        subtitle={
          health
            ? health.ohlc_fresh
              ? 'OHLC Data: FRESH ✓'
              : '⚠ OHLC Data: STALE'
            : 'Awaiting keeper heartbeat'
        }
        icon={<Zap className="w-5 h-5" />}
        trend={
          health && !health.ohlc_fresh ? 'down' : 'neutral'
        }
      />
    </div>
  );
};
