// ============================================================
//  atr.rs  —  Average True Range Engine  (Enhanced V2)
//
//  الإضافات:
//    ✨ compute_dynamic_bounds() — حساب نطاق الشبكة المتمركزة
//    ✨ BASELINE_ATR_RATIO أكثر دقة لأسعار CELO المنخفضة
//    ✨ VolatilityRegime يأخذ قيمة السعر بعين الاعتبار
//
//  High ATR  →  wider grid  (avoid whipsaw fills)
//  Low ATR   →  tighter grid (capture more granular moves)
// ============================================================

use crate::backtest::Candle;
use tracing::debug;

// ── Constants ─────────────────────────────────────────────────

pub const ATR_PERIOD: usize = 14;

const MIN_MULTIPLIER: f64 = 0.70;
const MAX_MULTIPLIER: f64 = 2.00;

/// نسبة ATR/السعر "الطبيعية" — معايَرة على CELO
/// عند أسعار منخفضة (< $0.20) تكون النسبة أعلى طبيعياً
const BASELINE_ATR_RATIO: f64 = 0.035; // 3.5% من السعر

/// عدد ATRs للنطاق الكلي في الشبكة المتمركزة (يمين + يسار)
/// الشبكة = [current_price - N*ATR, current_price + N*ATR]
pub const CENTERED_GRID_ATR_MULTIPLIER: f64 = 2.5;

// ─────────────────────────────────────────────────────────────
//  Core ATR calculation
// ─────────────────────────────────────────────────────────────

#[inline]
pub fn true_range(high: f64, low: f64, prev_close: f64) -> f64 {
    let hl  = high - low;
    let hpc = (high - prev_close).abs();
    let lpc = (low  - prev_close).abs();
    hl.max(hpc).max(lpc)
}

/// Wilder-smoothed ATR — نفس خوارزمية TradingView
pub fn compute_atr(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period + 1 { return None; }

    let start  = candles.len().saturating_sub(period + 1);
    let window = &candles[start..];

    let initial_sum: f64 = window[..period]
        .windows(2)
        .map(|w| true_range(w[1].high, w[1].low, w[0].close))
        .sum();

    let mut atr = initial_sum / period as f64;

    for pair in window[period..].windows(2) {
        let tr = true_range(pair[1].high, pair[1].low, pair[0].close);
        atr    = (atr * (period as f64 - 1.0) + tr) / period as f64;
    }

    if atr > 0.0 { Some(atr) } else { None }
}

// ─────────────────────────────────────────────────────────────
//  Adaptive multiplier
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AtrContext {
    pub atr:        f64,
    pub atr_pct:    f64,
    pub multiplier: f64,
    pub regime:     VolatilityRegime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VolatilityRegime {
    Low,
    Normal,
    High,
    Extreme,
}

impl std::fmt::Display for VolatilityRegime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low     => write!(f, "LOW     📉"),
            Self::Normal  => write!(f, "NORMAL  📊"),
            Self::High    => write!(f, "HIGH    ⚠️"),
            Self::Extreme => write!(f, "EXTREME 🚨"),
        }
    }
}

/// ✨ يأخذ مستوى السعر بعين الاعتبار
/// عند أسعار < $0.10: baseline أعلى (الأصول الرخيصة أكثر تقلباً نسبياً)
fn effective_baseline(current_price: f64) -> f64 {
    if current_price < 0.10 {
        BASELINE_ATR_RATIO * 2.5  // 8.75% للأصول < 10 سنت
    } else if current_price < 0.30 {
        BASELINE_ATR_RATIO * 1.8  // 6.3% للأصول 10-30 سنت
    } else {
        BASELINE_ATR_RATIO        // 3.5% للأسعار الطبيعية
    }
}

pub fn atr_context(atr: f64, current_price: f64) -> AtrContext {
    let atr_pct    = atr / current_price;
    let baseline   = effective_baseline(current_price);
    let ratio      = atr_pct / baseline;
    let raw_mult   = ratio.sqrt(); // sqrt يكبح التطرف
    let multiplier = raw_mult.clamp(MIN_MULTIPLIER, MAX_MULTIPLIER);

    let regime = if ratio < 0.5 {
        VolatilityRegime::Low
    } else if ratio < 1.5 {
        VolatilityRegime::Normal
    } else if ratio < 2.5 {
        VolatilityRegime::High
    } else {
        VolatilityRegime::Extreme
    };

    debug!(
        "ATR={:.6} ({:.2}%) | baseline={:.2}% | ratio={:.2} | mult={:.2} | regime={}",
        atr, atr_pct * 100.0, baseline * 100.0, ratio, multiplier, regime
    );

    AtrContext { atr, atr_pct, multiplier, regime }
}

// ─────────────────────────────────────────────────────────────
//  Grid range adapter (للشبكة التكيفية مع OHLC)
// ─────────────────────────────────────────────────────────────

pub fn adapt_grid_range(high: f64, low: f64, ctx: &AtrContext) -> (f64, f64) {
    let mid        = (high + low) / 2.0;
    let half_range = (high - low) / 2.0;

    let eff_high = mid + half_range * ctx.multiplier;
    let eff_low  = (mid - half_range * ctx.multiplier).max(0.0001);

    debug!(
        "Grid range: raw [{:.6}, {:.6}] → adapted [{:.6}, {:.6}] (×{:.2})",
        low, high, eff_low, eff_high, ctx.multiplier
    );

    (eff_high, eff_low)
}

