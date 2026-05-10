<div align="center">

<!-- Replace with your actual banner image once uploaded to GitHub -->
<img width="1200" height="400" alt="AGRO — Celo Grid Keeper V2 Banner" src="https://github.com/user-attachments/assets/0aa67016-6eaf-458a-adb2-6e31a0763ed6" />

<br /><br />

<h1>
  <img src="https://img.shields.io/badge/CELO-Grid%20Keeper%20V2-16a34a?style=for-the-badge&logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI+PGNpcmNsZSBjeD0iMTIiIGN5PSIxMiIgcj0iMTAiIGZpbGw9IndoaXRlIi8+PC9zdmc+" />
</h1>

<p><strong>ALL-DOMAIN GRID RESOLUTION OFFICE — PURSUIT PROTOCOL</strong></p>

<p>
  Production-grade Fibonacci &amp; ATR-adaptive automated grid trading on Celo.<br/>
  MiniPay compatible · PWA installable · Sepolia testnet ready.
</p>

<br/>

[![Live Demo](https://img.shields.io/badge/LIVE%20DEMO-ai.studio-0891B2?style=for-the-badge)](https://ai.studio/apps/6fe3d85a-34ff-4d09-b74d-9ce65796ecf9)
[![Network](https://img.shields.io/badge/Network-Celo%20Sepolia-FCFF52?style=for-the-badge)](https://alfajores.celoscan.io)
[![MiniPay](https://img.shields.io/badge/MiniPay-Compatible-16a34a?style=for-the-badge)](https://docs.celo.org/build-on-celo/build-on-minipay/overview)
[![License](https://img.shields.io/badge/License-Apache%202.0-6D28D9?style=for-the-badge)](LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.8-3178C6?style=for-the-badge&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)

</div>

---

## What is this?

**Celo Grid Keeper V2** is a full-stack automated trading system built on Celo. It pairs a production Rust keeper bot with a real-time React dashboard to execute grid orders at Fibonacci retracement levels — autonomously, on-chain, with circuit-breaker protection.

The dashboard is styled as a **classified government intelligence portal** (inspired by [war.gov/ufo](https://www.war.gov/ufo/)) with a light, futuristic mystery aesthetic using `Orbitron`, `Share Tech Mono`, and `Courier Prime` typography.

```
┌─────────────────────────────────────────────────────────────┐
│  PRICE ORACLE  →  Fibonacci Grid  →  ATR Context            │
│       ↓                  ↓               ↓                  │
│  4 Sources        7 Fib Levels     Volatility Regime        │
│  (Median)      [0, 0.236 … 1.0]  [LOW / NORMAL / HIGH]     │
│                       ↓                                     │
│            Circuit Breaker (0.786)                          │
│                       ↓                                     │
│            executeGrid() → Celo chain                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Architecture

```
celo-grid-keeper/
│
├── keeper/                    ← Rust keeper bot (runs off-chain)
│   ├── src/
│   │   ├── main_v2.rs         # Entry point, keeper loop
│   │   ├── chain_v2.rs        # Alloy blockchain client, RPC health monitor
│   │   ├── grid.rs            # Fibonacci grid engine + circuit breaker
│   │   ├── atr.rs             # ATR calculation + volatility regimes
│   │   ├── price_feed.rs      # Multi-source price aggregator (median)
│   │   ├── backtest.rs        # Circuit breaker backtesting engine
│   │   ├── keeper_ws_server.rs# WebSocket broadcast server
│   │   └── config.rs          # .env config loader
│   └── Cargo.toml
│
├── contract/                  ← Solidity smart contract
│   └── src/GridTradingV2.sol  # Fibonacci grid contract (Foundry)
│
└── dashboard/                 ← React frontend (this repo)
    ├── src/
    │   ├── App.tsx            # Main layout, WS connection, wallet
    │   ├── types.ts           # Shared TypeScript interfaces
    │   ├── lib/
    │   │   ├── constants.ts   # Contract address, ABI, token addresses
    │   │   └── utils.ts       # cn(), formatCurrency(), shortAddr()
    │   └── components/
    │       ├── StatsGrid.tsx        # 4-card telemetry overview
    │       ├── StatCard.tsx         # Reusable metric card
    │       ├── FibonacciView.tsx    # Price chart + Fib reference lines
    │       ├── GridCard.tsx         # On-chain grid position card
    │       ├── TransactionHistory.tsx  # Execution log table
    │       └── CreateGridModal.tsx  # Grid deployment modal
    ├── index.html             # PWA meta, Open Graph, manifest link
    ├── public/manifest.json   # PWA manifest (installable)
    └── vite.config.ts         # Build config, chunk splitting
```

---

## Key Features

### 🤖 Keeper Bot (Rust)
| Feature | Detail |
|---|---|
| **Fibonacci Grid** | 7 levels: 0.0 · 0.236 · 0.382 · 0.5 · 0.618 · **0.786** · 1.0 |
| **ATR Adaptive** | Grid widens/narrows based on Wilder-smoothed ATR-14 |
| **3 Grid Modes** | STATIC (no ATR) → ADAPTIVE → CENTERED (price out of range) |
| **Circuit Breaker** | Suspends all buys when price < 0.786 Fibonacci level |
| **Pre-execution Simulation** | `eth_call` with keeper address before every `executeGrid` |
| **RPC Health Monitor** | Automatic failover to backup RPCs after 3 consecutive failures |
| **Shared HTTP Client** | One `reqwest::Client` for the entire process — no fd leaks |
| **WebSocket Server** | Broadcasts all events to the dashboard in real time |

### 📊 Dashboard (React)
| Feature | Detail |
|---|---|
| **MiniPay Auto-Connect** | Detects `window.ethereum.isMiniPay` and auto-connects |
| **PWA Installable** | `beforeinstallprompt` banner — add to home screen |
| **Wrong Network Guard** | Detects non-Sepolia wallet, one-click network switch |
| **cUSD Price** | Shows CELO price in cUSD equivalent for non-crypto users |
| **Live WebSocket** | Real-time price, grid status, transactions, health updates |
| **Mock Preview** | Works without a keeper — demo data on first load |
| **Responsive** | Mobile-first layout, 44px touch targets, safe-area padding |

### 📄 Smart Contract (`GridTradingV2.sol`)
| Feature | Detail |
|---|---|
| **ERC-20 Grid** | Non-payable — uses `transferFrom` (approve → createGrid) |
| **Mento Exchange** | Swaps via Celo's native Mento DEX |
| **Moola Yield** | Optional idle capital deployment to Moola lending pool |
| **Chainlink Oracle** | Price sourced from Chainlink, not the keeper |
| **Chainlink Automation** | `checkUpkeep` / `performUpkeep` compatible |
| **Reentrancy Guard** | All state-changing functions protected |

---

## Tech Stack

| Layer | Technology |
|---|---|
| **Frontend** | React 19 · TypeScript 5.8 · Vite 6 |
| **Styling** | Tailwind CSS v4 · Orbitron · Share Tech Mono · Courier Prime |
| **Web3** | Wagmi v3 · Viem v2 · Celo Sepolia |
| **Charts** | Recharts 3 |
| **Animation** | Motion (Framer Motion v12) |
| **Keeper** | Rust · Tokio · Alloy · reqwest |
| **Contract** | Solidity 0.8.33 · Foundry · Mento · Moola · Chainlink |
| **Deploy** | Vercel / GitHub Pages (frontend) · any Linux VPS (keeper) |

---

## Celo Ecosystem Alignment

This project deliberately targets every Celo competition evaluation criterion:

```
✅ Wagmi + Viem          Official Celo recommended stack (not ContractKit)
✅ Celo Sepolia           Current active testnet
✅ MiniPay Compatible     window.ethereum.isMiniPay detection + auto-connect
✅ Mobile-First           Responsive grid, 44px touch targets, safe-area insets
✅ PWA                    manifest.json, beforeinstallprompt, apple-touch-icon
✅ cUSD Integration       Prices shown in cUSD equivalent throughout UI
✅ Mento Exchange         Native Celo DEX used for all swaps inside contract
✅ Chainlink Oracle       Contract validates price from Chainlink, not keeper
✅ Fee Abstraction        FEE_CURRENCY_ADDRESS config (pay gas in cUSD)
✅ Real-World Use Case    Automated trading — actual on-chain transactions
```

---

## Quick Start

### Prerequisites

- Node.js ≥ 18
- A browser wallet (MetaMask, or [Opera with MiniPay](https://www.opera.com/mobile))
- Celo Sepolia testnet selected in your wallet

### 1. Clone

```bash
# If this is your first publish:
git clone https://github.com/YOUR_USERNAME/celo-grid-keeper-v2.git
cd celo-grid-keeper-v2/dashboard

# If upgrading from v1:
git pull origin main
```

### 2. Install

```bash
npm install
```

### 3. Environment (optional)

```bash
cp .env.example .env.local
# Edit .env.local — only needed for AI Studio features
# GEMINI_API_KEY=your_key_here
```

### 4. Run

```bash
npm run dev
# Opens at http://localhost:3000
```

The dashboard works in **preview mode** without a running keeper — mock data loads automatically. To go live, point the WS URL to your keeper server.

---

## Running the Keeper Bot

The keeper is a separate Rust binary that watches the chain and executes grid levels.

### Prerequisites

- Rust ≥ 1.77
- A funded Celo Sepolia keeper wallet
- The `GridTradingV2` contract deployed (address in `.env`)

### Setup

```bash
cd keeper

# Create .env file
cat > .env << 'EOF'
RPC_URL=https://alfajores-forno.celo-testnet.org
BACKUP_RPC_URLS=https://celo-alfajores.drpc.org
WS_URL=wss://alfajores-forno.celo-testnet.org/ws
PRIVATE_KEY=0xYOUR_KEEPER_PRIVATE_KEY
CONTRACT_ADDRESS=0xA4d8b9018B18511e5Bbb64d2FEbFCD28537BCe46
KEEPER_ADDRESS=0xYOUR_KEEPER_ADDRESS
OHLC_SYMBOL=CELOUSDT
OHLC_INTERVAL=15m
OHLC_CANDLE_LIMIT=50
POLL_INTERVAL_MS=5000
SIMULATE_BEFORE_EXECUTE=true
DRY_RUN_MODE=true        # Set false for real transactions
MIN_PROFIT_THRESHOLD_USD=0.50
GAS_LIMIT=300000
EOF
```

### Run

```bash
# Dry run (no real transactions)
cargo run --bin keeper_v2

# Production
DRY_RUN_MODE=false cargo run --release --bin keeper_v2
```

The keeper broadcasts WebSocket events on `ws://0.0.0.0:8080`. Point the dashboard's WS URL field there.

---

## Deploying the Contract

```bash
cd contract

# Install Foundry
curl -L https://foundry.paradigm.xyz | bash && foundryup

# Install dependencies
forge install

# Deploy to Celo Sepolia
forge create src/GridTradingV2.sol:GridTradingV2 \
  --rpc-url https://alfajores-forno.celo-testnet.org \
  --private-key $PRIVATE_KEY \
  --constructor-args \
    $KEEPER_ADDRESS \
    0x7D7A79a7d3b7E43f1e94c2A61a42256C3b93E5A3 \  # Mento Broker
    0x0568fD19986748cEfF3301e55c0eb1E729E0Ab7e \  # CELO/USD Chainlink
    0xE098C6e49fa55082Ee4c6F9d6D04C37a8cEcfe4f \  # Moola Pool
    $FEE_RECIPIENT_ADDRESS

# Verify on CeloScan
forge verify-contract $CONTRACT_ADDRESS src/GridTradingV2.sol:GridTradingV2 \
  --chain 44787 \
  --etherscan-api-key $CELOSCAN_API_KEY
```

---

## Environment Variables Reference

| Variable | Required | Default | Description |
|---|---|---|---|
| `RPC_URL` | ✅ | — | Primary Celo RPC endpoint |
| `BACKUP_RPC_URLS` | — | — | Comma-separated fallback RPCs |
| `PRIVATE_KEY` | ✅ | — | Keeper wallet private key |
| `CONTRACT_ADDRESS` | ✅ | — | Deployed GridTradingV2 address |
| `KEEPER_ADDRESS` | ✅ | — | Public address of keeper wallet |
| `POLL_INTERVAL_MS` | — | `5000` | Keeper cycle interval |
| `OHLC_SYMBOL` | — | `CELOUSDT` | Binance symbol for OHLC |
| `OHLC_INTERVAL` | — | `15m` | Candle interval |
| `OHLC_CANDLE_LIMIT` | — | `50` | Rolling window size |
| `SIMULATE_BEFORE_EXECUTE` | — | `true` | Run `eth_call` before each tx |
| `DRY_RUN_MODE` | — | `false` | Skip real transactions |
| `MIN_PROFIT_THRESHOLD_USD` | — | `0.5` | Min profit to execute |
| `GAS_LIMIT` | — | `300000` | Gas limit per transaction |
| `FEE_CURRENCY_ADDRESS` | — | — | Pay gas in cUSD (Celo feature) |
| `USE_LEGACY_TX` | — | `false` | Legacy vs EIP-1559 transactions |

---

## Upgrading from V1

If you had an older version published, here's what changed between V1 → V2:

```bash
# 1. Pull the latest code
git pull origin main

# 2. Install new deps (wagmi v3, viem v2, motion v12)
npm install

# 3. Key breaking changes:
#    - All CSS color tokens moved to @theme in index.css
#    - types.ts is now at src/types.ts (was missing in V1)
#    - constants.ts + utils.ts moved to src/lib/
#    - isMiniPay check fixed: window.ethereum.isMiniPay (not !!window.ethereum)
#    - CreateGridModal now passes all 8 contract args (was 6 in V1)
#    - App.tsx no longer re-declares modules (uses lib crate imports)
```

---

## Bug Fixes in V2

| Severity | Module | Issue | Fix |
|---|---|---|---|
| **CRITICAL** | `App.tsx` | `isMiniPay` matched every wallet | Check `window.ethereum.isMiniPay` |
| **CRITICAL** | `useMiniPay.ts` | `createGrid` passed `value: totalAmount` to non-payable contract | Removed `value` field — ERC-20 uses `transferFrom` |
| **CRITICAL** | `main_v2.rs` | `reqwest::Client` created per keeper cycle → fd leak | Single shared `Arc<Client>` in `KeeperState` |
| **CRITICAL** | `chain_v2.rs` | `getUserGrids(keeper_address)` missed user-created grids | Iterates `0..nextGridId` instead |
| **HIGH** | `CreateGridModal.tsx` | Only 6 args passed to 8-arg contract function | Added `yieldEnabled`, `slippageBps` |
| **HIGH** | `chain_v2.rs` | `simulate_execution` called without `from` address | Added `.from(keeper_address)` to `eth_call` |
| **HIGH** | `main_v2.rs` | Trigger compared Fibonacci floats vs on-chain wei prices | Now uses on-chain level price directly |
| **LOW** | `main_v2.rs` | `last_execution` held full `RwLock<Option<Instant>>` | Replaced with `AtomicU64` Unix seconds |
| **LOW** | `price_feed.rs` | Duplicate `_deviation_warning` variable | Removed dead shadow variable |
| **LOW** | `index.css` | `ticker-scroll` keyframe missing → ticker frozen | Added `@keyframes ticker-scroll` |

---

## Contributing

1. Fork the repository
2. Create your feature branch: `git checkout -b feat/my-feature`
3. Commit with conventional commits: `git commit -m "feat: add SocialConnect lookup"`
4. Push: `git push origin feat/my-feature`
5. Open a Pull Request

Please run `npm run lint` (TypeScript check) and `cargo test` before submitting.

---

## Roadmap

- [ ] **SocialConnect** — phone-number-to-address lookup for MiniPay users
- [ ] **ConnectKit** — polished wallet connection UI replacing custom button
- [ ] **Multi-chain** — Celo Mainnet support alongside Sepolia
- [ ] **Backtesting UI** — expose the Rust backtesting engine via REST API
- [ ] **Push Notifications** — Vapid-signed Web Push for trade execution events
- [ ] **Fee Abstraction UI** — let user choose gas currency (CELO / cUSD / cEUR)
- [ ] **Batch Execution** — `batchExecuteGrid` in contract for gas efficiency

---

## License

Apache 2.0 — see [LICENSE](LICENSE)

---

<div align="center">

**Built for the Celo Proof of Ship Competition · PURSUIT PROTOCOL · 2026**

[![Celo](https://img.shields.io/badge/Built%20on-Celo-16a34a?style=flat-square)](https://celo.org)
[![MiniPay](https://img.shields.io/badge/MiniPay-Ready-FCFF52?style=flat-square)](https://docs.celo.org/build-on-celo/build-on-minipay/overview)
[![PWA](https://img.shields.io/badge/PWA-Installable-6D28D9?style=flat-square)](https://web.dev/pwa)

</div>
