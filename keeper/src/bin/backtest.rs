// ============================================================
//  src/bin/backtest.rs  —  Backtest Runner
//
//  Run:  cargo run --bin backtest
//  Or:   cargo run --bin backtest -- --interval 4h --window 20
// ============================================================

use celo_grid_keeper_v2::backtest::{
    fetch_candles, run_backtest, print_report, BacktestConfig,
};
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    info!("🚀 Starting backtest — fetching historical CELO/USDT data...");

    // جلب القيم الافتراضية من الـ Library التي عدلتها أنت سابقاً
    let default_cfg = BacktestConfig::default();

    // ── Configuration (سيتم استخدام الـ Default الذي وضعته لـ 4h و 2000 شمعة) ───
    let cfg = BacktestConfig {
        symbol:          std::env::var("BT_SYMBOL").unwrap_or(default_cfg.symbol),
        interval:        std::env::var("BT_INTERVAL").unwrap_or(default_cfg.interval),
        total_candles:   std::env::var("BT_CANDLES")
                             .ok().and_then(|v| v.parse().ok())
                             .unwrap_or(2000), // زدناها لـ 2000 لتناسب فريم الـ 4h
        window_size:     std::env::var("BT_WINDOW")
                             .ok().and_then(|v| v.parse().ok())
                             .unwrap_or(default_cfg.window_size),
        trade_size_usd:  std::env::var("BT_TRADE_SIZE")
                             .ok().and_then(|v| v.parse().ok())
                             .unwrap_or(default_cfg.trade_size_usd),
        crash_threshold: std::env::var("BT_CRASH_PCT")
                             .ok().and_then(|v| v.parse().ok())
                             .unwrap_or(default_cfg.crash_threshold),
                             
        local_peak_window: default_cfg.local_peak_window,
        use_dynamic_grid:  default_cfg.use_dynamic_grid,
    };

    info!(
        "📋 Config: {} {} | {} candles | window={} | trade=${:.0}",
        cfg.symbol, cfg.interval, cfg.total_candles,
        cfg.window_size, cfg.trade_size_usd
    );

    let client = reqwest::Client::builder()
        .user_agent("CeloGridKeeperV2-Backtest/1.0")
        .timeout(Duration::from_secs(20))
        .build()?;

    let candles = fetch_candles(
        &client,
        &cfg.symbol,
        &cfg.interval,
        cfg.total_candles,
    ).await?;

    info!("✅ Fetched {} candles — running simulation...", candles.len());

    let result = run_backtest(&candles, &cfg)?;
    print_report(&result);

    Ok(())
}
