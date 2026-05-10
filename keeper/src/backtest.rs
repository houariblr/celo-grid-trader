// ============================================================
//  backtest.rs  —  Fibonacci Circuit Breaker Backtesting Engine  V2
// ============================================================

use crate::atr::{atr_context, compute_atr, AtrContext, ATR_PERIOD};
use crate::grid::{compute_grid, compute_grid_auto, OrderSide, CIRCUIT_BREAKER_RATIO};
use crate::price_feed::OhlcData;
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use chrono::{DateTime, Utc}; // أضف Utc أيضاً لضمان التوافق
// حذفنا Deserialize غير المستخدمة هنا
use std::time::{Duration, Instant};
use tracing::info;

// ─────────────────────────────────────────────────────────────
//  Raw Binance kline data
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Candle {
    pub open_time_ms: u64,
    pub open:          f64,
    pub high:          f64,
    pub low:           f64,
    pub close:         f64,
    pub volume:        f64,
}

impl Candle {
    /// تحويل الطابع الزمني بالملي ثانية إلى تاريخ نصي دقيق (Y-M-D)
    pub fn date_str(&self) -> String {
        let secs = (self.open_time_ms / 1000) as i64;
        
        // استخدام Utc لضمان توافق التوقيت العالمي مع بيانات Binance
        let datetime: DateTime<Utc> = DateTime::from_timestamp(secs, 0)
            .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());

        datetime.format("%Y-%m-%d").to_string()
    }

    /// حساب True Range (TR) للشمعة الحالية مقارنة بإغلاق الشمعة السابقة
    /// أساسي لحساب الـ ATR الديناميكي للشبكة
    pub fn true_range(&self, prev_close: f64) -> f64 {
        let hl  = self.high - self.low;               // الفرق بين القمة والقاع
        let hpc = (self.high - prev_close).abs();     // الفرق بين القمة وإغلاق الأمس
        let lpc = (self.low  - prev_close).abs();     // الفرق بين القاع وإغلاق الأمس
        
        hl.max(hpc).max(lpc)
    }
}

// ─────────────────────────────────────────────────────────────
//  Backtest configuration
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub symbol:          String,
    pub interval:         String,
    pub total_candles:    usize,
    pub window_size:      usize,
    pub trade_size_usd:   f64,
    pub crash_threshold: f64,
    pub use_dynamic_grid: bool,
    pub local_peak_window: usize,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            symbol:            "CELOUSDT".to_string(),
            interval:          "4h".to_string(),
            total_candles:     500,
            window_size:       50,
            trade_size_usd:    100.0,
            crash_threshold:   15.0,
            use_dynamic_grid: true,
            local_peak_window: 14,
        }
    }
}

// ─────────────────────────────────────────────────────────────
//  Trade record & Grid Mode
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub candle_index:           usize,
    pub timestamp_ms:           u64,
    pub price:                  f64,
    pub side:                   OrderSide,
    pub fib_ratio:              f64,
    pub circuit_breaker_active: bool,
    pub blocked_by_cb:          bool,
    pub grid_mode:              GridMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GridMode {
    Static,
    Adaptive,
    Centered,
}

impl std::fmt::Display for GridMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static   => write!(f, "STATIC"),
            Self::Adaptive => write!(f, "ADAPTIVE"),
            Self::Centered => write!(f, "CENTERED"),
        }
    }
}

// ─────────────────────────────────────────────────────────────
//  Crash event & Result
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CrashEvent {
    pub start_index:           usize,
    pub end_index:             usize,
    pub start_date:            String,
    pub end_date:              String,
    pub peak_price:            f64,
    pub trough_price:          f64,
    pub drop_pct:              f64,
    pub buys_blocked:          usize,
    pub capital_preserved_usd: f64,
}

#[derive(Debug)]
pub struct BacktestResult {
    pub config:                  BacktestConfig,
    pub candles_analyzed:        usize,
    pub total_signals:           usize,
    pub buy_signals:             usize,
    pub sell_signals:            usize,
    pub executed_trades:         usize,
    pub blocked_by_cb:           usize,
    pub circuit_breaker_events:  usize,
    pub capital_preserved_usd:   f64,
    pub simulated_pnl_usd:       f64,
    pub max_price:               f64,
    pub min_price:               f64,
    pub max_drawdown_pct:        f64,
    pub crash_events:            Vec<CrashEvent>,
    pub all_trades:              Vec<TradeRecord>,
    pub static_candles:          usize,
    pub adaptive_candles:        usize,
    pub centered_candles:        usize,
}

// ─────────────────────────────────────────────────────────────
//  Binance kline fetcher
// ─────────────────────────────────────────────────────────────

