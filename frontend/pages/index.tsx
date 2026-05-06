import { useState, useEffect } from 'react';

const CONTRACT_ADDRESS = '0xA6e2d11127431A734B5062540b695397AE3dE10C';
const CELO_TOKEN = '0xF194afDf50B03e69Bd7D057c1Aa9e10c9954E4C9';
const CUSD_TOKEN = '0x874069Fa1Eb16D44d622F2e0Ca25eeA172369bC1';

const CREATE_GRID_SELECTOR = '0x5b5a8057';
const APPROVE_SELECTOR = '0x095ea7b3';

function pad(val: string): string {
  return val.replace('0x', '').padStart(64, '0');
}

function toWei(n: string): string {
  return (BigInt(Math.floor(parseFloat(n) * 1e18))).toString(16).padStart(64, '0');
}

function encodeApprove(spender: string, amount: string): string {
  return APPROVE_SELECTOR + pad(spender) + toWei(amount);
}

function encodeCreateGrid(
  lowerPrice: string,
  upperPrice: string,
  gridCount: string,
  amount: string
): string {
  return (
    CREATE_GRID_SELECTOR +
    pad(CELO_TOKEN) +
    pad(CUSD_TOKEN) +
    toWei(lowerPrice) +
    toWei(upperPrice) +
    BigInt(gridCount).toString(16).padStart(64, '0') +
    toWei(amount)
  );
}

