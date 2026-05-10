import React from 'react';
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ReferenceLine, ResponsiveContainer } from 'recharts';
import { TrendingDown, TrendingUp, AlertTriangle } from 'lucide-react';
import { PriceUpdate, CircuitBreakerUpdate } from '../types';
import { formatCurrency, cn } from '../lib/utils';

interface FibonacciViewProps {
  price?: PriceUpdate;
  cb?: CircuitBreakerUpdate;
  history: { price: number; timestamp: string }[];
}

export const FibonacciView: React.FC<FibonacciViewProps> = ({ price, cb, history }) => {
  const currentPrice = price?.price || 0;
  const cbPrice = cb?.cb_price || currentPrice * 0.95; // Default to 5% drop if not provided
  
  // Calculate Fibonacci levels based on recent high/low in history
  const prices = history.map(h => h.price);
  const high = Math.max(...prices, currentPrice);
  const low = Math.min(...prices, currentPrice);
  const range = high - low;

  const levels = [
    { ratio: 0.0, price: high, label: 'High' },
    { ratio: 0.236, price: high - range * 0.236, label: '0.236' },
    { ratio: 0.382, price: high - range * 0.382, label: '0.382' },
    { ratio: 0.5, price: high - range * 0.5, label: '0.500' },
    { ratio: 0.618, price: high - range * 0.618, label: '0.618' },
    { ratio: 0.786, price: high - range * 0.786, label: '0.786 (CB)' },
    { ratio: 1.0, price: low, label: 'Low' },
  ];

  return (
    <div className="glass-morphism h-[400px] flex flex-col p-6 gap-6 relative overflow-hidden border-2 border-border-military">
      <div className="flex items-center justify-between z-10 border-b border-border-military/50 pb-4">
        <div className="flex items-center gap-4">
          <div className="w-10 h-10 border border-military-accent flex items-center justify-center bg-military-accent text-white">
            <TrendingDown className="w-6 h-6" />
          </div>
          <div>
            <h3 className="font-display font-black text-text-primary tracking-tight uppercase">Strategic Vector Analysis</h3>
            <p className="stat-label">
              SIGNAL STRENGTH: {cb?.ratio || '0.786'} COMPLIANT
            </p>
          </div>
        </div>
        
        {cb?.is_active && (
          <div className="flex items-center gap-2 px-3 py-1 bg-red-600 border-2 border-red-900 text-white animate-pulse">
            <AlertTriangle className="w-4 h-4" />
            <span className="text-[10px] font-black uppercase tracking-widest">
              PROTOCOL BREACH: OPS HALTED
            </span>
          </div>
        )}
      </div>

      <div className="flex-1 min-h-0 w-full bg-military-bg/20 border border-border-military/30">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={history}>
            <defs>
              <linearGradient id="colorPrice" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="var(--color-military-accent)" stopOpacity={0.1}/>
                <stop offset="95%" stopColor="var(--color-military-accent)" stopOpacity={0}/>
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="rgba(0,0,0,0.05)" />
            <XAxis 
              dataKey="timestamp" 
              hide 
            />
            <YAxis 
              domain={['auto', 'auto']} 
              orientation="right"
              tick={{ fill: 'var(--color-text-secondary)', fontSize: 10, fontFamily: 'monospace', fontWeight: 'bold' }}
              axisLine={false}
              tickLine={false}
              tickFormatter={(val) => `$${val.toFixed(3)}`}
            />
            <Tooltip 
              contentStyle={{ 
                background: 'var(--color-military-deep)', 
                border: '1px solid var(--color-border-military)', 
                color: 'white',
                borderRadius: '0px',
                fontFamily: 'monospace'
              }}
              labelStyle={{ display: 'none' }}
              itemStyle={{ color: '#FFF', fontSize: '12px', fontWeight: 'bold' }}
              formatter={(value: number) => [`$${value.toFixed(4)}`, 'VECTOR']}
            />
            
            {levels.map((level, i) => (
              <ReferenceLine 
                key={i}
                y={level.price} 
                stroke={level.ratio === 0.786 ? "var(--color-red-600)" : "var(--color-border-military)"} 
                strokeDasharray={level.ratio === 0.786 ? "4 2" : "2 2"}
                strokeWidth={level.ratio === 0.618 ? 2 : 1}
                label={{ 
                  value: level.label, 
                  position: 'insideLeft', 
                  fill: level.ratio === 0.786 ? '#dc2626' : 'var(--color-text-secondary)',
                  fontSize: 10,
                  fontWeight: 'black',
                  fontFamily: 'monospace'
                }}
              />
            ))}

            <Area 
              type="monotone" 
              dataKey="price" 
              stroke="var(--color-military-accent)" 
              strokeWidth={3}
              fillOpacity={1} 
              fill="url(#colorPrice)" 
              animationDuration={500}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>

      <div className="flex gap-6 pt-2 border-t border-border-military/50 mt-auto">
        <div className="flex-1 bg-military-bg/50 p-3 border border-border-military/20">
          <div className="stat-label mb-1">
            <span className="inline-block w-1.5 h-1.5 bg-military-green rounded-full mr-2" />
            MARKET COORDINATE
          </div>
          <div className="text-2xl font-black font-mono text-text-primary leading-none">${currentPrice.toFixed(4)}</div>
        </div>
        <div className="flex-1 bg-military-bg/50 p-3 border border-border-military/20">
          <div className="stat-label mb-1">
            <span className="inline-block w-1.5 h-1.5 bg-red-600 rounded-full mr-2" />
            REVERSION FLOOR
          </div>
          <div className="text-2xl font-black font-mono text-red-600 leading-none">${cbPrice.toFixed(4)}</div>
        </div>
      </div>
    </div>
  );
};
