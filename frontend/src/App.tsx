/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 *
 * App.tsx — Celo Grid Keeper V2 Dashboard (Competition Edition)
 *
 * Competition improvements:
 *   [FIX]  isMiniPay now checks window.ethereum.isMiniPay (per Celo docs)
 *          instead of truthiness of window.ethereum (matched MetaMask too).
 *   [FIX]  MiniPay auto-connects on page load so wallet address shows
 *          immediately inside the Opera MiniPay browser.
 *   [ADD]  PWA install prompt intercepted and surfaced as a native banner —
 *          judges can install the app on their device with one tap.
 *   [ADD]  cUSD equivalent price shown alongside CELO for non-crypto judges.
 *   [ADD]  Network guard: warns if connected wallet is not on Celo Sepolia.
 */

import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import {
  Activity,
  ShieldCheck,
  Terminal,
  Globe,
  Database,
  Plus,
  Wallet,
  CheckCircle,
  Smartphone,
  AlertTriangle,
  Download,
  X,
} from 'lucide-react';
import {
  useAccount,
  useConnect,
  useWriteContract,
  useWaitForTransactionReceipt,
  useDisconnect,
  useChainId,
  useSwitchChain,
} from 'wagmi';
import { injected } from 'wagmi/connectors';
import { celoSepolia } from 'wagmi/chains';

import { StatsGrid }          from './components/StatsGrid';
import { StatCard }           from './components/StatCard';
import { GridCard }           from './components/GridCard';
import { TransactionHistory } from './components/TransactionHistory';
import { FibonacciView }      from './components/FibonacciView';
import { CreateGridModal }    from './components/CreateGridModal';
import {
  ResponsiveContainer,
  AreaChart,
  Area,
  ReferenceLine,
} from 'recharts';

import { GRID_ABI, GRID_CONTRACT_ADDRESS, CELO_TOKEN_SEPOLIA, CUSD_TOKEN_SEPOLIA } from './lib/constants';
import type {
  PriceUpdate,
  CircuitBreakerUpdate,
  GridStatus,
  TransactionUpdate,
  HealthUpdate,
  WsMessage,
} from './types';
import { cn, shortAddr } from './lib/utils';

// ─── PWA deferred install prompt ─────────────────────────────────────────────
let deferredInstallPrompt: any = null;

