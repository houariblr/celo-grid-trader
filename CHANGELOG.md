# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [2.1.4] — 2026-05-10 — PURSUIT Edition

### Added
- **PWA support**: `manifest.json`, `beforeinstallprompt` install banner, `apple-touch-icon`, Open Graph and Twitter Card meta tags in `index.html`
- **MiniPay auto-connect**: wallet connects automatically inside the Opera MiniPay browser on page load
- **Wrong network guard**: detects non-Sepolia wallet, one-click switch button
- **cUSD equivalent price**: shown in ticker and hero stats alongside CELO
- **PURSUIT dashboard theme**: classified government portal aesthetic with `Orbitron` + `Share Tech Mono` + `Courier Prime`
- **Keeper grid mode warnings**: logs when market price is above/below on-chain grid range
- `src/types.ts`: created — all component imports were broken without it
- `src/lib/utils.ts`: `shortAddr()`, `formatTime()`, `clamp()` helpers
- `src/lib/constants.ts`: consolidated at correct import path for `App.tsx`
- Tailwind v4 `@theme` color tokens for full PURSUIT palette (`cream`, `ink`, `cyan-agency`, `violet-agency`, `mint-agency`, `amber-agency`, `red-agency`, `border-agency`)
- `@keyframes ticker-scroll` added to CSS — ticker was frozen without it
- Mobile safe-area padding (`env(safe-area-inset-*)`) for notched devices
- 44px minimum touch targets on mobile (WCAG 2.5.5)
- Vite chunk splitting: separate vendor bundles for React, wagmi/viem, recharts, motion

### Fixed
- **[CRITICAL] `isMiniPay`**: was `!!window.ethereum` (matched every wallet) → now `!!window.ethereum?.isMiniPay`
- **[CRITICAL] `useMiniPay.createGrid`**: passed `value: totalAmount` to a non-payable ERC-20 contract → removed `value` field
- **[CRITICAL] `main_v2.rs` keeper loop**: `reqwest::Client` created per cycle (~720 fd leaks/hour) → single `Arc<Client>` in `KeeperState`
- **[CRITICAL] `chain_v2.get_active_grids`**: iterated `getUserGrids(keeper_address)` (missed all user grids) → iterates `0..nextGridId`
- **[HIGH] `CreateGridModal`**: only 6 args passed to 8-arg `createGrid` function → added `yieldEnabled`, `slippageBps`
- **[HIGH] `chain_v2.simulate_execution`**: `eth_call` had no `from` address → every simulation reverted on `onlyKeeper` → added `.from(keeper_address)`
- **[HIGH] `main_v2.run_keeper_cycle`**: triggered on Fibonacci float prices vs on-chain wei prices (completely different sets) → now uses on-chain level price
- **[HIGH] `StatsGrid`**: runtime crash when `rpc_current_url` was `undefined` → proper optional chaining + fallback
- **[LOW] `main_v2.rs`**: `last_execution` was `Arc<RwLock<Option<Instant>>>` → replaced with `AtomicU64` Unix seconds
- **[LOW] `price_feed.rs`**: duplicate `_deviation_warning` shadow variable → removed
- **[LOW] Ticker animation**: `animate-[ticker-scroll_...]` referenced a non-existent keyframe → added definition

---

## [1.0.0] — 2026-04-01 — Initial Release

### Added
- React + Vite dashboard with Wagmi v3 / Viem v2
- Celo Sepolia testnet support
- Rust keeper bot with Fibonacci grid engine
- ATR-14 adaptive grid with volatility regime detection
- Fibonacci circuit breaker at 0.786 ratio
- Multi-source price aggregation (Binance, Gate.io, MEXC, CoinGecko)
- WebSocket server broadcasting live events to dashboard
- `GridTradingV2.sol` with Mento Exchange + Moola yield
- Chainlink oracle price validation
- Chainlink Automation (`checkUpkeep` / `performUpkeep`)
- Pre-execution simulation via `eth_call`
- RPC health monitor with automatic failover
- Backtest engine for circuit breaker validation
