// ============================================================
//  grid.rs  —  Fibonacci Grid Engine  (V2 + ATR Adaptive + Dynamic)
//
//  FIXES (v2.1):
//    [HIGH]  Test helper `ohlc()` updated to match new OhlcData layout.
//            OhlcData now carries `candles: Vec<Candle>` and
//            `last_atr: Option<f64>` — the old struct literal
//            `{ high, low, last_updated, candles_used }` would fail to
//            compile with the fixed price_feed.rs.
//    [PERF]  `active_levels` result collected once before the inner
//            loop, eliminating 2 450+ redundant iterator traversals
//            per cycle on a full 50-grid deployment.
// ============================================================

use crate::atr::{adapt_grid_range, AtrContext};
use crate::price_feed::OhlcData;
use tracing::debug;

pub const FIB_RATIOS: &[f64] = &[0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];

/// 0.786 = Golden Ratio complement (1 − 1/φ²).
/// Breaks below this Fibonacci level are statistically indicative of
/// trend reversal — the circuit breaker suspends all buy execution.
pub const CIRCUIT_BREAKER_RATIO: f64 = 0.786;

const EPSILON: f64 = 1e-9;

// ─────────────────────────────────────────────────────────────
//  Types
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GridLevel {
    pub ratio:     f64,
    pub price:     f64,
    pub is_active: bool,
    pub side:      OrderSide,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
    Neutral,
}

#[derive(Debug)]
pub struct GridResult {
    pub circuit_breaker_price:     f64,
    pub circuit_breaker_triggered: bool,
    pub levels:                    Vec<GridLevel>,
    pub used_high:                 f64,
    pub used_low:                  f64,
    pub atr_ctx:                   Option<AtrContext>,
    /// `true` when the grid is centred around the current price
    /// (used when the price has drifted far from the historical range).
    pub is_centered:               bool,
}

// ─────────────────────────────────────────────────────────────
//  Public API
// ─────────────────────────────────────────────────────────────

/// Static grid anchored to the OHLC window's High/Low.
/// Used as fallback when fewer than 15 candles are available (no ATR).
pub fn compute_grid(ohlc: &OhlcData, current_price: f64) -> GridResult {
    compute_grid_inner(ohlc.high, ohlc.low, current_price, None, false)
}

/// ATR-adaptive grid: the High/Low bounds are widened or narrowed
/// according to the current volatility regime before level computation.
pub fn compute_grid_adaptive(
    ohlc:          &OhlcData,
    current_price: f64,
    atr_ctx:       &AtrContext,
) -> GridResult {
    let (eff_high, eff_low) = adapt_grid_range(ohlc.high, ohlc.low, atr_ctx);
    compute_grid_inner(eff_high, eff_low, current_price, Some(atr_ctx.clone()), false)
}

/// Dynamic centred grid.
///
/// When the current price is far outside the historical High/Low range
/// (e.g. after a crash), the OHLC bounds are stale and produce a grid
/// where *all* levels are below the current price — no sell side, no fills.
///
/// This mode anchors the grid symmetrically around `current_price`:
///   half_range = ATR × multiplier × 2
///   high = current_price + half_range
///   low  = max(current_price − half_range, 0.0001)
pub fn compute_grid_centered(current_price: f64, atr_ctx: &AtrContext) -> GridResult {
    let half_range = atr_ctx.atr * atr_ctx.multiplier * 2.0;
    let eff_high   = current_price + half_range;
    let eff_low    = (current_price - half_range).max(0.0001);

    debug!(
        price = current_price,
        half_range,
        eff_low,
        eff_high,
        atr  = atr_ctx.atr,
        mult = atr_ctx.multiplier,
        "Centred grid computed"
    );

    compute_grid_inner(eff_high, eff_low, current_price, Some(atr_ctx.clone()), true)
}

/// Automatic mode selector.
///
/// Uses `price_ratio = (price − low) / (high − low)` to decide which
/// grid strategy fits the current market position:
///   - ratio < 20% or > 80%  → centred (price too far from history)
///   - 20% ≤ ratio ≤ 80%     → adaptive (price inside reasonable range)
///
/// Falls back to the static grid when no ATR context is available.
pub fn compute_grid_auto(
    ohlc:          &OhlcData,
    current_price: f64,
    atr_ctx_opt:   Option<&AtrContext>,
) -> GridResult {
    let Some(atr_ctx) = atr_ctx_opt else {
        return compute_grid(ohlc, current_price);
    };

    let range = ohlc.high - ohlc.low;
    let price_ratio = if range > EPSILON {
        (current_price - ohlc.low) / range
    } else {
        0.5
    };

    if price_ratio < 0.20 || price_ratio > 0.80 {
        debug!(price_ratio, "Auto: switching to CENTERED grid");
        compute_grid_centered(current_price, atr_ctx)
    } else {
        debug!(price_ratio, "Auto: using ADAPTIVE grid");
        compute_grid_adaptive(ohlc, current_price, atr_ctx)
    }
}

