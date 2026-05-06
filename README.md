# ⚡ Celo Grid Trader
> Automated on-chain grid trading for Celo — built for **Proof of Ship** (May 4–29, 2026)

**Live Demo:** https://celo-grid-trader-c3bh1ec89-houarispd-5339s-projects.vercel.app  
**Contract:** `0xA6e2d11127431A734B5062540b695397AE3dE10C` (Celo Sepolia)  
**Chain:** Celo Sepolia · Chain ID `11142220`

---

## What is it?

A fully on-chain grid trading system for Celo. Users deposit cUSD, define a price range, and the system automatically buys CELO on every dip and sells on every rise — no human intervention needed. The interface runs inside MiniPay, Celo's mobile wallet targeting African markets.

---

## Why it stands out

| Feature | Others | This project |
|---------|--------|-------------|
| Grid logic | Off-chain (CEX bots) | **On-chain smart contract** |
| Execution engine | Python/JS scripts | **Rust keeper (alloy 2.0)** |
| Mobile support | Desktop only | **MiniPay-native** |
| Dependencies | Heavy SDKs | **Zero — raw window.ethereum** |

---

## Project Structure

```
celo-grid-trader/
│
├── contract/                          # Solidity Smart Contract
│   ├── src/
│   │   ├── GridTrading.sol            # Main contract
│   │   └── interfaces/
│   │       ├── IERC20.sol             # ERC20 interface
│   │       └── IMento.sol             # Mento DEX interface
│   ├── test/
│   │   └── GridTrading.t.sol          # 5/5 tests passing
│   ├── script/
│   │   └── Deploy.s.sol               # Deployment script
│   └── foundry.toml                   # Foundry config
│
├── keeper/                            # Rust Execution Bot
│   ├── src/
│   │   ├── main.rs                    # Main loop
│   │   ├── config.rs                  # Env config loader
│   │   ├── price_feed.rs              # Binance API price feed
│   │   ├── grid.rs                    # Grid level logic
│   │   └── chain.rs                   # On-chain calls via alloy
│   ├── Cargo.toml                     # alloy 2.0.4 + tokio
│   └── .env                           # RPC, keys, contract address
│
└── frontend/                          # MiniPay Web Interface
    ├── pages/
    │   └── index.tsx                  # Main UI (zero external deps)
    ├── hooks/
    │   └── useMiniPay.ts              # MiniPay detection hook
    ├── package.json                   # next + react only
    └── next.config.js
```

---

## System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      USER                               │
│                   (MiniPay App)                         │
└────────────────────────┬────────────────────────────────┘
                         │  createGrid(lower, upper, levels, cUSD)
                         │  closeGrid(gridId)
                         ▼
┌─────────────────────────────────────────────────────────┐
│               FRONTEND  (Vercel)                        │
│                                                         │
│   pages/index.tsx                                       │
│   ├── Connect wallet via window.ethereum                │
│   ├── Detect MiniPay environment                        │
│   ├── Encode calldata manually (no SDK)                 │
│   ├── Grid preview visualization                        │
│   └── TX status + Blockscout link                       │
│                                                         │
│   hooks/useMiniPay.ts                                   │
│   └── Auto-connect when inside MiniPay                  │
└────────────────────────┬────────────────────────────────┘
                         │  eth_sendTransaction
                         ▼
┌─────────────────────────────────────────────────────────┐
│           SMART CONTRACT  (Celo Sepolia)                │
│      0xA6e2d11127431A734B5062540b695397AE3dE10C         │
│                                                         │
│   GridTrading.sol                                       │
│   ├── createGrid()   → stores grid + levels on-chain   │
│   ├── executeGrid()  → buy/sell via Mento DEX           │
│   └── closeGrid()    → returns funds to user            │
│                                                         │
│   Storage                                               │
│   ├── mapping gridId → Grid struct                      │
│   ├── mapping gridId → GridLevel[]                      │
│   └── mapping user   → gridId[]                         │
│                                                         │
│   Events                                               │
│   ├── GridCreated(gridId, owner, lower, upper, count)   │
│   ├── GridExecuted(gridId, level, isBuy, amount)        │
│   └── GridClosed(gridId, owner)                         │
└────────────────────────┬────────────────────────────────┘
                         │  swap cUSD ↔ CELO
                         ▼
