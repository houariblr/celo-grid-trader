import { useState, useEffect } from 'react';

// ── V2 Contract addresses (Celo Sepolia with Mocks) ──
const CONTRACT_ADDRESS = '0x058c4D25d62378fEf00b2Fd9C8B4677E1956E120';
const CELO_TOKEN       = '0x797880C1997e60e4844C0A4d9833fE8e9668bDBa'; // MockCELO
const CUSD_TOKEN       = '0x2357e49215DaBc679297d36064d9732aFFe683B0'; // MockcUSD

// createGrid(address,address,uint256,uint256,uint256,uint256,bool,uint256)
const CREATE_GRID_SELECTOR = '0x9f5bc0e0';
const APPROVE_SELECTOR     = '0x095ea7b3';

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
  amount: string,
  yieldEnabled: boolean,
  slippageBps: number
): string {
  const boolPad = yieldEnabled ? '1'.padStart(64, '0') : '0'.padStart(64, '0');
  const slipPad = slippageBps.toString(16).padStart(64, '0');

  return (
    CREATE_GRID_SELECTOR +
    pad(CELO_TOKEN) +
    pad(CUSD_TOKEN) +
    toWei(lowerPrice) +
    toWei(upperPrice) +
    BigInt(gridCount).toString(16).padStart(64, '0') +
    toWei(amount) +
    boolPad +
    slipPad
  );
}