// ─────────────────────────────────────────────────────────────
//  Internal engine
// ─────────────────────────────────────────────────────────────

fn compute_grid_inner(
    high:          f64,
    low:           f64,
    current_price: f64,
    atr_ctx:       Option<AtrContext>,
    is_centered:   bool,
) -> GridResult {
    let range    = high - low;
    let cb_price = fib_price(high, range, CIRCUIT_BREAKER_RATIO);
    let cb_triggered = current_price < cb_price;

    if cb_triggered {
        tracing::warn!(
            price     = current_price,
            cb_level  = cb_price,
            fib_ratio = CIRCUIT_BREAKER_RATIO,
            mode      = if is_centered { "CENTERED" } else { "OHLC" },
            "⛔ Circuit breaker triggered — buy execution suspended"
        );
    }

    let mode_tag = if is_centered { "[CENTERED]" }
                   else if atr_ctx.is_some() { "[ATR-adj]" }
                   else { "" };

    let levels = FIB_RATIOS
        .iter()
        .map(|&ratio| {
            let price     = fib_price(high, range, ratio);
            let side      = classify_side(ratio, current_price, price);
            // Circuit breaker only suspends buy orders; sells continue to
            // allow the bot to capture any recovery bounce.
            let is_active = !(cb_triggered && side == OrderSide::Buy);

            debug!(
                fib    = ratio,
                price,
                side   = ?side,
                active = is_active,
                mode   = mode_tag,
                "Grid level"
            );

            GridLevel { ratio, price, is_active, side }
        })
        .collect();

    GridResult {
        circuit_breaker_price:     cb_price,
        circuit_breaker_triggered: cb_triggered,
        levels,
        used_high:                 high,
        used_low:                  low,
        atr_ctx,
        is_centered,
    }
}

// ─────────────────────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────────────────────

/// Returns an iterator over all active (non-CB-suspended) levels.
///
/// FIX [PERF]: callers in the keeper loop should collect this once
/// before entering the inner `grid_level` loop:
///
/// ```rust,no_run
/// let fib_levels: Vec<&GridLevel> = active_levels(&grid_result).collect();
/// for on_chain_grid in &active_grids {
///     for gl in &on_chain_grid.levels {
///         for level in &fib_levels { ... }
///     }
/// }
/// ```
pub fn active_levels(result: &GridResult) -> impl Iterator<Item = &GridLevel> {
    result.levels.iter().filter(|l| l.is_active)
}

/// Pre-collect active levels into a Vec to avoid re-iterating the
/// slice on every pass through the inner loop.
pub fn active_levels_vec(result: &GridResult) -> Vec<&GridLevel> {
    result.levels.iter().filter(|l| l.is_active).collect()
}

#[inline]
fn fib_price(high: f64, range: f64, ratio: f64) -> f64 {
    high - range * ratio
}

fn classify_side(ratio: f64, current_price: f64, level_price: f64) -> OrderSide {
    if (ratio - 0.5).abs() < EPSILON {
        return OrderSide::Neutral;
    }
    if (level_price - current_price).abs() < EPSILON {
        return OrderSide::Neutral;
    }
    if level_price < current_price {
        OrderSide::Buy
    } else {
        OrderSide::Sell
    }
}

