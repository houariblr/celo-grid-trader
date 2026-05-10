import React, { useState } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { X, Plus, Info, AlertTriangle, Loader2, Wallet } from 'lucide-react';
import { Address, parseUnits } from 'viem';
import { cn } from '../lib/utils';

interface CreateGridModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (data: any) => Promise<void>;
  isConnected: boolean;
  isDeploying: boolean;
  isWaiting: boolean;
}

export const CreateGridModal: React.FC<CreateGridModalProps> = ({ 
  isOpen, 
  onClose, 
  onSubmit, 
  isConnected,
  isDeploying,
  isWaiting
}) => {
  const [lower, setLower] = useState('0.40');
  const [upper, setUpper] = useState('0.70');
  const [levels, setLevels] = useState('10');
  const [amount, setAmount] = useState('5.0');
  const [yieldEnabled, setYieldEnabled] = useState(false);

  // Checks if all fields are valid
  const isValid = lower && upper && levels && amount && isConnected;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!isValid) return;

    try {
      await onSubmit({
        lower: parseUnits(lower, 18),
        upper: parseUnits(upper, 18),
        levels: BigInt(levels),
        amount: parseUnits(amount, 18),
        yieldEnabled,
        slippageBps: 100n // 1% default as requested
      });
    } catch (err) {
      console.error("Grid deployment error:", err);
    }
  };

  // Combine loading states
  const isLoading = isDeploying || isWaiting;

  return (
    <AnimatePresence>
      {isOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
          <motion.div 
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="absolute inset-0 bg-ink/60 backdrop-blur-sm"
          />
          <motion.div 
            initial={{ opacity: 0, scale: 0.95, y: 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 10 }}
            className="bg-cream border-2 border-ink w-full max-w-md p-8 relative overflow-hidden shadow-2xl"
          >
            {/* CLASSIFIED STAMP BACKGROUND */}
            <div className="absolute -top-6 -right-6 pointer-events-none opacity-5 origin-center rotate-12">
               <div className="text-[120px] font-display font-black leading-none uppercase select-none">GRID-ALPHA</div>
            </div>

            <div className="flex justify-between items-center mb-8 pb-4 border-b border-border-agency">
              <div className="flex items-center gap-4">
                <div className="w-10 h-10 border border-ink flex items-center justify-center bg-cyan-agency text-white">
                  <Plus className="w-6 h-6" />
                </div>
                <div>
                  <h3 className="font-display text-lg font-black uppercase tracking-tight">Deployment Authorization</h3>
                  <p className="font-mono text-[9px] text-ink-muted uppercase tracking-[0.25em] mt-0.5">PURSUIT PROTOCOL // SEPOLIA SECTOR</p>
                </div>
              </div>
              <button 
                onClick={onClose}
                className="text-ink-muted hover:text-ink transition-colors"
                id="close-modal-btn"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <form onSubmit={handleSubmit} className="space-y-6">
              <div className="grid grid-cols-2 gap-6">
                <div className="space-y-2">
                  <label className="stat-label flex items-center gap-2">
                    <span className="w-1 h-1 bg-cyan-agency rounded-full" /> LOWER BOUND
                  </label>
                  <div className="relative">
                    <span className="absolute left-3 top-1/2 -translate-y-1/2 text-[10px] font-mono font-bold text-ink-muted">$</span>
                    <input 
                      type="number" step="0.0001"
                      placeholder="0.40"
                      value={lower} onChange={(e) => setLower(e.target.value)}
                      className="w-full bg-cream-dark border border-border-agency px-6 py-2.5 text-sm font-mono text-ink focus:outline-none focus:border-cyan-agency transition-colors"
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <label className="stat-label flex items-center gap-2">
                    <span className="w-1 h-1 bg-cyan-agency rounded-full" /> UPPER BOUND
                  </label>
                  <div className="relative">
                    <span className="absolute left-3 top-1/2 -translate-y-1/2 text-[10px] font-mono font-bold text-ink-muted">$</span>
                    <input 
                      type="number" step="0.0001"
                      placeholder="0.70"
                      value={upper} onChange={(e) => setUpper(e.target.value)}
                      className="w-full bg-cream-dark border border-border-agency px-6 py-2.5 text-sm font-mono text-ink focus:outline-none focus:border-cyan-agency transition-colors"
                    />
                  </div>
                </div>
              </div>

              <div className="space-y-3">
                <label className="stat-label">GRID SIGNAL DENSITY</label>
                <input 
                  type="range" min="5" max="100" step="1"
                  value={levels} onChange={(e) => setLevels(e.target.value)}
                  className="w-full h-1.5 bg-cream-dark appearance-none cursor-pointer accent-cyan-agency border border-border-agency rounded-sm"
                />
                <div className="flex justify-between font-mono text-[9px] text-ink-muted uppercase tracking-widest font-black">
                  <span>LO-DEN</span>
                  <span className="text-violet-agency bg-violet-agency/5 px-2 py-0.5 border border-violet-agency/20">{levels} SIGNAL LEVELS</span>
                  <span>HI-DEN</span>
                </div>
              </div>

              <div className="space-y-2">
                <label className="stat-label">CAPITAL ALLOCATION (CELO)</label>
                <div className="relative">
                  <Wallet className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-ink-muted" />
                  <input 
                    type="number" step="0.1"
                    placeholder="5.0"
                    value={amount} onChange={(e) => setAmount(e.target.value)}
                    className="w-full bg-cream-dark border border-border-agency pl-10 pr-4 py-3 text-sm font-mono text-ink focus:outline-none focus:border-cyan-agency font-bold"
                  />
                </div>
              </div>

              {/* Yield Checkbox */}
              <label className="flex items-center gap-4 p-4 bg-cream-dark border border-border-agency cursor-pointer hover:bg-cream-mid transition-all select-none group">
                <div className="relative flex items-center">
                  <input 
                    type="checkbox"
                    checked={yieldEnabled}
                    onChange={(e) => setYieldEnabled(e.target.checked)}
                    className="peer sr-only"
                  />
                  <div className="w-5 h-5 border-2 border-border-agency peer-checked:bg-ink peer-checked:border-ink transition-all" />
                  <Plus className="absolute inset-0 m-auto w-3 h-3 text-cream opacity-0 peer-checked:opacity-100 transition-opacity" strokeWidth={4} />
                </div>
                <div>
                  <p className="text-xs font-black uppercase tracking-tight text-ink">Enable Pursuit Yield</p>
                  <p className="text-[9px] text-ink-muted uppercase font-mono tracking-wider mt-0.5">AUTODEPLOY IDLE CAPITAL TO LENDING AGGREGATORS</p>
                </div>
              </label>

              <div className="p-4 bg-amber-agency/5 border border-amber-agency/20 flex gap-4">
                <AlertTriangle className="w-5 h-5 text-amber-agency shrink-0" />
                <p className="text-[10px] text-ink-soft leading-relaxed font-mono uppercase font-black uppercase">
                  {isValid 
                    ? "STATUS: COORDINATES VALIDATED. SIGNAL READY FOR DEPLOYMENT." 
                    : "STATUS: AWAITING CREDENTIALS & COORDINATE ENTRY."}
                </p>
              </div>

              <div className="pt-2">
                <button 
                  type="submit"
                  id="deploy-bot-btn"
                  disabled={!isValid || isLoading}
                  className={cn(
                    "w-full py-4 font-display font-black uppercase tracking-[0.2em] text-xs transition-all flex items-center justify-center gap-3 shadow-lg",
                    !isValid || isLoading 
                      ? "bg-cream-dark text-ink-faint border border-border-agency cursor-not-allowed" 
                      : "bg-ink text-white hover:bg-ink-soft active:translate-y-0.5"
                  )}
                >
                  {isLoading ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin" />
                      AUTHORIZING...
                    </>
                  ) : (
                    '⧉ AUTHORIZE DEPLOYMENT'
                  )}
                </button>
                <div className="mt-3 text-center">
                   <p className="font-mono text-[8px] text-ink-faint uppercase font-black tracking-[0.4em]">PURSUIT COMMAND // ALPHA CLEARANCE REQUIRED</p>
                </div>
              </div>
            </form>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
};