/// ✨ حساب حدود الشبكة المتمركزة حول السعر الحالي
///
/// بدلاً من High/Low التاريخي، نستخدم ATR لتحديد النطاق:
///   high = current_price + ATR × multiplier × CENTERED_GRID_ATR_MULTIPLIER
///   low  = current_price - ATR × multiplier × CENTERED_GRID_ATR_MULTIPLIER
///
/// هذا يضمن أن الشبكة تتبع السعر الحالي دائماً
pub fn compute_centered_bounds(current_price: f64, ctx: &AtrContext) -> (f64, f64) {
    let half_range = ctx.atr * ctx.multiplier * CENTERED_GRID_ATR_MULTIPLIER;
    let eff_high   = current_price + half_range;
    let eff_low    = (current_price - half_range).max(0.0001);

    debug!(
        "Centered bounds: price={:.6} ± {:.6} → [{:.6}, {:.6}]",
        current_price, half_range, eff_low, eff_high
    );

    (eff_high, eff_low)
}

/// هل السعر الحالي "بعيد جداً" عن النطاق التاريخي؟
/// إذا كان السعر في أسفل 20% أو أعلى 80% من النطاق → الشبكة المتمركزة أفضل
pub fn should_use_centered_grid(current_price: f64, ohlc_high: f64, ohlc_low: f64) -> bool {
    let range = ohlc_high - ohlc_low;
    if range < 1e-9 { return true; }
    let ratio = (current_price - ohlc_low) / range;
    ratio < 0.20 || ratio > 0.80
}

// ─────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::Candle;

    fn candle(close: f64, high: f64, low: f64) -> Candle {
        Candle { open_time_ms: 0, open: close, high, low, close, volume: 1000.0 }
    }

    #[test]
    fn test_atr_requires_enough_candles() {
        let candles: Vec<Candle> = (0..5).map(|_| candle(1.0, 1.1, 0.9)).collect();
        assert!(compute_atr(&candles, 14).is_none());
    }

    #[test]
    fn test_atr_flat_market() {
        let candles: Vec<Candle> = (0..30).map(|_| candle(1.0, 1.05, 0.95)).collect();
        let atr = compute_atr(&candles, 14).unwrap();
        assert!((atr - 0.10).abs() < 0.01, "atr={}", atr);
    }

    #[test]
    fn test_multiplier_normal_regime() {
        let ctx = atr_context(0.035, 1.0);
        assert_eq!(ctx.regime, VolatilityRegime::Normal);
        assert!((ctx.multiplier - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_multiplier_clamped() {
        let ctx = atr_context(10.0, 1.0);
        assert!(ctx.multiplier <= MAX_MULTIPLIER);

        let ctx2 = atr_context(0.0001, 1.0);
        assert!(ctx2.multiplier >= MIN_MULTIPLIER);
    }

    #[test]
    fn test_centered_bounds_always_surround_price() {
        let prices = [1.0, 0.5, 0.08, 0.001];
        for price in prices {
            let ctx = atr_context(price * 0.05, price);
            let (high, low) = compute_centered_bounds(price, &ctx);
            assert!(high > price, "high={:.6} > price={:.6}", high, price);
            assert!(low  < price, "low={:.6}  < price={:.6}", low,  price);
        }
    }

    #[test]
    fn test_centered_bounds_low_never_negative() {
        let ctx = AtrContext {
            atr: 5.0, atr_pct: 5.0, multiplier: 2.0,
            regime: VolatilityRegime::Extreme,
        };
        let (_, eff_low) = compute_centered_bounds(0.001, &ctx);
        assert!(eff_low > 0.0);
    }

    #[test]
    fn test_should_use_centered_at_extremes() {
        // سعر عند قاع النطاق
        assert!(should_use_centered_grid(0.05, 1.0, 0.0));
        // سعر في منتصف النطاق
        assert!(!should_use_centered_grid(0.50, 1.0, 0.0));
        // سعر عند قمة النطاق
        assert!(should_use_centered_grid(0.95, 1.0, 0.0));
    }

    #[test]
    fn test_effective_baseline_low_price() {
        // الأصول الرخيصة لها baseline أعلى
        let ctx_cheap  = atr_context(0.005, 0.08); // 6.25% ATR
        let ctx_normal = atr_context(0.035, 1.0);   // 3.5% ATR (baseline)
        // كلاهما يجب أن يكون في نفس النظام التقريباً
        // لأن الـ baseline مختلف لكل منهما
        assert!(ctx_cheap.regime == VolatilityRegime::Normal || 
                ctx_cheap.regime == VolatilityRegime::Low);
        assert_eq!(ctx_normal.regime, VolatilityRegime::Normal);
    }

    #[test]
    fn test_adapt_grid_range_widens_on_high_atr() {
        let ctx = AtrContext {
            atr: 0.2, atr_pct: 0.2, multiplier: 1.5,
            regime: VolatilityRegime::High,
        };
        let (eff_high, eff_low) = adapt_grid_range(1.0, 0.5, &ctx);
        assert!(eff_high > 1.0);
        assert!(eff_low  < 0.5);
    }
}