export default function Home() {
  const [account, setAccount]         = useState('');
  const [isMiniPay, setIsMiniPay]     = useState(false);
  const [lowerPrice, setLowerPrice]   = useState('');
  const [upperPrice, setUpperPrice]   = useState('');
  const [gridCount, setGridCount]     = useState('5');
  const [amount, setAmount]           = useState('');
  const [yieldEnabled, setYield]      = useState(false);
  const [slippage, setSlippage]       = useState('100'); // 1%
  const [txHash, setTxHash]           = useState('');
  const [loading, setLoading]         = useState(false);
  const [error, setError]             = useState('');
  const [status, setStatus]           = useState('');

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
    if (!eth) return setError('No wallet found.');
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
    } catch (e: any) { setError(e.message); }
  };

  const createGrid = async () => {
    if (!account || !lowerPrice || !upperPrice || !amount) return;
    if (parseFloat(lowerPrice) >= parseFloat(upperPrice))
      return setError('Lower price must be less than upper price');

    setLoading(true); setError(''); setTxHash('');
    try {
      const eth = (window as any).ethereum;

      // Step 1: mint test cUSD (MockERC20)
      setStatus('Step 1/3: Minting test cUSD...');
      const mintData = '0x40c10f19' + pad(account) + toWei(amount);
      await eth.request({
        method: 'eth_sendTransaction',
        params: [{ from: account, to: CUSD_TOKEN, data: mintData, gas: '0x186A0' }],
      });

      // Step 2: approve
      setStatus('Step 2/3: Approving cUSD...');
      const approveTx = await eth.request({
        method: 'eth_sendTransaction',
        params: [{
          from: account,
          to: CUSD_TOKEN,
          data: encodeApprove(CONTRACT_ADDRESS, amount),
          gas: '0x186A0',
        }],
      });
      setStatus('Waiting for approval...');
      await waitForTx(eth, approveTx);

      // Step 3: createGrid
      setStatus('Step 3/3: Creating grid on-chain...');
      const data = encodeCreateGrid(
        lowerPrice, upperPrice, gridCount, amount,
        yieldEnabled, parseInt(slippage)
      );
      const tx = await eth.request({
        method: 'eth_sendTransaction',
        params: [{ from: account, to: CONTRACT_ADDRESS, data, gas: '0xF4240' }],
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

  async function waitForTx(eth: any, hash: string): Promise<void> {
    for (let i = 0; i < 30; i++) {
      await new Promise(r => setTimeout(r, 2000));
      const receipt = await eth.request({ method: 'eth_getTransactionReceipt', params: [hash] });
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
    width: '100%', background: '#000', border: '1px solid #333',
    borderRadius: 8, padding: '10px 12px', color: '#fff',
    fontSize: 14, boxSizing: 'border-box' as const, outline: 'none',
  };
  const labelStyle = { fontSize: 11, color: '#666', display: 'block', marginBottom: 6 };

  return (
    <main style={{ minHeight: '100vh', background: '#0a0a0a', color: '#fff', fontFamily: 'monospace' }}>

      {/* Header */}
      <header style={{
        borderBottom: '1px solid #1a1a1a', padding: '16px 24px',
        display: 'flex', justifyContent: 'space-between', alignItems: 'center',
      }}>
        <div>
          <div style={{ fontSize: 18, fontWeight: 'bold', color: '#FCFF52' }}>⚡ GRID TRADER</div>
          <div style={{ fontSize: 11, color: '#444', marginTop: 2 }}>Celo · Automated On-Chain Trading</div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          {isMiniPay && (
            <span style={{
              fontSize: 11, background: '#FCFF5220', color: '#FCFF52',
              padding: '4px 10px', borderRadius: 20, border: '1px solid #FCFF5240',
            }}>MiniPay ✓</span>
          )}
          {account ? (
            <span style={{
              fontSize: 12, color: '#aaa', background: '#111',
              padding: '6px 12px', borderRadius: 8, border: '1px solid #222',
            }}>
              {account.slice(0, 6)}...{account.slice(-4)}
            </span>
          ) : (
            <button onClick={connect} style={{
              background: '#FCFF52', color: '#000', border: 'none',
              padding: '8px 16px', borderRadius: 8, fontWeight: 'bold',
              cursor: 'pointer', fontSize: 13,
            }}>Connect</button>
          )}
        </div>
      </header>

      <div style={{ maxWidth: 480, margin: '0 auto', padding: '32px 24px' }}>

        {/* Create Grid Form */}
        <div style={{ background: '#111', border: '1px solid #1e1e1e', borderRadius: 16, padding: 24, marginBottom: 20 }}>
          <h2 style={{ fontSize: 16, fontWeight: 'bold', margin: '0 0 20px' }}>Create Grid</h2>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
            <div>
              <label style={labelStyle}>Lower Price (USD)</label>
              <input type="number" value={lowerPrice} onChange={e => setLowerPrice(e.target.value)} placeholder="0.30" style={inputStyle} />
            </div>
            <div>
              <label style={labelStyle}>Upper Price (USD)</label>
              <input type="number" value={upperPrice} onChange={e => setUpperPrice(e.target.value)} placeholder="0.60" style={inputStyle} />
            </div>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
            <div>
              <label style={labelStyle}>Grid Levels</label>
              <select value={gridCount} onChange={e => setGridCount(e.target.value)} style={inputStyle}>
                {[3, 5, 10, 20].map(n => <option key={n} value={n}>{n} levels</option>)}
              </select>
            </div>
            <div>
              <label style={labelStyle}>Amount (cUSD)</label>
              <input type="number" value={amount} onChange={e => setAmount(e.target.value)} placeholder="100" style={inputStyle} />
            </div>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 20 }}>
            <div>
              <label style={labelStyle}>Slippage Tolerance</label>
              <select value={slippage} onChange={e => setSlippage(e.target.value)} style={inputStyle}>
                <option value="50">0.5%</option>
                <option value="100">1% (default)</option>
                <option value="200">2%</option>
                <option value="500">5%</option>
              </select>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, paddingTop: 22 }}>
              <input
                type="checkbox"
                id="yield"
                checked={yieldEnabled}
                onChange={e => setYield(e.target.checked)}
                style={{ width: 16, height: 16, cursor: 'pointer' }}
              />
              <label htmlFor="yield" style={{ ...labelStyle, marginBottom: 0, cursor: 'pointer' }}>
                Enable Moola Yield
              </label>
            </div>
          </div>

          {/* Grid Preview */}
          {levels.length > 0 && (
            <div style={{ background: '#000', border: '1px solid #1a1a1a', borderRadius: 10, padding: 12, marginBottom: 16 }}>
              <div style={{ fontSize: 11, color: '#555', marginBottom: 8 }}>
                Grid Preview — {gridCount} levels · ${lowerPrice} → ${upperPrice}
              </div>
              <div style={{ display: 'flex', gap: 3, alignItems: 'flex-end', height: 60 }}>
                {levels.map((price, i) => (
                  <div key={i} style={{ flex: 1, textAlign: 'center' }}>
                    <div style={{
                      height: `${20 + (i / (levels.length - 1)) * 40}px`,
                      background: '#FCFF5225', border: '1px solid #FCFF5245',
                      borderRadius: 3, marginBottom: 4,
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

          {status && (
            <div style={{
              background: '#FCFF5215', border: '1px solid #FCFF5230',
              borderRadius: 8, padding: '10px 12px', marginBottom: 12,
              fontSize: 12, color: '#FCFF52',
            }}>⏳ {status}</div>
          )}

          {error && (
            <div style={{
              background: '#ff000015', border: '1px solid #ff000035',
              borderRadius: 8, padding: '10px 12px', marginBottom: 12,
              fontSize: 12, color: '#ff6666',
            }}>{error}</div>
          )}

          <button
            onClick={account ? createGrid : connect}
            disabled={loading || (!!account && (!lowerPrice || !upperPrice || !amount))}
            style={{
              width: '100%',
              background: loading ? '#1a1a1a' : '#FCFF52',
              color: loading ? '#555' : '#000',
              border: 'none', padding: '14px', borderRadius: 10,
              fontWeight: 'bold', fontSize: 14,
              cursor: loading ? 'not-allowed' : 'pointer',
            }}
          >
            {loading ? status || 'Processing...' : account ? 'Create Grid' : 'Connect Wallet'}
          </button>
        </div>

        {/* TX Success */}
        {txHash && (
          <div style={{ background: '#00ff0010', border: '1px solid #00ff0025', borderRadius: 12, padding: 16 }}>
            <div style={{ fontSize: 13, color: '#00cc66', marginBottom: 8 }}>✅ Grid Created!</div>
            <a
              href={`https://celo-sepolia.blockscout.com/tx/${txHash}`}
              target="_blank" rel="noopener noreferrer"
              style={{ fontSize: 11, color: '#00cc66', wordBreak: 'break-all', textDecoration: 'none' }}
            >
              {txHash} ↗
            </a>
          </div>
        )}

        {/* Testnet Notice */}
        <div style={{ marginTop: 20, padding: 16, background: '#111', border: '1px solid #1e1e1e', borderRadius: 12 }}>
          <div style={{ fontSize: 11, color: '#FCFF52', marginBottom: 8 }}>⚠️ Testnet Mode</div>
          <div style={{ fontSize: 11, color: '#555', lineHeight: 1.8 }}>
            Running on Celo Sepolia with mock contracts.<br />
            cUSD is minted automatically for testing.<br />
            Oracle price: $0.45 (MockChainlink)
          </div>
        </div>

        {/* How it works */}
        <div style={{ marginTop: 12, padding: 16, background: '#111', border: '1px solid #1e1e1e', borderRadius: 12 }}>
          <div style={{ fontSize: 11, color: '#444', marginBottom: 8 }}>How it works</div>
          <div style={{ fontSize: 12, color: '#666', lineHeight: 1.8 }}>
            1. Deposit cUSD into the grid contract<br />
            2. Set your price range and grid levels<br />
            3. Keeper bot buys CELO on dips, sells on rises<br />
            4. Price sourced from Chainlink oracle (manipulation-proof)<br />
            5. Optional: earn yield on idle cUSD via Moola
          </div>
        </div>

      </div>
    </main>
  );
}
