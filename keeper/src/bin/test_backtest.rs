/**
 * @project Celo Grid Keeper V2 - Backtest Engine
 * @license SPDX-License-Identifier: Apache-2.0
 */

use serde::{Deserialize, Serialize};

// ─── Data Structures ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub open_time_ms: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub symbol: String,
    pub interval: String,
    pub total_candles: usize,
    pub window_size: usize,
    pub trade_size_usd: f64,
    pub crash_threshold: f64,
}

#[derive(Debug, Default)]
pub struct BacktestResult {
    pub simulated_pnl_usd: f64,
    pub circuit_breaker_events: usize,
    pub blocked_by_cb: usize,
    pub capital_preserved_usd: f64,
    pub candles_analyzed: usize,
    pub crash_events: Vec<CrashDetail>,
}

#[derive(Debug, Clone)]
pub struct CrashDetail {
    pub peak_price: f64,
    pub trough_price: f64,
    pub drop_pct: f64,
}

// ─── Core Logic ───

pub fn run_backtest(candles: &[Candle], cfg: &BacktestConfig) -> Result<BacktestResult, String> {
    if candles.len() < cfg.window_size {
        return Err(format!("Insufficient candles: {} < {}", candles.len(), cfg.window_size));
    }

    let mut result = BacktestResult::default();
    
    // محاكاة بسيطة للمحرك - يمكنك تعديل المنطق هنا بناءً على كودك الفعلي
    for i in cfg.window_size..candles.len() {
        let window = &candles[i - cfg.window_size..i];
        let current_candle = &candles[i];
        
        // حساب أعلى وأدنى سعر في النافذة (Fibonacci Basis)
        let high = window.iter().map(|c| c.high).fold(f64::MIN, f64::max);
        let _low = window.iter().map(|c| c.low).fold(f64::MAX, f64::min);
        let drop = ((high - current_candle.low) / high) * 100.0;

        // Circuit Breaker Logic
        if drop > cfg.crash_threshold {
            result.circuit_breaker_events += 1;
            result.blocked_by_cb += 1; // محاكاة منع عملية شراء
            result.capital_preserved_usd += cfg.trade_size_usd;
            result.crash_events.push(CrashDetail {
                peak_price: high,
                trough_price: current_candle.low,
                drop_pct: drop,
            });
        }
        
        result.candles_analyzed += 1;
    }

    Ok(result)
}

// ─── Unit Tests ───

#[cfg(test)]
mod tests {
    use super::*; // لاستيراد كل ما هو موجود في الملف العلوي

    fn make_candle(open_time_ms: u64, open: f64, high: f64, low: f64, close: f64) -> Candle {
        Candle { open_time_ms, open, high, low, close, volume: 1000.0 }
    }

    fn flat_candles(n: usize, price: f64) -> Vec<Candle> {
        (0..n)
            .map(|i| make_candle(i as u64 * 86_400_000, price, price * 1.001, price * 0.999, price))
            .collect()
    }

    fn crash_candles() -> Vec<Candle> {
        let mut candles = Vec::new();
        for i in 0..60 {
            candles.push(make_candle(i as u64 * 86_400_000, 1.0, 1.02, 0.98, 1.0));
        }
        for i in 0..20 {
            let price = 1.0 - (0.82 / 20.0) * (i + 1) as f64;
            candles.push(make_candle((60 + i) as u64 * 86_400_000, price + 0.01, price + 0.02, price - 0.01, price));
        }
        for i in 0..20 {
            let price = 0.18 + (0.42 / 20.0) * (i + 1) as f64;
            candles.push(make_candle((80 + i) as u64 * 86_400_000, price - 0.01, price + 0.02, price - 0.01, price));
        }
        candles
    }

    fn default_cfg() -> BacktestConfig {
        BacktestConfig {
            symbol: "CELOUSDT".to_string(),
            interval: "1d".to_string(),
            total_candles: 100,
            window_size: 50,
            trade_size_usd: 100.0,
            crash_threshold: 15.0,
        }
    }

    #[test]
    fn test_needs_enough_candles() {
        let candles = flat_candles(30, 1.0);
        let cfg = BacktestConfig { window_size: 50, ..default_cfg() };
        let result = run_backtest(&candles, &cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_crash_triggers_circuit_breaker() {
        let candles = crash_candles();
        let result = run_backtest(&candles, &default_cfg()).unwrap();
        assert!(result.circuit_breaker_events > 0);
        assert!(result.blocked_by_cb > 0);
        assert!(result.capital_preserved_usd > 0.0);
    }

    #[test]
    fn test_capital_preserved_math() {
        let candles = crash_candles();
        let cfg = BacktestConfig { trade_size_usd: 200.0, ..default_cfg() };
        let result = run_backtest(&candles, &cfg).unwrap();
        let expected = result.blocked_by_cb as f64 * 200.0;
        assert!((result.capital_preserved_usd - expected).abs() < 1e-9);
    }

    #[test]
    fn test_candles_analyzed_count() {
        let candles = flat_candles(100, 1.0);
        let result = run_backtest(&candles, &default_cfg()).unwrap();
        assert_eq!(result.candles_analyzed, 100 - 50);
    }
}

fn main() {
    println!("Running Backtest Engine standalone...");
    // يمكنك إضافة كود هنا لتشغيل اختبار واحد سريع إذا أردت
    println!("Use 'cargo test' to run the full verification suite.");
}
