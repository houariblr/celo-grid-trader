import React from 'react';
import { motion } from 'motion/react';
import { cn } from '../lib/utils';

interface StatCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  icon?: React.ReactNode;
  trend?: 'up' | 'down' | 'neutral';
  loading?: boolean;
  className?: string;
}

export const StatCard: React.FC<StatCardProps> = ({ 
  title, 
  value, 
  subtitle, 
  icon, 
  trend, 
  loading,
  className 
}) => {
  return (
    <motion.div 
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      className={cn("glass-morphism p-6 transition-all hover:border-military-accent/50", className)}
    >
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-2">
             <div className="w-1.5 h-1.5 bg-military-green" />
             <p className="stat-label text-text-secondary">{title}</p>
          </div>
          <div className="mt-1 flex items-baseline gap-2">
            <h3 className="text-3xl font-display font-black tracking-tight text-military-deep leading-none uppercase">
              {loading ? <div className="h-8 w-24 bg-military-bg animate-pulse" /> : value}
            </h3>
            {trend && (
              <span className={cn(
                "text-[10px] font-black font-mono",
                trend === 'up' ? "text-military-green" : trend === 'down' ? "text-red-600" : "text-text-secondary"
              )}>
                {trend === 'up' ? '▲' : trend === 'down' ? '▼' : '▬'}
              </span>
            )}
          </div>
          {subtitle && (
            <p className="mt-3 text-[10px] text-text-secondary font-mono uppercase tracking-widest leading-tight opacity-70 font-bold">{subtitle}</p>
          )}
        </div>
        <div className="p-1 opacity-20 text-military-deep">
          {icon}
        </div>
      </div>
    </motion.div>
  );
};
