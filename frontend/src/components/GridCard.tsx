import React from 'react';
import { motion } from 'motion/react';
import { Box, Layers, PlayCircle, StopCircle } from 'lucide-react';
import { GridStatus } from '../types';
import { cn, formatCurrency } from '../lib/utils';

interface GridCardProps {
  grid: GridStatus;
}

export const GridCard: React.FC<GridCardProps> = ({ grid }) => {
  const filledCount = grid.levels?.filter(l => l.filled).length ?? 0;
  const progress = (filledCount / (grid.level_count || 1)) * 100;

  return (
    <motion.div 
      layout
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      className="glass-morphism p-6 flex flex-col gap-6 overflow-hidden border-2 border-border-military"
    >
      <div className="flex justify-between items-center pb-4 border-b border-border-military/50">
        <div className="flex items-center gap-4">
          <div className="w-10 h-10 border border-military-deep flex items-center justify-center bg-military-deep text-white">
            <Box className="w-6 h-6" />
          </div>
          <div>
            <h4 className="font-display font-black text-military-deep tracking-tight uppercase">Asset Path #{grid.grid_id}</h4>
            <p className="text-[9px] text-text-secondary font-mono font-bold tracking-widest uppercase">VECTOR-ID: {grid.grid_id}</p>
          </div>
        </div>
        <div className={cn(
          "flex items-center gap-2 px-3 py-1 font-mono text-[9px] font-black uppercase tracking-widest border",
          grid.active 
            ? "border-military-green text-military-green bg-military-green-light/20" 
            : "border-red-600 text-red-600 bg-red-50"
        )}>
          {grid.active ? <PlayCircle className="w-3 h-3" /> : <StopCircle className="w-3 h-3" />}
          {grid.active ? 'ACTIVE' : 'HALTED'}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-6 bg-military-alt p-4 border border-border-military/40">
        <div>
          <p className="stat-label mb-1">MIN_THRESHOLD</p>
          <p className="text-sm font-black font-mono text-military-deep">{formatCurrency(grid.lower_price)}</p>
        </div>
        <div className="text-right border-l border-border-military/40 pl-4">
          <p className="stat-label mb-1">MAX_THRESHOLD</p>
          <p className="text-sm font-black font-mono text-military-deep">{formatCurrency(grid.upper_price)}</p>
        </div>
      </div>

      <div className="space-y-3">
        <div className="flex justify-between font-mono text-[9px] font-black uppercase tracking-[0.2em] mb-1">
          <span className="text-text-secondary">SIGNAL DENSITY</span>
          <span className="text-military-green bg-military-green/10 px-2 py-0.5 border border-military-green/20">{filledCount} / {grid.level_count} DEPTH</span>
        </div>
        <div className="h-2 w-full bg-military-bg border border-border-military overflow-hidden">
          <motion.div 
            initial={{ width: 0 }}
            animate={{ width: `${progress}%` }}
            className="h-full bg-military-green"
          />
        </div>
      </div>

      {grid.levels && (
        <div className="flex flex-wrap gap-1 mt-2">
          {grid.levels.map((level, i) => (
            <div 
              key={i}
              title={formatCurrency(level.price)}
              className={cn(
                "w-2.5 h-2.5 transition-all border border-transparent",
                level.filled 
                  ? "bg-military-green" 
                  : "bg-white/50 border-border-military/30 hover:border-military-accent",
                level.is_buy 
                  ? "opacity-100" 
                  : "opacity-60"
              )}
            />
          ))}
        </div>
      )}
      
      <div className="mt-2 text-center">
         <p className="font-mono text-[8px] text-text-secondary/40 font-bold tracking-[0.3em] uppercase">SYSTEM CLEARANCE // SEPOLIA SECTOR</p>
      </div>
    </motion.div>
  );
};
