import { type Address } from 'viem';

// ── Contract ───────────────────────────────────────────────────────────────
export const GRID_CONTRACT_ADDRESS =
  '0xA4d8b9018B18511e5Bbb64d2FEbFCD28537BCe46' as Address;

// ── Tokens (Celo Sepolia) ──────────────────────────────────────────────────
export const CELO_TOKEN_SEPOLIA =
  '0x471EcE3750Da237f93B8E2992157d39A130178f1' as Address;

export const CUSD_TOKEN_SEPOLIA =
  '0x765DE816845861e75A25fCA122bb6898B8B1282a' as Address;

// ── Chain ─────────────────────────────────────────────────────────────────
export const CELO_SEPOLIA_CHAIN_ID = 44787;

// ── GridTradingV2 ABI (subset used by the UI) ─────────────────────────────
export const GRID_ABI = [
  {
    type: 'constructor',
    inputs: [
      { name: '_keeper',       type: 'address', internalType: 'address' },
      { name: '_mentoExchange', type: 'address', internalType: 'address' },
      { name: '_priceFeed',    type: 'address', internalType: 'address' },
      { name: '_moolaPool',    type: 'address', internalType: 'address' },
      { name: '_feeRecipient', type: 'address', internalType: 'address' },
    ],
    stateMutability: 'nonpayable',
  },
  {
    type: 'function',
    name: 'createGrid',
    inputs: [
      { name: 'baseToken',     type: 'address',  internalType: 'address'  },
      { name: 'quoteToken',    type: 'address',  internalType: 'address'  },
      { name: 'lowerPrice',    type: 'uint256',  internalType: 'uint256'  },
      { name: 'upperPrice',    type: 'uint256',  internalType: 'uint256'  },
      { name: 'gridCount',     type: 'uint256',  internalType: 'uint256'  },
      { name: 'totalAmount',   type: 'uint256',  internalType: 'uint256'  },
      { name: 'yieldEnabled',  type: 'bool',     internalType: 'bool'     },
      { name: 'slippageBps',   type: 'uint256',  internalType: 'uint256'  },
    ],
    outputs: [{ name: 'gridId', type: 'uint256', internalType: 'uint256' }],
    stateMutability: 'nonpayable',
  },
  {
    type: 'function',
    name: 'getUserGrids',
    inputs: [{ name: 'user', type: 'address', internalType: 'address' }],
    outputs: [{ name: '', type: 'uint256[]', internalType: 'uint256[]' }],
    stateMutability: 'view',
  },
  {
    type: 'function',
    name: 'grids',
    inputs: [{ name: '', type: 'uint256', internalType: 'uint256' }],
    outputs: [
      { name: 'owner',         type: 'address', internalType: 'address' },
      { name: 'baseToken',     type: 'address', internalType: 'address' },
      { name: 'quoteToken',    type: 'address', internalType: 'address' },
      { name: 'lowerPrice',    type: 'uint256', internalType: 'uint256' },
      { name: 'upperPrice',    type: 'uint256', internalType: 'uint256' },
      { name: 'gridCount',     type: 'uint256', internalType: 'uint256' },
      { name: 'amountPerGrid', type: 'uint256', internalType: 'uint256' },
      { name: 'quoteBalance',  type: 'uint256', internalType: 'uint256' },
      { name: 'baseBalance',   type: 'uint256', internalType: 'uint256' },
      { name: 'active',        type: 'bool',    internalType: 'bool'    },
      { name: 'yieldEnabled',  type: 'bool',    internalType: 'bool'    },
      { name: 'slippageBps',   type: 'uint256', internalType: 'uint256' },
      { name: 'createdAt',     type: 'uint256', internalType: 'uint256' },
    ],
    stateMutability: 'view',
  },
  {
    type: 'function',
    name: 'nextGridId',
    inputs: [],
    outputs: [{ name: '', type: 'uint256', internalType: 'uint256' }],
    stateMutability: 'view',
  },
] as const;