pub async fn fetch_candles(
    client:   &Client,
    symbol:   &str,
    interval: &str,
    limit:    usize,
) -> Result<Vec<Candle>> {
    let url = format!(
        "https://api.binance.com/api/v3/klines?symbol={}&interval={}&limit={}",
        symbol, interval, limit.min(1000)
    );

    let raw: Vec<Vec<serde_json::Value>> = client
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .context("Binance klines request failed")?
        .json()
        .await
        .context("Binance klines JSON parse failed")?;

    if raw.is_empty() {
        return Err(anyhow!("Binance returned empty klines for {}", symbol));
    }

    let mut candles = Vec::with_capacity(raw.len());
    for row in &raw {
        let open_time = row.get(0).and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("missing open_time"))?;
        let parse = |idx: usize, name: &str| -> Result<f64> {
            row.get(idx)
               .and_then(|v| v.as_str())
               .ok_or_else(|| anyhow!("missing {}", name))?
               .parse::<f64>()
               .with_context(|| format!("parse {} failed", name))
        };
        candles.push(Candle {
            open_time_ms: open_time,
            open:          parse(1, "open")?,
            high:          parse(2, "high")?,
            low:           parse(3, "low")?,
            close:         parse(4, "close")?,
            volume:        parse(5, "volume")?,
        });
    }

    info!("📥 Fetched {} candles | {} {} | {} → {}",
        candles.len(), symbol, interval,
        candles.first().map(|c| c.date_str()).unwrap_or_default(),
        candles.last().map(|c| c.date_str()).unwrap_or_default(),
    );

    Ok(candles)
}

// ─────────────────────────────────────────────────────────────
//  Rolling-window OHLC (الإصلاح هنا: استخدام candles بدل window)
// ─────────────────────────────────────────────────────────────

fn rolling_ohlc(candles: &[Candle], start: usize, end: usize) -> OhlcData {
    let mut high = f64::NEG_INFINITY;
    let mut low  = f64::INFINITY;
    let slice = &candles[start..=end]; // نأخذ الجزء المطلوب
    
    for c in slice {
        if c.high > high { high = c.high; }
        if c.low  < low  { low  = c.low;  }
    }
    
    let last_atr = crate::atr::compute_atr(slice, 14).unwrap_or(0.0);
    OhlcData { 
        high, 
        low, 
        candles: slice.to_vec(), // تمرير الشموع المقطوعة كذاكرة
        last_atr: Some(last_atr),
        last_updated: Instant::now(), 
        candles_used: slice.len() 
    }
}

// ─────────────────────────────────────────────────────────────
//  Core backtest runner
// ─────────────────────────────────────────────────────────────

