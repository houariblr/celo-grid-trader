// ============================================================
//  price_feed.rs  –  Multi-Source Price Aggregator
//  Sources: Binance → Gate.io → MEXC → CoinGecko (fallback chain)
//  Median aggregation for manipulation resistance
//
//  FIXES (v2.1):
//    [CRITICAL] Duplicate `_deviation_warning` variable removed
//    [PERF]     ATR cached at fetch time — no recompute per cycle
//    [PERF]     Shared reqwest::Client passed into ohlc_updater_task
//               instead of building a new pool on every loop iteration
// ============================================================

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::warn;

use crate::backtest::Candle;

// ─────────────────────────────────────────────────────────────
//  OhlcData
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OhlcData {
    pub high:         f64,
    pub low:          f64,
    pub candles:      Vec<Candle>,
    /// FIX [PERF]: ATR is computed once when the candles are fetched and
    /// cached here.  Callers just read `last_atr` — no repeated O(n) work
    /// inside the hot keeper-cycle path.
    pub last_atr:     Option<f64>,
    pub last_updated: Instant,
    pub candles_used: usize,
}

impl OhlcData {
    /// Construct a placeholder used before the first real fetch.
    /// ATR is None because we have no candles yet.
    pub fn placeholder(high: f64, low: f64) -> Self {
        Self {
            high,
            low,
            candles:      Vec::new(),
            last_atr:     None,
            last_updated: Instant::now(),
            candles_used: 0,
        }
    }

    pub fn is_fresh(&self, max_age_secs: u64) -> bool {
        self.last_updated.elapsed() < Duration::from_secs(max_age_secs)
    }

    /// FIX [PERF]: ATR is now a direct field lookup — O(1), no allocation.
    /// The old implementation re-ran the full Wilder-smoothed ATR calculation
    /// (O(n) over all candles) on every keeper cycle read.
    #[inline]
    pub fn get_last_atr(&self) -> Option<f64> {
        self.last_atr
    }
}

pub type SharedOhlc = Arc<RwLock<OhlcData>>;

// ─────────────────────────────────────────────────────────────
//  AggregatedPrice
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AggregatedPrice {
    pub price:                   f64,
    pub sources_count:           usize,
    pub sources_used:            Vec<&'static str>,
    pub price_deviation_warning: bool,
}

// ─────────────────────────────────────────────────────────────
//  Per-source fetchers
// ─────────────────────────────────────────────────────────────

async fn fetch_binance(client: &Client, symbol: &str) -> Result<f64> {
    #[derive(Deserialize)]
    struct Resp { price: String }
    let url = format!("https://api.binance.com/api/v3/ticker/price?symbol={}", symbol);
    let resp: Resp = client.get(&url)
        .timeout(Duration::from_secs(6))
        .send().await?
        .json().await?;
    resp.price.parse::<f64>().context("Binance parse")
}

async fn fetch_gateio(client: &Client, symbol: &str) -> Result<f64> {
    #[derive(Deserialize)]
    struct Resp { last: String }
    let gateio_symbol = if symbol == "CELOUSDT" { "CELO_USDT" } else { symbol };
    let url = format!(
        "https://api.gateio.ws/api/v4/spot/tickers?currency_pair={}",
        gateio_symbol
    );
    let resp: Vec<Resp> = client.get(&url)
        .timeout(Duration::from_secs(6))
        .send().await?
        .json().await?;
    resp.into_iter()
        .next()
        .ok_or_else(|| anyhow!("Gate.io empty"))?
        .last
        .parse::<f64>()
        .context("Gate.io parse")
}

async fn fetch_mexc(client: &Client, symbol: &str) -> Result<f64> {
    #[derive(Deserialize)]
    struct Resp { price: String }
    let url = format!("https://api.mexc.com/api/v3/ticker/price?symbol={}", symbol);
    let resp: Resp = client.get(&url)
        .timeout(Duration::from_secs(6))
        .send().await?
        .json().await?;
    resp.price.parse::<f64>().context("MEXC parse")
}

async fn fetch_coingecko(client: &Client) -> Result<f64> {
    #[derive(Deserialize)]
    struct Inner { usd: f64 }
    #[derive(Deserialize)]
    struct Resp { celo: Inner }
    let resp: Resp = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=celo&vs_currencies=usd")
        .timeout(Duration::from_secs(10))
        .send().await?
        .json().await?;
    Ok(resp.celo.usd)
}

// ─────────────────────────────────────────────────────────────
//  Median aggregator
// ─────────────────────────────────────────────────────────────

