// ============================================================
//  test_keeper_v2.rs  –  اختبار شامل للـ Keeper Engine V2
//  المميزات:
//    ✅ اختبار Price Feed (4 مصادر)
//    ✅ اختبار OHLC + Fibonacci Grid
//    ✅ اختبار ChainClientV2 (RPC Health, Dynamic Gas)
//    ✅ Dry-Run Mode (بدون تنفيذ فعلي)
//    ✅ Health Check Report
// ============================================================

use celo_grid_keeper_v2::price_feed::{fetch_aggregated_price, fetch_ohlc, KlineParams};
use celo_grid_keeper_v2::grid::{compute_grid_auto, active_levels};
use celo_grid_keeper_v2::chain_v2::{ChainClient, GasConfig};
use celo_grid_keeper_v2::atr;
use std::time::Duration;
use colored::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("{}", "╔════════════════════════════════════════════════════════════╗".bright_blue());
    println!("{}", "║     Celo Grid Keeper V2 - Engine Test Suite                ║".bright_blue());
    println!("{}", "╚════════════════════════════════════════════════════════════╝".bright_blue());

    tracing_subscriber::fmt().with_target(false).compact().init();

    let client = reqwest::Client::builder()
        .user_agent("CeloGridKeeperV2-Test/2.0")
        .timeout(Duration::from_secs(15))
        .build()?;

    let mut all_tests_passed = true;

    // ═══════════════════════════════════════════════════════════
    //  TEST 1: Price Feed (4 مصادر)
    // ═══════════════════════════════════════════════════════════
    println!("\n{}", "▶ TEST 1: Aggregated Price Feed".bold());
    println!("{}", "─────────────────────────────────────".dimmed());
    
    match fetch_aggregated_price(&client, "CELOUSDT").await {
        Ok(price) => {
            println!("{} Price: {:.4} USDT", "✅".green(), price.price);
            println!("  Sources: {:?}", price.sources_used);
            println!("  Deviation Warning: {}", 
                if price.price_deviation_warning { "⚠️ YES".yellow() } else { "✅ NO".green() });
        }
        Err(e) => {
            println!("{} Failed to fetch price: {}", "❌".red(), e);
            all_tests_passed = false;
        }
    }

    // ═══════════════════════════════════════════════════════════
    //  TEST 2: OHLC Data + ATR
    // ═══════════════════════════════════════════════════════════
    println!("\n{}", "▶ TEST 2: OHLC Data + ATR Calculation".bold());
    println!("{}", "─────────────────────────────────────".dimmed());
    
    let params = KlineParams {
        symbol: "CELOUSDT".to_string(),
        interval: "15m".to_string(),
        limit: 50,
    };
    
    match fetch_ohlc(&client, &params).await {
        Ok(ohlc) => {
            println!("{} High: {:.4} | Low: {:.4}", "✅".green(), ohlc.high, ohlc.low);
            
            if let Some(atr) = ohlc.get_last_atr() {
                // استخدام منتصف النطاق كسعر تقديري لحساب ATR%
                let mid_price = (ohlc.high + ohlc.low) / 2.0;
                let atr_pct = (atr / mid_price) * 100.0;
                println!("  ATR(14): {:.6} ({:.2}%)", atr, atr_pct);
                
                // اختبار ATR Context
                let atr_ctx = atr::atr_context(atr, mid_price);
                println!("  Regime: {:?} | Multiplier: {:.2}", atr_ctx.regime, atr_ctx.multiplier);
            } else {
                println!("{} ATR not calculated", "⚠️".yellow());
            }
        }
        Err(e) => {
            println!("{} Failed to fetch OHLC: {}", "❌".red(), e);
            all_tests_passed = false;
        }
    }

    // ═══════════════════════════════════════════════════════════
    //  TEST 3: Fibonacci Grid with Adaptive Spacing
    // ═══════════════════════════════════════════════════════════
    println!("\n{}", "▶ TEST 3: Adaptive Fibonacci Grid".bold());
    println!("{}", "─────────────────────────────────────".dimmed());
    
    let ohlc = fetch_ohlc(&client, &params).await?;
    let price = fetch_aggregated_price(&client, "CELOUSDT").await?;
    
    let atr_ctx = ohlc.get_last_atr().map(|atr| atr::atr_context(atr, price.price));
    let grid = compute_grid_auto(&ohlc, price.price, atr_ctx.as_ref());
    
    println!("{} Circuit Breaker: {}", 
        if grid.circuit_breaker_triggered { "⚠️".yellow() } else { "✅".green() },
        if grid.circuit_breaker_triggered { "TRIGGERED".red() } else { "Normal".green() });
    
    let active_count = active_levels(&grid).count();
    println!("\n  Grid Levels ({} active):", active_count);
    for (_i, level) in grid.levels.iter().enumerate() {
        let icon = match level.side {
            celo_grid_keeper_v2::grid::OrderSide::Buy => "🟢",
            celo_grid_keeper_v2::grid::OrderSide::Sell => "🔴",
            celo_grid_keeper_v2::grid::OrderSide::Neutral => "⚪",
        };
        let status = if level.is_active { "".green() } else { "[SKIP]".dimmed() };
        println!("    {} {:.4} | {:.3} | {:?} {}", 
            icon, level.price, level.ratio, level.side, status);
    }

    // ═══════════════════════════════════════════════════════════
    //  TEST 4: ChainClientV2 (Dry Run)
    // ═══════════════════════════════════════════════════════════
    println!("\n{}", "▶ TEST 4: ChainClientV2 - Dry Run".bold());
    println!("{}", "─────────────────────────────────────".dimmed());
    
    dotenvy::dotenv().ok();
    
    // التحقق من وجود متغيرات البيئة
    let required_vars = ["CONTRACT_ADDRESS", "KEEPER_ADDRESS", "RPC_URL", "PRIVATE_KEY"];
    let mut missing_vars = vec![];
    for var in &required_vars {
        if std::env::var(var).is_err() {
            missing_vars.push(*var);
        }
    }
    
    if !missing_vars.is_empty() {
        println!("{} Missing env vars: {:?}", "⚠️".yellow(), missing_vars);
        println!("   Skipping on-chain tests...");
    } else {
        // إعداد ChainClientV2
        let gas_config = GasConfig {
            gas_limit: 300000,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            fee_currency: std::env::var("FEE_CURRENCY_ADDRESS")
                .ok()
                .and_then(|addr| addr.parse().ok()),
            use_legacy_tx: false,
        };
        
        let backup_urls: Vec<String> = std::env::var("BACKUP_RPC_URLS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        println!("  Initializing ChainClientV2...");
        println!("    Primary RPC: {}", std::env::var("RPC_URL")?.dimmed());
        println!("    Backup RPCs: {} endpoints", backup_urls.len());
        println!("    Fee Currency: {:?}", gas_config.fee_currency);
        
        let dry_run = std::env::var("DRY_RUN_MODE")
            .map(|v| v == "true")
            .unwrap_or(true); // افتراضياً true للاختبار
        
        println!("    DRY RUN MODE: {} (no real transactions)", 
            if dry_run { "ENABLED".yellow() } else { "DISABLED".red() });
        
        match ChainClient::new(
            std::env::var("CONTRACT_ADDRESS")?,
            std::env::var("KEEPER_ADDRESS")?,
            std::env::var("RPC_URL")?,
            backup_urls,
            std::env::var("PRIVATE_KEY")?,
            3,
            gas_config,
            true, // simulate_before_execute
            0.5,  // min_profit_usd
            dry_run,
        ) {
            Ok(chain) => {
                // اختبار RPC Health
                println!("\n  Checking RPC Health...");
                let health = chain.get_health_report().await;
                println!("    RPC Status: {}", health["rpc_status"].as_str().unwrap_or("Unknown"));
                println!("    Current URL: {}", health["rpc_current_url"].as_str().unwrap_or("N/A"));
                println!("    Success Rate: {:.1}%", health["success_rate"].as_f64().unwrap_or(0.0));
                println!("    Consecutive Failures: {}", health["consecutive_failures"].as_u64().unwrap_or(0));
                
                // اختبار جلب الـ Grids
                println!("\n  Fetching Active Grids...");
                match chain.get_active_grids().await {
                    Ok(grids) => {
                        println!("{} Found {} active grid(s)", "✅".green(), grids.len());
                        for g in &grids {
                            println!("    Grid #{} | {} levels | range: {:.4} - {:.4}", 
                                g.id, g.levels.len(), 
                                g.lower_price.to_string().parse::<f64>().unwrap_or(0.0) / 1e18,
                                g.upper_price.to_string().parse::<f64>().unwrap_or(0.0) / 1e18);
                        }
                        
                        // اختبار Simulation (بدون تنفيذ فعلي)
                        if let Some(grid) = grids.first() {
                            if let Some(level) = grid.levels.iter().find(|l| !l.filled) {
                                println!("\n  Simulating execution (Grid #{}, Level {})...", 
                                    grid.id, level.index);
                                match chain.simulate_execution(grid.id, level.index).await {
                                    Ok(result) => {
                                        println!("{} Simulation Result:", "✅".green());
                                        println!("    Would Succeed: {}", result.would_succeed);
                                        println!("    Estimated Gas: {}", result.estimated_gas);
                                        println!("    Gas Cost: ${:.4}", result.gas_cost_usd);
                                        if let Some(err) = result.revert_reason {
                                            println!("    ⚠️ Revert Reason: {}", err);
                                        }
                                    }
                                    Err(e) => {
                                        println!("{} Simulation failed: {}", "❌".red(), e);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("{} Failed to fetch grids: {}", "❌".red(), e);
                        all_tests_passed = false;
                    }
                }
            }
            Err(e) => {
                println!("{} Failed to initialize ChainClient: {}", "❌".red(), e);
                all_tests_passed = false;
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    //  FINAL REPORT
    // ═══════════════════════════════════════════════════════════
    println!("\n{}", "═══════════════════════════════════════════════════════════════".bright_blue());
    if all_tests_passed {
        println!("{} All tests passed! Engine is ready.", "✅✅✅".green().bold());
    } else {
        println!("{} Some tests failed. Check logs above.", "❌❌❌".red().bold());
        std::process::exit(1);
    }
    println!("{}", "═══════════════════════════════════════════════════════════════".bright_blue());

    Ok(())
}