pub fn run_backtest(candles: &[Candle], cfg: &BacktestConfig) -> Result<BacktestResult> {
    if candles.len() < cfg.window_size + 1 {
        return Err(anyhow!("Insufficient data"));
    }

    let mut all_trades: Vec<TradeRecord> = Vec::new();
    let mut cb_events = 0;
    let mut last_cb_state = false;
    let mut open_buy_cost = 0.0;
    let mut open_buy_qty = 0.0;
    let mut realised_pnl = 0.0;
    let mut running_peak = candles[cfg.window_size - 1].close;
    let mut max_drawdown_pct = 0.0;
    let mut crash_events: Vec<CrashEvent> = Vec::new();
    let mut crash_start: Option<(usize, f64)> = None;

    let mut static_candles   = 0usize;
    let mut adaptive_candles  = 0usize;
    let mut centered_candles  = 0usize;

    for i in cfg.window_size..candles.len() {
        let c = &candles[i];
        let ref_price = candles[i - 1].close;
        let win_start = i.saturating_sub(cfg.window_size - 1);
        
        // جلب بيانات OHLC مع الذاكرة التاريخية للـ ATR
        let ohlc = rolling_ohlc(candles, win_start, i - 1);

        let atr_start = i.saturating_sub(ATR_PERIOD + 2);
        let atr_candles = &candles[atr_start..i];
        let atr_ctx_opt: Option<AtrContext> = if atr_candles.len() > ATR_PERIOD {
            compute_atr(atr_candles, ATR_PERIOD).map(|atr| atr_context(atr, ref_price))
        } else {
            None
        };

        let grid_result = if cfg.use_dynamic_grid {
            compute_grid_auto(&ohlc, ref_price, atr_ctx_opt.as_ref())
        } else {
            compute_grid(&ohlc, ref_price)
        };

        match (grid_result.is_centered, atr_ctx_opt.is_some()) {
            (true, _)     => centered_candles  += 1,
            (false, true) => adaptive_candles  += 1,
            (false, false)=> static_candles    += 1,
        }

        let local_start = i.saturating_sub(cfg.local_peak_window);
        let local_high = candles[local_start..i].iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);

        // تم إضافة "_" لتجنب تحذير المتغير غير المستخدم
        let _local_cb_triggered = c.low < local_high * (1.0 - (1.0 - CIRCUIT_BREAKER_RATIO));
        let smart_cb_triggered = c.low < local_high * CIRCUIT_BREAKER_RATIO;
        let cb_triggered = grid_result.circuit_breaker_triggered || smart_cb_triggered;

        if cb_triggered && !last_cb_state { cb_events += 1; }
        last_cb_state = cb_triggered;

        let current_mode = if grid_result.is_centered {
            GridMode::Centered
        } else if atr_ctx_opt.is_some() {
            GridMode::Adaptive
        } else {
            GridMode::Static
        };

        for level in &grid_result.levels {
            let buy_triggered  = level.side == OrderSide::Buy && c.low <= level.price && level.price <= c.open.max(candles[i-1].close);
            let sell_triggered = level.side == OrderSide::Sell && c.high >= level.price && level.price >= c.open.min(candles[i-1].close);

            if !buy_triggered && !sell_triggered { continue; }

            let blocked = cb_triggered && level.side == OrderSide::Buy;

            all_trades.push(TradeRecord {
                candle_index: i,
                timestamp_ms: c.open_time_ms,
                price: level.price,
                side: level.side.clone(),
                fib_ratio: level.ratio,
                circuit_breaker_active: cb_triggered,
                blocked_by_cb: blocked,
                grid_mode: current_mode.clone(),
            });

            if !blocked {
                match level.side {
                    OrderSide::Buy => {
                        let qty = cfg.trade_size_usd / level.price;
                        open_buy_cost += cfg.trade_size_usd;
                        open_buy_qty  += qty;
                    }
                    OrderSide::Sell => {
                        if open_buy_qty > 1e-9 {
                            let sell_qty   = open_buy_qty.min(cfg.trade_size_usd / level.price);
                            let cost_basis = (sell_qty / open_buy_qty) * open_buy_cost;
                            realised_pnl  += (sell_qty * level.price) - cost_basis;
                            open_buy_qty  -= sell_qty;
                            open_buy_cost -= cost_basis;
                        }
                    }
                    _ => {}
                }
            }
        }

        if c.high > running_peak { running_peak = c.high; }
        let dd = (running_peak - c.low) / running_peak * 100.0;
        if dd > max_drawdown_pct { max_drawdown_pct = dd; }

        let drop_from_peak = (running_peak - c.low) / running_peak * 100.0;
        if drop_from_peak >= cfg.crash_threshold {
            if crash_start.is_none() { crash_start = Some((i, running_peak)); }
        } else if let Some((start_idx, peak)) = crash_start.take() {
            let trough = candles[start_idx..=i].iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
            let buys_blocked = all_trades.iter().filter(|t| t.candle_index >= start_idx && t.candle_index <= i && t.blocked_by_cb).count();
            crash_events.push(CrashEvent {
                start_index: start_idx, end_index: i,
                start_date: candles[start_idx].date_str(), end_date: candles[i].date_str(),
                peak_price: peak, trough_price: trough, drop_pct: (peak - trough) / peak * 100.0,
                buys_blocked, capital_preserved_usd: buys_blocked as f64 * cfg.trade_size_usd,
            });
        }
    }

    // إحصاءات نهائية
    let total_signals = all_trades.len();
    let blocked_by_cb = all_trades.iter().filter(|t| t.blocked_by_cb).count();
    let last_price = candles.last().map(|c| c.close).unwrap_or(0.0);
    let total_pnl = realised_pnl + (open_buy_qty * last_price - open_buy_cost);

    Ok(BacktestResult {
        config: cfg.clone(),
        candles_analyzed: candles.len() - cfg.window_size,
        total_signals,
        buy_signals: all_trades.iter().filter(|t| t.side == OrderSide::Buy).count(),
        sell_signals: all_trades.iter().filter(|t| t.side == OrderSide::Sell).count(),
        executed_trades: total_signals - blocked_by_cb,
        blocked_by_cb,
        circuit_breaker_events: cb_events,
        capital_preserved_usd: blocked_by_cb as f64 * cfg.trade_size_usd,
        simulated_pnl_usd: total_pnl,
        max_price: candles.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max),
        min_price: candles.iter().map(|c| c.low).fold(f64::INFINITY, f64::min),
        max_drawdown_pct,
        crash_events,
        all_trades,
        static_candles,
        adaptive_candles,
        centered_candles,
    })
}

// ─────────────────────────────────────────────────────────────
//  Terminal report printer
// ─────────────────────────────────────────────────────────────

pub fn print_report(r: &BacktestResult) {
    let sep = "═".repeat(62);
    let sep2 = "─".repeat(62);

    println!("\n{}", sep);
    println!("  🦀 CELO GRID KEEPER V2 — BACKTEST REPORT");
    println!("  Fibonacci Dynamic Grid + Circuit Breaker Analysis");
    println!("{}", sep);

    println!("\n📋 CONFIGURATION");
    println!("{}", sep2);
    println!("  Symbol          : {}", r.config.symbol);
    println!("  Interval        : {}", r.config.interval);
    println!("  Window size     : {} candles", r.config.window_size);
    println!("  Trade size      : ${:.0}", r.config.trade_size_usd);
    println!("  Dynamic grid    : {}", if r.config.use_dynamic_grid { "✅ ENABLED" } else { "❌ DISABLED" });

    println!("\n📊 PERFORMANCE SUMMARY");
    println!("{}", sep2);
    println!("  Net P&L         : ${:.2}", r.simulated_pnl_usd);
    println!("  Max Drawdown    : {:.2}%", r.max_drawdown_pct);
    println!("  Capital Saved   : ${:.2}", r.capital_preserved_usd);
    println!("  CB Activations  : {}", r.circuit_breaker_events);
    println!("{}", sep);
}