export default function App() {
  // ── Wallet ──────────────────────────────────────────────────────────────
  const { address, isConnected: isWalletConnected } = useAccount();
  const { connect: connectWallet, isPending: isConnecting } = useConnect();
  const { disconnect } = useDisconnect();
  const chainId = useChainId();
  const { switchChain } = useSwitchChain();

  const { writeContractAsync, data: hash, isPending: isDeploying } = useWriteContract();
  const { isLoading: isWaitingForReceipt, isSuccess: isTxSuccess } =
    useWaitForTransactionReceipt({ hash });

  // ── MiniPay detection (Celo docs: window.ethereum.isMiniPay) ──────────
  const isMiniPay =
    typeof window !== 'undefined' &&
    !!(window as any).ethereum?.isMiniPay;

  // MiniPay auto-connect — wallet is already unlocked inside the browser
  useEffect(() => {
    if (isMiniPay && !isWalletConnected) {
      connectWallet({ connector: injected() });
    }
  }, [isMiniPay, isWalletConnected, connectWallet]);

  // ── App state ────────────────────────────────────────────────────────────
  const [price,        setPrice]        = useState<PriceUpdate>();
  const [cb,           setCb]           = useState<CircuitBreakerUpdate>();
  const [health,       setHealth]       = useState<HealthUpdate>();
  const [grids,        setGrids]        = useState<GridStatus[]>([]);
  const [transactions, setTransactions] = useState<TransactionUpdate[]>([]);
  const [priceHistory, setPriceHistory] = useState<{ price: number; timestamp: string }[]>([]);
  const [wsUrl,        setWsUrl]        = useState('ws://localhost:8080');
  const [isLive,       setIsLive]       = useState(false);
  const [wsConnected,  setWsConnected]  = useState(false);
  const [isModalOpen,  setIsModalOpen]  = useState(false);
  const [showPwaBanner, setShowPwaBanner] = useState(false);

  const ws = useRef<WebSocket | null>(null);

  // ── PWA install prompt ───────────────────────────────────────────────────
  useEffect(() => {
    const handler = (e: Event) => {
      e.preventDefault();
      deferredInstallPrompt = e;
      setShowPwaBanner(true);
    };
    window.addEventListener('beforeinstallprompt', handler);
    return () => window.removeEventListener('beforeinstallprompt', handler);
  }, []);

  const handleInstall = async () => {
    if (!deferredInstallPrompt) return;
    deferredInstallPrompt.prompt();
    const { outcome } = await deferredInstallPrompt.userChoice;
    if (outcome === 'accepted') setShowPwaBanner(false);
    deferredInstallPrompt = null;
  };

  // ── Mock data for preview ────────────────────────────────────────────────
  useEffect(() => {
    if (isLive) return;
    const mockPrice = 0.5234;
    setPrice({ price: mockPrice, sources: 4, deviation_warning: false, timestamp: new Date().toISOString() });
    setCb({ reason: 'NONE', is_active: false, cb_price: 0.4521, ratio: 0.786 });
    setGrids([
      {
        grid_id:     1,
        lower_price: 0.45,
        upper_price: 0.65,
        active:      true,
        level_count: 50,
        levels: Array.from({ length: 50 }, (_, i) => ({
          index:  i,
          price:  0.45 + i * 0.004,
          filled: i < 15 || i > 45,
          is_buy: i < 25,
        })),
      },
      {
        grid_id:     2,
        lower_price: 0.40,
        upper_price: 0.70,
        active:      true,
        level_count: 35,
        levels: Array.from({ length: 35 }, (_, i) => ({
          index:  i,
          price:  0.40 + i * 0.008,
          filled: i === 12 || i === 18,
          is_buy: true,
        })),
      },
    ]);
    setHealth({
      chain_client: {
        rpc_status:          'Healthy',
        rpc_current_url:     'https://alfajores-forno.celo-testnet.org',
        total_requests:      1842,
        success_rate:        100.0,
        avg_response_time_ms: 38,
        consecutive_failures: 0,
      },
      last_execution_seconds_ago: 12,
      ohlc_fresh: true,
      timestamp: new Date().toISOString(),
    });
    setPriceHistory(
      Array.from({ length: 30 }, (_, i) => ({
        price:     mockPrice + (Math.random() * 0.04 - 0.02),
        timestamp: new Date(Date.now() - (30 - i) * 60_000).toISOString(),
      }))
    );
  }, [isLive]);

  // ── WebSocket ─────────────────────────────────────────────────────────────
  const connectWs = () => {
    if (ws.current) { ws.current.close(); }
    try {
      const socket = new WebSocket(wsUrl);
      ws.current = socket;

      socket.onopen  = () => { setWsConnected(true); setIsLive(true); };
      socket.onclose = () => { setWsConnected(false); setIsLive(false); };
      socket.onerror = () => { setWsConnected(false); setIsLive(false); };
      socket.onmessage = (e) => {
        try { handleMessage(JSON.parse(e.data) as WsMessage); } catch {}
      };
    } catch {
      setWsConnected(false);
      setIsLive(false);
    }
  };

  const handleMessage = (msg: WsMessage) => {
    switch (msg.type) {
      case 'PRICE_UPDATE': {
        const p = msg.data as PriceUpdate;
        setPrice(p);
        setPriceHistory(prev => [...prev.slice(-49), { price: p.price, timestamp: p.timestamp }]);
        break;
      }
      case 'CIRCUIT_BREAKER':
        setCb(msg.data as CircuitBreakerUpdate);
        break;
      case 'GRID_STATUS': {
        const g = msg.data as GridStatus;
        setGrids(prev => {
          const idx = prev.findIndex(x => x.grid_id === g.grid_id);
          if (idx >= 0) { const next = [...prev]; next[idx] = { ...next[idx], ...g }; return next; }
          return [...prev, g];
        });
        break;
      }
      case 'TRANSACTION':
        setTransactions(prev => [msg.data as TransactionUpdate, ...prev.slice(0, 49)]);
        break;
      case 'HEALTH':
        setHealth(msg.data as HealthUpdate);
        break;
    }
  };

  // ── Grid deployment ───────────────────────────────────────────────────────
  const handleCreateGrid = async (formData: any) => {
    if (!address) { connectWallet({ connector: injected() }); return; }
    try {
      const txHash = await writeContractAsync({
        account:      address,
        chain:        celoSepolia,
        address:      GRID_CONTRACT_ADDRESS,
        abi:          GRID_ABI,
        functionName: 'createGrid',
        args: [
          CELO_TOKEN_SEPOLIA,
          CUSD_TOKEN_SEPOLIA,
          formData.lower,
          formData.upper,
          formData.levels,
          formData.amount,
          formData.yieldEnabled,
          formData.slippageBps,
        ],
      });
      if (txHash) {
        setTransactions(prev => [{
          id:        Math.random().toString(36).slice(2, 9),
          hash:      txHash,
          type:      'BUY' as const,
          price:     price?.price ?? 0,
          status:    'PENDING' as const,
          timestamp: new Date().toISOString(),
        }, ...prev]);
      }
    } catch (err) {
      console.error('Grid deployment rejected:', err);
    }
  };

  // Mark tx success
  useEffect(() => {
    if (isTxSuccess && hash) {
      setTransactions(prev =>
        prev.map(tx => tx.hash === hash ? { ...tx, status: 'SUCCESS' as const } : tx)
      );
      setTimeout(() => setIsModalOpen(false), 2000);
    }
  }, [isTxSuccess, hash]);

  // ── Wrong network guard ───────────────────────────────────────────────────
  const isWrongNetwork = isWalletConnected && chainId !== celoSepolia.id;

  // ── cUSD equivalent (1 CELO = price USD, 1 cUSD ≈ 1 USD) ────────────────
  const cusdEquivalent = price ? `≈ ${price.price.toFixed(4)} cUSD` : null;

  return (
    <div className="min-h-screen bg-military-bg text-text-primary flex flex-col font-sans selection:bg-military-green selection:text-white">

      {/* ── CLASSIFICATION BANNER ── */}
      <div className="bg-military-deep text-white px-4 md:px-6 py-2 flex items-center justify-between font-mono text-[10px] tracking-[0.3em] uppercase z-50 shrink-0">
        <div className="flex items-center gap-4 md:gap-6 overflow-hidden">
          <span className="bg-military-green text-military-deep px-2 py-0.5 font-black shrink-0">UNCLASSIFIED</span>
          <span className="opacity-70 font-bold hidden sm:block truncate">ALL-DOMAIN GRID RESOLUTION OFFICE (AGRO)</span>
          <span className="flex items-center gap-2 text-military-green shrink-0">
            <Activity className="w-3 h-3 animate-pulse" />
            <span className="animate-agency-pulse hidden md:inline">SIGNAL_SYNC_ACTIVE</span>
          </span>
        </div>
        <div className="font-bold opacity-70 border-l border-white/20 pl-4 shrink-0 hidden sm:block">
          {new Date().toISOString().replace('T', ' ').slice(0, 19)} ZULU
        </div>
      </div>

      {/* ── WRONG NETWORK BANNER ── */}
      {isWrongNetwork && (
        <div className="bg-red-600 text-white px-6 py-2 flex items-center justify-between font-mono text-[10px] tracking-widest uppercase">
          <span className="flex items-center gap-2">
            <AlertTriangle className="w-4 h-4 shrink-0" />
            WRONG NETWORK — SWITCH TO CELO SEPOLIA TO EXECUTE TRADES
          </span>
          <button
            onClick={() => switchChain({ chainId: celoSepolia.id })}
            className="bg-white text-red-600 px-4 py-1 font-black text-[10px] hover:bg-red-50 active:scale-95 transition-all"
          >
            SWITCH NETWORK
          </button>
        </div>
      )}

      {/* ── TICKER ── */}
      <div className="bg-white border-b border-border-military overflow-hidden h-9 flex items-center shrink-0">
        <div className="flex gap-12 animate-ticker-scroll whitespace-nowrap px-4 font-mono text-[11px] font-black tracking-widest text-text-secondary uppercase">
          {/* Duplicated for seamless loop */}
          {[0, 1].map(copy => (
            <span key={copy} className="flex gap-12">
              <span className="flex items-center gap-3">
                <span className="w-1.5 h-1.5 bg-military-green rotate-45 inline-block" />
                CELO/USDT <span className="text-military-deep">${price?.price.toFixed(4) ?? '0.0000'}</span>
                {cusdEquivalent && <span className="text-text-secondary opacity-60 text-[9px]">{cusdEquivalent}</span>}
              </span>
              <span className="flex items-center gap-3">
                <span className="w-1.5 h-1.5 bg-military-green rotate-45 inline-block" />
                VOLATILITY <span className="text-military-green">STABLE_01</span>
              </span>
              <span className="flex items-center gap-3">
                <span className="w-1.5 h-1.5 bg-military-green rotate-45 inline-block" />
                CONTAINMENT{' '}
                <span className={cn('px-2', cb?.is_active ? 'bg-red-100 text-red-600' : 'bg-military-green/10 text-military-green')}>
                  {cb?.is_active ? 'PROTOCOL_BREACH' : 'SECURE'}
                </span>
              </span>
              <span className="flex items-center gap-3">
                <span className="w-1.5 h-1.5 bg-military-green rotate-45 inline-block" />
                LATENCY <span className="text-military-deep">{health?.chain_client.avg_response_time_ms.toFixed(0) ?? '0'} MS</span>
              </span>
              <span className="flex items-center gap-3">
                <span className="w-1.5 h-1.5 bg-military-green rotate-45 inline-block" />
                POSITIONS <span className="text-military-green">{grids.length} ACTIVE</span>
              </span>
              <span className="flex items-center gap-3">
                <span className="w-1.5 h-1.5 bg-military-green rotate-45 inline-block" />
                {isMiniPay ? <><Smartphone className="w-3 h-3 inline" /> MINIPAY_DETECTED</> : 'BROWSER_MODE'}
              </span>
            </span>
          ))}
        </div>
      </div>

      {/* ── HEADER ── */}
      <header className="bg-white border-b-4 border-military-deep px-4 md:px-8 py-6 md:py-8 shrink-0">
        <div className="max-w-[1400px] mx-auto flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
          <div className="flex items-center gap-4 md:gap-8">
            <div className="w-14 h-14 md:w-20 md:h-20 border-4 border-military-deep flex items-center justify-center shrink-0 bg-military-alt relative shadow-inner">
              <div className="absolute inset-2 border border-military-deep/20 flex items-center justify-center">
                <Database className="w-6 h-6 md:w-10 md:h-10 text-military-deep" />
              </div>
              <div className="absolute -top-1 -right-1 w-4 h-4 bg-military-green border border-military-deep" />
            </div>
            <div>
              <h1 className="font-display text-xl md:text-2xl font-black tracking-tight uppercase leading-none text-military-deep">
                ALL-DOMAIN GRID <br className="hidden sm:block" /> RESOLUTION OFFICE{' '}
                <span className="text-military-green">AGRO</span>
              </h1>
              <p className="font-mono text-[10px] md:text-[11px] tracking-[0.3em] md:tracking-[0.4em] text-text-secondary uppercase mt-2 font-black border-l-2 border-military-green pl-3">
                PURSUIT PROTOCOL // SEPOLIA SECTOR COMMAND
              </p>
            </div>
          </div>

          <div className="flex items-center gap-4 self-end sm:self-auto">
            {/* Coordinates — desktop only */}
            <div className="hidden lg:flex flex-col items-end gap-1 font-mono text-[10px] tracking-widest text-text-secondary uppercase font-bold">
              <span className="flex items-center gap-2">VECTOR_COORD: <span className="text-military-deep">41.37°N 2.16°E</span></span>
              <span className="flex items-center gap-2">SYSTEM_TIME: <span className="text-military-deep">{new Date().toLocaleDateString()}</span></span>
              <span className="flex items-center gap-2">STATUS: <span className="text-military-green">NOMINAL</span></span>
            </div>

            <div className="h-12 w-[1px] bg-border-military hidden lg:block" />

            {/* Wallet button */}
            {address ? (
              <button
                onClick={() => disconnect()}
                className="bg-military-alt border-2 border-military-deep px-4 md:px-6 py-2 md:py-3 flex items-center gap-3 hover:bg-military-bg transition-all active:translate-y-px shadow-[4px_4px_0px_#16a34a]"
              >
                <div className="w-3 h-3 bg-military-green" />
                <span className="font-mono text-[11px] font-black text-military-deep tracking-[0.2em]">
                  {shortAddr(address)}
                </span>
              </button>
            ) : (
              <button
                onClick={() => connectWallet({ connector: injected() })}
                disabled={isConnecting}
                className="bg-military-deep text-white px-6 md:px-8 py-2 md:py-3 font-display text-[11px] font-black uppercase tracking-[0.3em] hover:bg-military-accent transition-all active:scale-95 shadow-[4px_4px_0px_#16a34a] flex items-center gap-2"
              >
                {isConnecting ? (
                  <><Activity className="w-4 h-4 animate-spin" /> AUTHORIZING...</>
                ) : (
                  <><Wallet className="w-4 h-4" /> ESTABLISH LINK</>
                )}
              </button>
            )}
          </div>
        </div>
      </header>

      {/* ── TACTICAL NAV ── */}
      <nav className="bg-military-deep border-b border-white/10 w-full overflow-x-auto shrink-0">
        <div className="max-w-[1400px] mx-auto flex px-4 min-w-max">
          {['DASHBOARD', 'POSITION_MANIFEST', 'INTEL_REPORTS', 'INCIDENT_LOG'].map((item, i) => (
            <button
              key={item}
              className={cn(
                'px-6 md:px-8 py-4 font-mono text-[10px] tracking-[0.3em] uppercase transition-all hover:text-military-green whitespace-nowrap',
                i === 0 ? 'bg-white/5 text-military-green border-b-2 border-military-green font-black' : 'text-white/50'
              )}
            >
              {item}
            </button>
          ))}
          <div className="ml-auto flex items-center gap-3 pr-4">
            <div className="flex items-center gap-2 bg-white/5 px-3 py-2 border border-white/10">
              <Terminal className="w-4 h-4 text-military-green shrink-0" />
              <input
                value={wsUrl}
                onChange={e => setWsUrl(e.target.value)}
                className="bg-transparent border-none text-[10px] font-mono text-white/70 focus:outline-none w-32 md:w-44"
                placeholder="ws://localhost:8080"
              />
            </div>
            <button
              onClick={connectWs}
              className={cn(
                'px-4 md:px-6 py-2 font-mono text-[10px] font-black uppercase tracking-widest transition-all whitespace-nowrap',
                wsConnected ? 'bg-military-green text-military-deep' : 'bg-white/10 text-white hover:bg-white/20'
              )}
            >
              {wsConnected ? '● ONLINE' : 'SYNC_LINK'}
            </button>
          </div>
        </div>
      </nav>

      {/* ── MAIN ── */}
      <main className="max-w-[1400px] mx-auto px-4 md:px-8 py-8 md:py-12 w-full flex flex-col gap-8 md:gap-12">

        {/* HERO */}
        <section className="grid grid-cols-1 lg:grid-cols-2 bg-white border-2 border-military-deep shadow-2xl relative overflow-hidden">
          <div className="absolute inset-0 pointer-events-none opacity-[0.03]"
            style={{ backgroundImage: 'radial-gradient(circle, #000 1px, transparent 1px)', backgroundSize: '30px 30px' }}
          />

          {/* Price chart */}
          <div className="border-b-2 lg:border-b-0 lg:border-r-2 border-military-deep p-8 md:p-12 flex items-center justify-center bg-military-alt/30 relative">
            <div className="relative w-full aspect-square max-w-[400px]">
              <div className="absolute inset-0 flex items-center justify-center opacity-20 pointer-events-none">
                <div className="w-[80%] h-[80%] border-2 border-dashed border-military-deep rounded-full"
                  style={{ animation: 'spin-slow 20s linear infinite' }} />
                <div className="w-[55%] h-[55%] border-2 border-dashed border-military-deep rounded-full absolute"
                  style={{ animation: 'spin-slow-reverse 15s linear infinite' }} />
              </div>
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={priceHistory}>
                  <defs>
                    <linearGradient id="heroGradient" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%"  stopColor="#16a34a" stopOpacity={0.1} />
                      <stop offset="95%" stopColor="#16a34a" stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <Area
                    type="monotone"
                    dataKey="price"
                    stroke="#0f172a"
                    strokeWidth={3}
                    fill="url(#heroGradient)"
                    animationDuration={500}
                  />
                  <ReferenceLine y={price?.price ?? 0} stroke="#16a34a" strokeDasharray="3 3" />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </div>

          {/* Hero text */}
          <div className="p-8 md:p-12 flex flex-col justify-between gap-8">
            <div>
              <p className="font-mono text-[9px] tracking-[0.4em] text-text-secondary uppercase mb-3 flex items-center gap-2 font-black">
                <span className="inline-block w-4 h-[1px] bg-military-green" />
                PURSUIT ACQUISITION SYSTEM — ACTIVE
              </p>
              <h2 className="font-display text-3xl md:text-4xl font-black uppercase text-military-deep leading-none tracking-tight">
                FIBONACCI<br />GRID<br />COMMAND
              </h2>
              <p className="mt-6 text-sm text-text-secondary leading-relaxed font-mono border-l-4 border-military-green pl-4 font-bold uppercase tracking-wider">
                ATR-adaptive grid executes at Fibonacci retracement levels.
                0.786 circuit breaker suspends buys on trend reversal signals.
              </p>
            </div>
            <div className="flex flex-col gap-4">
              <div className="grid grid-cols-3 gap-px bg-border-military border border-border-military">
                {[
                  { label: 'PRICE',    val: `$${(price?.price ?? 0).toFixed(4)}` },
                  { label: 'cUSD EQ', val: cusdEquivalent ?? '—' },
                  { label: 'CB',       val: cb?.is_active ? '⛔ TRIP' : '✅ SAFE' },
                ].map(({ label, val }) => (
                  <div key={label} className="bg-white px-3 py-3 text-center">
                    <p className="stat-label mb-1">{label}</p>
                    <p className="font-display text-sm font-black text-military-deep truncate">{val}</p>
                  </div>
                ))}
              </div>
              <div className="flex gap-3">
                <button
                  onClick={() => setIsModalOpen(true)}
                  className="flex-1 bg-military-deep text-white py-4 font-display text-[11px] font-black uppercase tracking-[0.3em] hover:bg-military-accent transition-all active:scale-95 shadow-[4px_4px_0px_#16a34a] flex items-center justify-center gap-2"
                >
                  <Plus className="w-4 h-4" />
                  DEPLOY GRID
                </button>
                {isMiniPay && (
                  <div className="flex items-center gap-2 px-4 py-4 bg-blue-50 border border-blue-200">
                    <Smartphone className="w-4 h-4 text-blue-600 shrink-0" />
                    <span className="font-mono text-[9px] text-blue-700 font-black uppercase tracking-wider">MiniPay</span>
                  </div>
                )}
              </div>
            </div>
          </div>
        </section>

        {/* STATS */}
        <section>
          <div className="flex items-center gap-4 mb-4">
            <p className="stat-label flex items-center gap-2">
              <span className="w-2 h-2 bg-military-green rotate-45 inline-block" />
              REAL-TIME TELEMETRY — {isLive ? 'LIVE FEED' : 'PREVIEW MODE'}
            </p>
            {!isLive && (
              <span className="font-mono text-[9px] text-amber-600 uppercase tracking-widest font-black border border-amber-200 bg-amber-50 px-2 py-0.5">
                ⚠ MOCK DATA — CONNECT WS FOR LIVE
              </span>
            )}
          </div>
          <StatsGrid price={price} cb={cb} health={health} />
        </section>

        {/* FIBONACCI VIEW */}
        <section>
          <p className="stat-label mb-4 flex items-center gap-2">
            <span className="w-2 h-2 bg-military-green rotate-45 inline-block" />
            STRATEGIC VECTOR ANALYSIS — FIBONACCI SIGNAL GRID
          </p>
          <FibonacciView price={price} cb={cb} history={priceHistory} />
        </section>

        {/* TRANSACTION HISTORY */}
        <section>
          <p className="stat-label mb-4 flex items-center gap-2">
            <span className="w-2 h-2 bg-military-green rotate-45 inline-block" />
            FIELD INCIDENT LOG — EXECUTION RECORD
          </p>
          <div className="bg-white border-2 border-military-deep">
            <TransactionHistory transactions={transactions} />
          </div>
        </section>

        {/* GRID INVENTORY */}
        <section>
          <div className="flex items-center justify-between mb-4">
            <p className="stat-label flex items-center gap-2">
              <span className="w-2 h-2 bg-military-green rotate-45 inline-block" />
              ON-CHAIN POSITION MANIFEST
            </p>
            <span className="font-mono text-[10px] font-black text-military-green uppercase tracking-widest bg-military-green/10 px-3 py-1 border border-military-green/20">
              {grids.filter(g => g.active).length} ACTIVE
            </span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
            {grids.map(grid => (
              <GridCard key={grid.grid_id} grid={grid} />
            ))}
          </div>
        </section>
      </main>

      {/* ── SUCCESS TOAST ── */}
      <AnimatePresence>
        {isTxSuccess && (
          <motion.div
            initial={{ opacity: 0, y: 50 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 50 }}
            className="fixed bottom-8 right-4 md:right-8 z-[100] bg-white border-2 border-military-green p-4 shadow-2xl flex items-center gap-4 max-w-sm"
          >
            <div className="p-2 bg-military-green/10">
              <CheckCircle className="w-5 h-5 text-military-green" />
            </div>
            <div>
              <p className="font-display text-sm font-black text-military-deep uppercase">Grid Deployed!</p>
              <p className="font-mono text-[10px] text-text-secondary uppercase tracking-wider mt-0.5">Confirmed on Celo Sepolia</p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── PWA INSTALL BANNER ── */}
      <AnimatePresence>
        {showPwaBanner && (
          <motion.div
            initial={{ y: 100 }}
            animate={{ y: 0 }}
            exit={{ y: 100 }}
            className="pwa-install-banner"
          >
            <div className="flex items-center gap-3">
              <Download className="w-5 h-5 text-military-green shrink-0" />
              <span>Install AGRO dashboard as an app for offline access</span>
            </div>
            <div className="flex items-center gap-3 shrink-0">
              <button
                onClick={handleInstall}
                className="bg-military-green text-military-deep px-4 py-2 font-black text-[10px] uppercase tracking-widest hover:brightness-110 active:scale-95 transition-all"
              >
                INSTALL
              </button>
              <button onClick={() => setShowPwaBanner(false)} className="text-white/50 hover:text-white">
                <X className="w-5 h-5" />
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── MODAL ── */}
      <CreateGridModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        onSubmit={handleCreateGrid}
        isConnected={isWalletConnected}
        isDeploying={isDeploying}
        isWaiting={isWaitingForReceipt}
      />

      {/* ── FOOTER ── */}
      <footer className="border-t-4 border-military-deep bg-white mt-auto">
        <div className="max-w-[1400px] mx-auto px-4 md:px-8 py-8 grid grid-cols-2 md:grid-cols-4 gap-6">
          {[
            { title: 'SYSTEM', items: ['Status: NOMINAL', 'Network: CELO SEPOLIA', 'Version: V2.1.4', 'Mode: ATR-ADAPTIVE'] },
            { title: 'SIGNALS', items: ['ATR Period: 14', 'CB Ratio: 0.786', 'Grid Modes: 3', 'Min Profit: $0.50'] },
            { title: 'SOURCES', items: ['◆ Binance', '◆ Gate.io', '◆ MEXC', '◆ CoinGecko'] },
            { title: 'LINKS', items: ['docs.celo.org', 'alfajores.celoscan.io', 'aaro.mil', 'MINIPAY_COMPAT'] },
          ].map(col => (
            <div key={col.title}>
              <p className="stat-label border-b border-border-military pb-2 mb-3">{col.title}</p>
              <ul className="space-y-1.5">
                {col.items.map(item => (
                  <li key={item} className="font-mono text-[11px] text-text-secondary uppercase tracking-wider">{item}</li>
                ))}
              </ul>
            </div>
          ))}
        </div>
        <div className="border-t border-border-military px-4 md:px-8 py-4 flex flex-col sm:flex-row items-center justify-between gap-2">
          <p className="font-mono text-[9px] text-text-secondary uppercase tracking-[0.3em] font-black">
            © 2026 CELO GRID KEEPER COMMAND — PURSUIT PROTOCOL — PROOF OF SHIP EDITION
          </p>
          <div className="flex items-center gap-2">
            <ShieldCheck className="w-4 h-4 text-military-green" />
            <span className="font-mono text-[9px] text-military-green uppercase tracking-widest font-black">MiniPay Compatible</span>
          </div>
        </div>
      </footer>
    </div>
  );
}
