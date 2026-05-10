import React from 'react';
import { Terminal } from 'lucide-react';
import { TransactionUpdate } from '../types';
import { cn } from '../lib/utils';

interface TransactionHistoryProps {
  transactions: TransactionUpdate[];
}

export const TransactionHistory: React.FC<TransactionHistoryProps> = ({ transactions }) => {
  return (
    <div className="flex flex-col h-full bg-cream-mid">
      <div className="overflow-x-auto custom-scrollbar">
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="bg-cream-dark border-b-2 border-border-agency font-mono text-[9px] tracking-[0.2em] text-ink-muted uppercase">
              <th className="px-4 py-3 font-normal">TIMESTAMP</th>
              <th className="px-4 py-3 font-normal text-center">TYPE</th>
              <th className="px-4 py-3 font-normal">TX HASH</th>
              <th className="px-4 py-3 font-normal">PRICE</th>
              <th className="px-4 py-3 font-normal text-right">STATUS</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border-agency/50 font-mono text-[11px]">
            {transactions.length > 0 ? (
              transactions.map((tx) => (
                <tr key={tx.id} className="hover:bg-cyan-agency/5 transition-colors group">
                  <td className="px-4 py-3 text-ink-muted shrink-0 whitespace-nowrap">
                    {new Date(tx.timestamp).toLocaleTimeString([], { hour12: false })}
                  </td>
                  <td className="px-4 py-3 text-center">
                    <span className={cn(
                      "px-2 py-0.5 rounded-sm font-black text-[9px]",
                      tx.type === 'BUY' ? "bg-mint-agency/10 text-mint-agency" : "bg-cyan-agency/10 text-cyan-agency"
                    )}>
                      {tx.type}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    {tx.hash ? (
                      <a 
                        href={`https://sepolia.celoscan.io/tx/${tx.hash}`}
                        target="_blank"
                        rel="noreferrer"
                        className="text-cyan-agency hover:underline flex items-center gap-1"
                      >
                        {tx.hash.slice(0, 6)}...
                        <Terminal className="w-3 h-3" />
                      </a>
                    ) : (
                      <span className="text-ink-faint italic font-light italic text-[9px]">internal_op</span>
                    )}
                  </td>
                  <td className="px-4 py-3 font-black text-ink whitespace-nowrap">
                    ${tx.price.toFixed(4)}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-2 uppercase tracking-widest text-[9px] font-black whitespace-nowrap">
                      {tx.status === 'SUCCESS' ? (
                        <>
                          <span className="text-mint-agency">CONFIRMED</span>
                          <div className="w-1.5 h-1.5 rounded-full bg-mint-agency" />
                        </>
                      ) : tx.status === 'PENDING' ? (
                        <>
                          <span className="text-amber-agency animate-pulse">PENDING...</span>
                          <div className="w-1.5 h-1.5 rounded-full bg-amber-agency animate-pulse" />
                        </>
                      ) : (
                        <>
                          <span className="text-red-agency">REVERTED</span>
                          <div className="w-1.5 h-1.5 rounded-full bg-red-agency" />
                        </>
                      )}
                    </div>
                  </td>
                </tr>
              ))
            ) : (
              <tr>
                <td colSpan={5} className="px-4 py-20 text-center text-ink-faint italic text-xs uppercase tracking-[0.3em]">
                   NO FIELD INCIDENTS RECORDED // MONITORING SIGNAL...
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
};