┌─────────────────────────────────────────────────────────┐
│                  MENTO DEX                              │
│              (Celo Native Exchange)                     │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │  executeGrid(gridId, level, price)
                         │
┌─────────────────────────────────────────────────────────┐
│              RUST KEEPER BOT                            │
│                                                         │
│   main.rs  (tokio async loop every 5s)                  │
│   │                                                     │
│   ├── price_feed.rs                                     │
│   │   └── GET api.binance.com/CELOUSDT → f64            │
│   │                                                     │
│   ├── chain.rs  (alloy 2.0.4)                           │
│   │   ├── get_active_grids()                            │
│   │   │   └── read GridCreated events (last 100k blocks)│
│   │   │   └── fetch grid data + levels from contract    │
│   │   └── execute_grid()                                │
│   │       └── send executeGrid() tx on-chain            │
│   │                                                     │
│   └── grid.rs                                           │
│       └── find_executable_levels(grid, current_price)   │
│           ├── isBuy  && price <= level.price → BUY      │
│           └── isSell && price >= level.price → SELL     │
└─────────────────────────────────────────────────────────┘
```

---

## Data Flow

```
Every 5 seconds:

1. Keeper fetches CELO/USD price from Binance API
        │
        ▼
2. Keeper reads all GridCreated events from blockchain
        │
        ▼
3. For each active grid:
   → fetch grid levels from contract
   → compare current price to each level
        │
        ▼
4. If level is executable:
   → send executeGrid(gridId, levelIndex, price) tx
        │
        ▼
5. Contract executes swap on Mento DEX
   → BUY:  cUSD → CELO  (when price drops to level)
   → SELL: CELO → cUSD  (when price rises to level)
```

---

## Grid Logic Example

```
CELO price range: $0.30 → $0.60
Grid levels: 5
Amount: 100 cUSD (20 cUSD per level)

Level 0: $0.300  → BUY  20 cUSD worth of CELO
Level 1: $0.375  → BUY  20 cUSD worth of CELO
Level 2: $0.450  → BUY  20 cUSD worth of CELO
Level 3: $0.525  → BUY  20 cUSD worth of CELO
Level 4: $0.600  → BUY  20 cUSD worth of CELO

After buy at level 0 → level 0 becomes SELL at $0.300
Price rises to $0.300 → SELL → profit captured ✓
```

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Smart Contract | Solidity 0.8.20 + Foundry |
| Testing | Forge (5/5 tests) |
| Execution Bot | Rust + alloy 2.0.4 + tokio |
| Price Feed | Binance Public API |
| Frontend | Next.js 14 + React 18 |
| Wallet | window.ethereum (MiniPay native) |
| Hosting | Vercel |
| DEX | Mento (Celo native) |

---

## Deployment Info

| | Testnet | Mainnet |
|-|---------|---------|
| Chain ID | 11142220 | 42220 |
| RPC | https://celo-sepolia.drpc.org | https://forno.celo.org |
| Explorer | https://celo-sepolia.blockscout.com | https://celoscan.io |
| Contract | `0xA6e2d11...` | pending |

---

## Setup

### Contract
```bash
cd contract
forge build
forge test -vv
source .env && forge script script/Deploy.s.sol \
  --rpc-url https://celo-sepolia.drpc.org \
  --broadcast --legacy
```

### Keeper
```bash
cd keeper
cp .env.example .env   # fill in PRIVATE_KEY, CONTRACT_ADDRESS
cargo run --release
```

### Frontend
```bash
cd frontend
npm install
npm run dev            # localhost:3000
vercel --prod          # deploy
```

---

## Keeper .env
```bash
RPC_URL=https://celo-sepolia.drpc.org
WS_URL=wss://celo-sepolia.drpc.org
PRIVATE_KEY=0x...
CONTRACT_ADDRESS=0xA6e2d11127431A734B5062540b695397AE3dE10C
KEEPER_ADDRESS=0x0f9AF1a6B19bA30C881C97E3a6Cf54540a3C72Ba
POLL_INTERVAL_MS=5000
```

---

## Roadmap

- [x] Smart contract + 5 tests
- [x] Deploy on Celo Sepolia
- [x] Rust keeper bot (alloy 2.0)
- [x] MiniPay frontend
- [x] Deploy on Vercel
- [ ] Mainnet deployment
- [ ] Chainlink price feed (replace Binance API)
- [ ] WebSocket price feed (replace polling)
- [ ] Submit on Proof of Ship before May 29