pub async fn fetch_aggregated_price(
    client: &Client,
    symbol: &str,
) -> Result<AggregatedPrice> {
    let (b, g, m, c) = tokio::join!(
        fetch_binance(client, symbol),
        fetch_gateio(client, symbol),
        fetch_mexc(client, symbol),
        fetch_coingecko(client),
    );

    let mut prices: Vec<(f64, &'static str)> = Vec::new();
    if let Ok(p) = b { prices.push((p, "Binance")); }
    if let Ok(p) = g { prices.push((p, "Gate.io")); }
    if let Ok(p) = m { prices.push((p, "MEXC")); }
    if let Ok(p) = c { prices.push((p, "CoinGecko")); }

    if prices.is_empty() {
        return Err(anyhow!("All price sources failed"));
    }

    prices.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let n = prices.len();
    let median = if n % 2 == 0 {
        (prices[n / 2 - 1].0 + prices[n / 2].0) / 2.0
    } else {
        prices[n / 2].0
    };

    let min_p = prices.first().unwrap().0;
    let max_p = prices.last().unwrap().0;

    // FIX [LOW]: removed the dead `_deviation_warning` shadow that was
    // computed and immediately discarded on the line before this one.
    let deviation_warning = (max_p - min_p) / min_p > 0.01;

    Ok(AggregatedPrice {
        price:                   median,
        sources_count:           n,
        sources_used:            prices.into_iter().map(|(_, name)| name).collect(),
        price_deviation_warning: deviation_warning,
    })
}

// ─────────────────────────────────────────────────────────────
//  Klines  (OHLC candle window for ATR)
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct KlineParams {
    pub symbol:   String,
    pub interval: String,
    pub limit:    u32,
}

#[derive(Debug, Deserialize)]
struct RawKline(Vec<serde_json::Value>);

impl RawKline {
    fn to_candle(&self) -> Result<Candle> {
        Ok(Candle {
            open_time_ms: self.0.get(0).and_then(|v| v.as_u64()).unwrap_or(0),
            open:         parse_field(&self.0, 1, "open")?,
            high:         parse_field(&self.0, 2, "high")?,
            low:          parse_field(&self.0, 3, "low")?,
            close:        parse_field(&self.0, 4, "close")?,
            volume:       parse_field(&self.0, 5, "volume")?,
        })
    }
}

fn parse_field(arr: &[serde_json::Value], idx: usize, name: &str) -> Result<f64> {
    arr.get(idx)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Field {} missing", name))?
        .parse::<f64>()
        .with_context(|| format!("Parse {} failed", name))
}

/// Fetch OHLC candles and build an OhlcData with the ATR pre-computed.
///
/// FIX [PERF]: ATR is computed here, once, when data arrives.  Every
/// subsequent read via `get_last_atr()` is a free field access with no
/// allocation or iteration.
pub async fn fetch_ohlc(client: &Client, params: &KlineParams) -> Result<OhlcData> {
    let url = format!(
        "https://api.binance.com/api/v3/klines?symbol={}&interval={}&limit={}",
        params.symbol, params.interval, params.limit
    );

    let raw: Vec<RawKline> = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send().await?
        .json().await?;

    if raw.is_empty() {
        return Err(anyhow!("Empty klines response"));
    }

    let mut candles      = Vec::with_capacity(raw.len());
    let mut global_high  = f64::NEG_INFINITY;
    let mut global_low   = f64::INFINITY;

    for r in &raw {
        let c = r.to_candle()?;
        if c.high > global_high { global_high = c.high; }
        if c.low  < global_low  { global_low  = c.low;  }
        candles.push(c);
    }

    // Compute ATR once at fetch time and cache it in the struct.
    let last_atr = crate::atr::compute_atr(&candles, 14);

    Ok(OhlcData {
        high:         global_high,
        low:          global_low,
        candles,
        last_atr,
        last_updated: Instant::now(),
        candles_used: raw.len(),
    })
}

// ─────────────────────────────────────────────────────────────
//  Background updater task
// ─────────────────────────────────────────────────────────────

/// FIX [PERF]: accepts a shared `Arc<Client>` instead of constructing a new
/// client (and therefore a new connection pool + TLS context) on every spawn.
/// The caller owns one client and passes a clone of the Arc here.
pub async fn ohlc_updater_task(
    shared_ohlc:   SharedOhlc,
    params:        KlineParams,
    interval_secs: u64,
    max_retries:   u32,
    client:        Arc<Client>,
) {
    loop {
        let mut succeeded = false;

        for attempt in 1..=max_retries {
            match fetch_ohlc(&client, &params).await {
                Ok(data) => {
                    *shared_ohlc.write().await = data;
                    succeeded = true;
                    break;
                }
                Err(e) => {
                    warn!(attempt, error = %e, "OHLC fetch failed");
                    tokio::time::sleep(Duration::from_secs(2 * attempt as u64)).await;
                }
            }
        }

        if !succeeded {
            warn!(
                max_retries,
                "All OHLC fetch attempts exhausted — retrying after interval"
            );
        }

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}