export default function Home() {
  const [account, setAccount] = useState('');
  const [isMiniPay, setIsMiniPay] = useState(false);
  const [lowerPrice, setLowerPrice] = useState('');
  const [upperPrice, setUpperPrice] = useState('');
  const [gridCount, setGridCount] = useState('5');
  const [amount, setAmount] = useState('');
  const [txHash, setTxHash] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [status, setStatus] = useState('');

  useEffect(() => {
    const eth = (window as any).ethereum;
    if (!eth) return;
    if (eth.isMiniPay) {
      setIsMiniPay(true);
      eth.request({ method: 'eth_requestAccounts' }).then((accounts: string[]) => {
        setAccount(accounts[0]);
      });
    }
  }, []);

  const connect = async () => {
    const eth = (window as any).ethereum;
    if (!eth) return setError('No wallet found. Install MetaMask or use MiniPay.');
    try {
      const accounts = await eth.request({ method: 'eth_requestAccounts' });
      setAccount(accounts[0]);

      await eth.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: '0xaa044c' }],
      }).catch(() =>
        eth.request({
          method: 'wallet_addEthereumChain',
          params: [{
            chainId: '0xaa044c',
            chainName: 'Celo Sepolia',
            nativeCurrency: { name: 'CELO', symbol: 'CELO', decimals: 18 },
            rpcUrls: ['https://celo-sepolia.drpc.org'],
            blockExplorerUrls: ['https://celo-sepolia.blockscout.com'],
          }],
        })
      );
    } catch (e: any) {
      setError(e.message);
    }
  };

  const createGrid = async () => {
    if (!account || !lowerPrice || !upperPrice || !amount) return;
    if (parseFloat(lowerPrice) >= parseFloat(upperPrice)) {
      return setError('Lower price must be less than upper price');
    }
    setLoading(true);
    setError('');
    setTxHash('');

    try {
      const eth = (window as any).ethereum;

      // Step 1: approve cUSD
      setStatus('Step 1/2: Approving cUSD spend...');
      const approveTx = await eth.request({
        method: 'eth_sendTransaction',
        params: [{
          from: account,
          to: CUSD_TOKEN,
          data: encodeApprove(CONTRACT_ADDRESS, amount),
          gas: '0x186A0',
        }],
      });

      // انتظر تأكيد الـ approve
      setStatus('Waiting for approval confirmation...');
      await waitForTx(eth, approveTx);

      // Step 2: createGrid
      setStatus('Step 2/2: Creating grid on-chain...');
      const data = encodeCreateGrid(lowerPrice, upperPrice, gridCount, amount);
      const tx = await eth.request({
        method: 'eth_sendTransaction',
        params: [{
          from: account,
          to: CONTRACT_ADDRESS,
          data,
          gas: '0x7A120',
        }],
      });

      setTxHash(tx);
      setStatus('');
    } catch (e: any) {
      setError(e.message);
      setStatus('');
    } finally {
      setLoading(false);
    }
  };

  // انتظار تأكيد transaction
  async function waitForTx(eth: any, txHash: string): Promise<void> {
    for (let i = 0; i < 30; i++) {
      await new Promise(r => setTimeout(r, 2000));
      const receipt = await eth.request({
        method: 'eth_getTransactionReceipt',
        params: [txHash],
      });
      if (receipt) return;
    }
  }

  const levels = lowerPrice && upperPrice && parseInt(gridCount) > 1
    ? Array.from({ length: parseInt(gridCount) }, (_, i) => {
        const p = parseFloat(lowerPrice) +
          (parseFloat(upperPrice) - parseFloat(lowerPrice)) *
          (i / (parseInt(gridCount) - 1));
        return p.toFixed(3);
      })
    : [];

  const inputStyle = {
    width: '100%',
    background: '#000',
    border: '1px solid #333',
    borderRadius: 8,
    padding: '10px 12px',
    color: '#fff',
    fontSize: 14,
    boxSizing: 'border-box' as const,
    outline: 'none',
  };

  const labelStyle = {
    fontSize: 11,
    color: '#666',
    display: 'block',
    marginBottom: 6,
  };

  return (
    <main style={{
      minHeight: '100vh',
      background: '#0a0a0a',
      color: '#fff',
      fontFamily: 'monospace',
    }}>
      {/* Header */}
      <header style={{
        borderBottom: '1px solid #1a1a1a',
        padding: '16px 24px',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
      }}>
        <div>
          <div style={{ fontSize: 18, fontWeight: 'bold', color: '#FCFF52' }}>
            ⚡ GRID TRADER
          </div>
          <div style={{ fontSize: 11, color: '#444', marginTop: 2 }}>
            Celo · Automated On-Chain Trading
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          {isMiniPay && (
            <span style={{
              fontSize: 11,
              background: '#FCFF5220',
              color: '#FCFF52',
              padding: '4px 10px',
              borderRadius: 20,
              border: '1px solid #FCFF5240',
            }}>MiniPay ✓</span>
          )}
          {account ? (
            <span style={{
              fontSize: 12,
              color: '#aaa',
              background: '#111',
              padding: '6px 12px',
              borderRadius: 8,
              border: '1px solid #222',
            }}>
              {account.slice(0, 6)}...{account.slice(-4)}
            </span>
          ) : (
            <button onClick={connect} style={{
              background: '#FCFF52',
              color: '#000',
              border: 'none',
              padding: '8px 16px',
              borderRadius: 8,
              fontWeight: 'bold',
              cursor: 'pointer',
              fontSize: 13,
            }}>Connect</button>
          )}
        </div>
      </header>

      <div style={{ maxWidth: 480, margin: '0 auto', padding: '32px 24px' }}>

        {/* Create Grid Form */}
        <div style={{
          background: '#111',
          border: '1px solid #1e1e1e',
          borderRadius: 16,
          padding: 24,
          marginBottom: 20,
        }}>
          <h2 style={{ fontSize: 16, fontWeight: 'bold', margin: '0 0 20px' }}>
            Create Grid
          </h2>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
            <div>
              <label style={labelStyle}>Lower Price (USD)</label>
              <input
                type="number"
                value={lowerPrice}
                onChange={e => setLowerPrice(e.target.value)}
                placeholder="0.30"
                style={inputStyle}
              />
            </div>
            <div>
              <label style={labelStyle}>Upper Price (USD)</label>
              <input
                type="number"
                value={upperPrice}
                onChange={e => setUpperPrice(e.target.value)}
                placeholder="0.60"
                style={inputStyle}
              />
            </div>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 20 }}>
            <div>
              <label style={labelStyle}>Grid Levels</label>
              <select
                value={gridCount}
                onChange={e => setGridCount(e.target.value)}
                style={inputStyle}
              >
                {[3, 5, 10, 20].map(n => (
                  <option key={n} value={n}>{n} levels</option>
                ))}
              </select>
            </div>
            <div>
              <label style={labelStyle}>Amount (cUSD)</label>
              <input
                type="number"
                value={amount}
                onChange={e => setAmount(e.target.value)}
                placeholder="100"
                style={inputStyle}
              />
            </div>
          </div>

          {/* Grid Preview */}
          {levels.length > 0 && (
            <div style={{
              background: '#000',
              border: '1px solid #1a1a1a',
              borderRadius: 10,
              padding: 12,
              marginBottom: 16,
            }}>
              <div style={{ fontSize: 11, color: '#555', marginBottom: 8 }}>
                Grid Preview — {gridCount} levels between ${lowerPrice} and ${upperPrice}
              </div>
              <div style={{ display: 'flex', gap: 3, alignItems: 'flex-end', height: 60 }}>
                {levels.map((price, i) => (
                  <div key={i} style={{ flex: 1, textAlign: 'center' }}>
                    <div style={{
                      height: `${20 + (i / (levels.length - 1)) * 40}px`,
                      background: '#FCFF5225',
                      border: '1px solid #FCFF5245',
                      borderRadius: 3,
                      marginBottom: 4,
                    }} />
                    <div style={{ fontSize: 9, color: '#444' }}>${price}</div>
                  </div>
                ))}
              </div>
              {amount && (
                <div style={{ fontSize: 11, color: '#555', marginTop: 8 }}>
                  ~${(parseFloat(amount) / parseInt(gridCount)).toFixed(2)} cUSD per level
                </div>
              )}
            </div>
          )}

          {/* Status */}
          {status && (
            <div style={{
              background: '#FCFF5215',
              border: '1px solid #FCFF5230',
              borderRadius: 8,
              padding: '10px 12px',
              marginBottom: 12,
              fontSize: 12,
              color: '#FCFF52',
            }}>⏳ {status}</div>
          )}

          {/* Error */}
          {error && (
            <div style={{
              background: '#ff000015',
              border: '1px solid #ff000035',
              borderRadius: 8,
              padding: '10px 12px',
              marginBottom: 12,
              fontSize: 12,
              color: '#ff6666',
            }}>{error}</div>
          )}

          <button
            onClick={account ? createGrid : connect}
            disabled={loading || (!!account && (!lowerPrice || !upperPrice || !amount))}
            style={{
              width: '100%',
              background: loading ? '#1a1a1a' : '#FCFF52',
              color: loading ? '#555' : '#000',
              border: 'none',
              padding: '14px',
              borderRadius: 10,
              fontWeight: 'bold',
              fontSize: 14,
              cursor: loading ? 'not-allowed' : 'pointer',
              transition: 'all 0.2s',
            }}
          >
            {loading ? status || 'Processing...' : account ? 'Create Grid' : 'Connect Wallet'}
          </button>
        </div>

        {/* TX Success */}
        {txHash && (
          <div style={{
            background: '#00ff0010',
            border: '1px solid #00ff0025',
            borderRadius: 12,
            padding: 16,
          }}>
            <div style={{ fontSize: 13, color: '#00cc66', marginBottom: 8 }}>
              ✅ Grid Created Successfully!
            </div>
            <div style={{ fontSize: 11, color: '#555', marginBottom: 8 }}>
              Transaction Hash:
            </div>
            <a
              href={`https://celo-sepolia.blockscout.com/tx/${txHash}`}
              target="_blank"
              rel="noopener noreferrer"
              style={{
                fontSize: 11,
                color: '#00cc66',
                wordBreak: 'break-all',
                textDecoration: 'none',
              }}
            >
              {txHash} ↗
            </a>
          </div>
        )}

        {/* Info */}
        <div style={{
          marginTop: 20,
          padding: 16,
          background: '#111',
          border: '1px solid #1e1e1e',
          borderRadius: 12,
        }}>
          <div style={{ fontSize: 11, color: '#444', marginBottom: 8 }}>How it works</div>
          <div style={{ fontSize: 12, color: '#666', lineHeight: 1.8 }}>
            1. Deposit cUSD into the grid contract<br />
            2. Set your price range and number of levels<br />
            3. The keeper bot automatically buys CELO on dips and sells on rises<br />
            4. Profit from market volatility 24/7
          </div>
        </div>

      </div>
    </main>
  );
}