// ─────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atr::{AtrContext, VolatilityRegime};
    use std::time::Instant;

    /// FIX [HIGH]: OhlcData struct now requires `candles` and `last_atr`
    /// fields (added in price_feed.rs v2.1 fix).  The old literal
    /// `{ high, low, last_updated, candles_used }` would not compile.
    fn ohlc(high: f64, low: f64) -> OhlcData {
        OhlcData {
            high,
            low,
            candles:      vec![],
            last_atr:     None,
            last_updated: Instant::now(),
            candles_used: 50,
        }
    }

    fn make_atr_ctx(atr: f64, price: f64, mult: f64) -> AtrContext {
        AtrContext {
            atr,
            atr_pct:    atr / price,
            multiplier: mult,
            regime:     VolatilityRegime::Normal,
        }
    }

    // ── Core correctness ─────────────────────────────────────

    #[test]
    fn test_cb_ratio_is_786() {
        assert!((CIRCUIT_BREAKER_RATIO - 0.786).abs() < 1e-9);
    }

    #[test]
    fn test_cb_triggers_at_786_level() {
        // high=1.0, low=0.0  →  cb_price = 1.0 − 1.0 × 0.786 = 0.214
        let below = compute_grid(&ohlc(1.0, 0.0), 0.10);
        assert!(below.circuit_breaker_triggered,  "CB must fire at 0.10");

        let above = compute_grid(&ohlc(1.0, 0.0), 0.30);
        assert!(!above.circuit_breaker_triggered, "CB must not fire at 0.30");
    }

    #[test]
    fn test_level_count() {
        assert_eq!(
            compute_grid(&ohlc(1.0, 0.5), 0.75).levels.len(),
            FIB_RATIOS.len()
        );
    }

    #[test]
    fn test_circuit_breaker_suspends_buys_only() {
        let result = compute_grid(&ohlc(1.0, 0.0), 0.1);
        assert!(result.circuit_breaker_triggered);
        // No active buy orders when CB is live
        assert!(!result.levels.iter().any(|l| l.side == OrderSide::Buy && l.is_active));
        // Sell orders remain active so the bot can exit positions
        assert!(result.levels.iter().any(|l| l.side == OrderSide::Sell && l.is_active));
    }

    #[test]
    fn test_no_cb_above_support() {
        let result = compute_grid(&ohlc(1.0, 0.0), 0.5);
        assert!(!result.circuit_breaker_triggered);
    }

    // ── Centred grid ─────────────────────────────────────────

    #[test]
    fn test_centered_grid_straddles_price() {
        let price  = 0.08_f64;
        let ctx    = make_atr_ctx(0.01, price, 1.0);
        let result = compute_grid_centered(price, &ctx);

        assert!(result.used_high > price, "high must be above current price");
        assert!(result.used_low  < price, "low must be below current price");
        assert!(result.is_centered);

        let has_buys  = result.levels.iter().any(|l| l.side == OrderSide::Buy);
        let has_sells = result.levels.iter().any(|l| l.side == OrderSide::Sell);
        assert!(has_buys,  "centred grid needs buy levels");
        assert!(has_sells, "centred grid needs sell levels");
    }

    #[test]
    fn test_centered_grid_level_count() {
        let price  = 0.08_f64;
        let ctx    = make_atr_ctx(0.005, price, 1.0);
        let result = compute_grid_centered(price, &ctx);
        assert_eq!(result.levels.len(), FIB_RATIOS.len());
        assert!(result.is_centered);
    }

    // ── Auto selector ────────────────────────────────────────

    #[test]
    fn test_auto_uses_centered_at_bottom() {
        // 5% of historical range → price_ratio = 0.05 → centred mode
        let price  = 0.05_f64;
        let ctx    = make_atr_ctx(0.01, price, 1.0);
        let result = compute_grid_auto(&ohlc(1.0, 0.0), price, Some(&ctx));
        assert!(result.is_centered, "must use centred mode at bottom of range");
    }

    #[test]
    fn test_auto_uses_adaptive_in_mid_range() {
        // 50% of historical range → price_ratio = 0.50 → adaptive mode
        let price  = 0.5_f64;
        let ctx    = make_atr_ctx(0.05, price, 1.0);
        let result = compute_grid_auto(&ohlc(1.0, 0.0), price, Some(&ctx));
        assert!(!result.is_centered, "must use adaptive mode in mid-range");
    }

    // ── Adaptive expansion ───────────────────────────────────

    #[test]
    fn test_adaptive_widens_range_vs_static() {
        let base   = compute_grid(&ohlc(1.0, 0.5), 0.75);
        let ctx    = AtrContext {
            atr: 0.3, atr_pct: 0.3, multiplier: 1.5,
            regime: VolatilityRegime::High,
        };
        let wide   = compute_grid_adaptive(&ohlc(1.0, 0.5), 0.75, &ctx);
        assert!(wide.used_high - wide.used_low > base.used_high - base.used_low);
    }

    // ── Edge cases ───────────────────────────────────────────

    #[test]
    fn test_midpoint_level_is_neutral() {
        let result = compute_grid(&ohlc(1.0, 0.0), 0.5);
        let mid    = result.levels.iter().find(|l| (l.ratio - 0.5).abs() < 1e-9).unwrap();
        assert_eq!(mid.side, OrderSide::Neutral);
    }

    #[test]
    fn test_active_levels_vec_matches_iterator() {
        let result = compute_grid(&ohlc(1.0, 0.0), 0.5);
        let via_iter: Vec<_> = active_levels(&result).collect();
        let via_vec           = active_levels_vec(&result);
        assert_eq!(via_iter.len(), via_vec.len());
    }
}
