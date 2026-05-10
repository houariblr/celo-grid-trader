// ============================================================
//  main_v2.rs  –  Production-Grade Keeper Entry Point
//
//  FIXES (v2.1):
//    [CRITICAL] reqwest::Client no longer created per keeper cycle.
//               One shared Arc<Client> lives in KeeperState and is
//               reused by every cycle — eliminates the fd leak that
//               would exhaust the OS connection pool over time.
//
//    [CRITICAL] Module declarations replaced with `use` imports from
//               the library crate (lib.rs).  Having both `mod foo`
//               and a lib.rs `pub mod foo` causes the compiler to
//               compile each module twice, producing duplicate-type
//               errors and preventing the two binaries from sharing
//               any common code.
//
//    [LOW]      last_execution switched from
//               Arc<RwLock<Option<Instant>>> to Arc<AtomicU64>
//               (Unix seconds).  A timestamp is a scalar — a full
//               async read/write lock is unnecessary overhead.
//
//    [PERF]     active_levels_vec() used in the inner grid loop so
//               the Fibonacci slice is collected once per cycle, not
//               re-iterated on every on-chain grid level comparison.
//
//    [PERF]     Shared HTTP client passed to ohlc_updater_task so it
//               no longer rebuilds a new connection pool on every
//               background-task spawn.
// ============================================================

// Use the library crate — do NOT re-declare modules here.
// Declaring `mod config;` here AND `pub mod config;` in lib.rs
// compiles the module twice, breaks type identity between binaries,
// and causes "mismatched types" errors that are hard to diagnose.
use celo_grid_keeper_v2::config::Config;
// BUG-8 FIX: OrderSide & active_levels_vec removed (no longer used after trigger fix)
use celo_grid_keeper_v2::grid::compute_grid_auto;
use celo_grid_keeper_v2::price_feed::{
    fetch_aggregated_price, ohlc_updater_task, KlineParams, OhlcData, SharedOhlc,
};
use celo_grid_keeper_v2::chain_v2::{ChainClient, GasConfig};
use celo_grid_keeper_v2::keeper_ws_server::{create_update_payload, WsServer};
use celo_grid_keeper_v2::atr;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::{Context, Result};
use tokio::sync::RwLock;
use tokio::signal;
use tracing::{debug, error, info, instrument, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use serde_json::json;

// ─────────────────────────────────────────────────────────────
//  Shared state
// ─────────────────────────────────────────────────────────────

pub struct KeeperState {
    pub ohlc:                    SharedOhlc,
    pub chain_client:            Arc<ChainClient>,
    pub ws_server:               Arc<WsServer>,
    pub config:                  Arc<Config>,
    /// FIX [LOW]: was Arc<RwLock<Option<Instant>>>.
    /// Unix-second timestamp stored atomically — no async lock needed
    /// for a single u64 scalar.  0 = never executed.
    pub last_execution_unix_sec: Arc<AtomicU64>,
    pub circuit_breaker_active:  Arc<RwLock<bool>>,
    /// FIX [CRITICAL]: single shared HTTP client — one connection pool,
    /// one DNS resolver, one TLS session cache for the entire process.
    pub http_client:             Arc<reqwest::Client>,
}

impl std::fmt::Debug for KeeperState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeeperState")
            .field("ohlc", &"<SharedOhlc>")
            .field("chain_client", &"<ChainClient>")
            .field("ws_server", &"<WsServer>")
            .field("config", &self.config)
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────
//  Entry point
// ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging()?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "🚀 Celo Grid Keeper V2 — Production Edition"
    );

    // 1. Config
    let config = Arc::new(Config::from_env().context("Failed to load configuration")?);

    info!(
        rpc_url              = %config.rpc_url,
        keeper_address       = %config.keeper_address,
        contract_address     = %config.contract_address,
        simulate             = config.simulate_before_execute,
        dry_run              = config.dry_run_mode,
        "Configuration loaded"
    );

    // 2. WebSocket server
    let (ws_server, _ws_rx) = WsServer::new();
    let ws_server = Arc::new(ws_server);
    {
        let ws = Arc::clone(&ws_server);
        // BUG-17 FIX: Use the configured bind address (config.ws_server_bind_address) instead of
        // a hardcoded string, so the operator can control the interface/port via .env.
        let ws_bind = config.ws_server_bind_address.clone();
        tokio::spawn(async move {
            if let Err(e) = ws.run(&ws_bind).await {
                error!(error = %e, "WebSocket server error");
            }
        });
    }

    // 3. Shared HTTP client
    //    FIX [CRITICAL]: one client for the entire process lifetime.
    //    reqwest::Client internally holds a connection pool — creating
    //    a new one per keeper cycle leaks file descriptors at ~720/hour.
    let http_client = Arc::new(
        reqwest::Client::builder()
            .user_agent("CeloGridKeeperV2/2.1")
            .timeout(Duration::from_secs(15))
            .build()
            .context("Failed to create HTTP client")?,
    );

    // 4. Chain client V2
    let gas_config = GasConfig {
        gas_limit:               config.gas_limit,
        max_fee_per_gas:         None,
        max_priority_fee_per_gas: None,
        fee_currency:            config.fee_currency_address
            .as_ref()
            .and_then(|a| a.parse().ok()),
        use_legacy_tx:           config.use_legacy_tx,
    };

    let chain_client = Arc::new(
        ChainClient::new(
            config.contract_address.clone(),
            config.keeper_address.clone(),
            config.rpc_url.clone(),
            config.backup_rpc_urls.clone(),
            config.private_key.clone(),
            config.max_rpc_retries,
            gas_config,
            config.simulate_before_execute,
            config.min_profit_threshold_usd,
            config.dry_run_mode,
        )
        .context("Failed to initialise ChainClientV2")?,
    );

    info!(
        fee_currency = ?config.fee_currency_address,
        gas_limit    = config.gas_limit,
        "ChainClientV2 initialised"
    );

    // 5. OHLC / price feed
    let initial_ohlc = OhlcData::placeholder(config.initial_high, config.initial_low);
    let shared_ohlc: SharedOhlc = Arc::new(RwLock::new(initial_ohlc));

    let kline_params = KlineParams {
        symbol:   config.ohlc_symbol.clone(),
        interval: config.ohlc_interval.clone(),
        limit:    config.ohlc_candle_limit,
    };

    {
        let shared   = Arc::clone(&shared_ohlc);
        let params   = kline_params.clone();
        let secs     = config.ohlc_refresh_secs;
        let retries  = config.ohlc_max_retries;
        // FIX [PERF]: pass the shared client instead of building a new one
        let client   = Arc::clone(&http_client);
        tokio::spawn(async move {
            ohlc_updater_task(shared, params, secs, retries, client).await;
        });
    }

    info!(
        symbol       = %config.ohlc_symbol,
        interval     = %config.ohlc_interval,
        refresh_secs = config.ohlc_refresh_secs,
        "OHLC updater started"
    );

    // 6. Wait for first OHLC data
    info!("⏳ Waiting for first OHLC update…");
    wait_for_first_ohlc_update(&shared_ohlc, config.initial_high).await;
    info!("✅ Data synchronised — keeper loop starting");

    // 7. Build KeeperState
    let keeper_state = Arc::new(KeeperState {
        ohlc:                    shared_ohlc,
        chain_client:            Arc::clone(&chain_client),
        ws_server:               Arc::clone(&ws_server),
        config:                  Arc::clone(&config),
        last_execution_unix_sec: Arc::new(AtomicU64::new(0)),
        circuit_breaker_active:  Arc::new(RwLock::new(false)),
        http_client,
    });

    // 8. Spawn parallel tasks
    let keeper_handle = tokio::spawn(run_keeper_loop(Arc::clone(&keeper_state)));
    let health_handle = tokio::spawn(run_health_monitor(Arc::clone(&keeper_state)));

    let handles = vec![keeper_handle, health_handle];
    handle_shutdown(Arc::clone(&keeper_state), handles).await?;

    info!("👋 Keeper shutdown complete");
    Ok(())
}

// ─────────────────────────────────────────────────────────────
//  Keeper loop
// ─────────────────────────────────────────────────────────────

#[instrument(skip(state))]
async fn run_keeper_loop(state: Arc<KeeperState>) {
    let mut ticker = tokio::time::interval(
        Duration::from_millis(state.config.poll_interval_ms)
    );
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;

        let t0 = std::time::Instant::now();

        match run_keeper_cycle(&state).await {
            Ok(_) => {
                debug!(cycle_ms = t0.elapsed().as_millis(), "Cycle completed");

                // FIX [LOW]: atomic store — no async lock
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                state.last_execution_unix_sec.store(now, Ordering::Relaxed);
            }
            Err(e) => {
                error!(error = %e, "Keeper cycle failed");
                state.ws_server.broadcast(create_update_payload("ERROR", json!({
                    "message": e.to_string(),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })));
            }
        }
    }
}

#[instrument(skip(state))]
async fn run_keeper_cycle(state: &Arc<KeeperState>) -> Result<()> {
    // 1. OHLC snapshot
    let ohlc = state.ohlc.read().await.clone();

    // 2. Staleness guard — activates circuit breaker on stale data
    if !ohlc.is_fresh(900) {
        warn!("OHLC data is stale — activating safety circuit breaker");
        *state.circuit_breaker_active.write().await = true;

        state.ws_server.broadcast(create_update_payload("CIRCUIT_BREAKER", json!({
            "reason": "STALE_DATA",
            "is_active": true
        })));

        return Err(anyhow::anyhow!("OHLC data stale — trading suspended"));
    }

    // Reset stale CB once data is fresh again
    {
        let mut cb = state.circuit_breaker_active.write().await;
        if *cb {
            *cb = false;
            info!("Circuit breaker deactivated — fresh OHLC data available");
        }
    }

    // 3. Aggregated price  (FIX [CRITICAL]: uses state.http_client, not a new one)
    let aggregated = fetch_aggregated_price(
        &state.http_client,
        &state.config.ohlc_symbol,
    ).await?;
    let current_price = aggregated.price;

    state.ws_server.broadcast(create_update_payload("PRICE_UPDATE", json!({
        "price":             current_price,
        "sources":           aggregated.sources_used.len(),
        "deviation_warning": aggregated.price_deviation_warning,
        "timestamp":         chrono::Utc::now().to_rfc3339()
    })));

    // 4. ATR context + grid computation
    let atr_ctx_opt = ohlc.get_last_atr()
        .map(|v| atr::atr_context(v, current_price));

    let grid_result = compute_grid_auto(&ohlc, current_price, atr_ctx_opt.as_ref());

    if grid_result.circuit_breaker_triggered {
        warn!(
            price    = current_price,
            cb_price = grid_result.circuit_breaker_price,
            "Fibonacci circuit breaker triggered"
        );
        state.ws_server.broadcast(create_update_payload("CIRCUIT_BREAKER", json!({
            "reason":        "FIBONACCI_CB",
            "current_price": current_price,
            "cb_price":      grid_result.circuit_breaker_price,
            "ratio":         0.786,
            "is_active":     true
        })));
    }

    // 5. Fetch active on-chain grids
    let active_grids = state.chain_client.get_active_grids().await
        .context("Failed to fetch active grids")?;

    if active_grids.is_empty() {
        debug!("No active grids");
        return Ok(());
    }


    info!(grid_count = active_grids.len(), "Processing active grids");

    // 6. Process each grid
    for on_chain_grid in &active_grids {
        state.ws_server.broadcast(create_update_payload("GRID_STATUS", json!({
            "grid_id":     on_chain_grid.id,
            "lower_price": format_units(on_chain_grid.lower_price),
            "upper_price": format_units(on_chain_grid.upper_price),
            "level_count": on_chain_grid.levels.len(),
            "active":      on_chain_grid.active
        })));

        for grid_level in &on_chain_grid.levels {
            if grid_level.filled { continue; }

            // BUG-8 FIX: Trigger based on the on-chain level's own price, not
            // Fibonacci float prices derived from OHLC data.  The two price sets
            // are completely unrelated — using Fibonacci prices caused the keeper
            // to fire executeGrid transactions that the contract would immediately
            // revert with "GridV2: price condition not met", burning gas with no fills.
            let level_price_f64 = format_units(grid_level.price);

            let triggered = if grid_level.is_buy {
                current_price <= level_price_f64
            } else {
                current_price >= level_price_f64
            };

            if !triggered { continue; }

            // Fibonacci circuit breaker: suspend buy orders when the price
            // breaks below the 78.6% retracement (statistically indicative of
            // a trend reversal).  Sell orders are always allowed so the bot can
            // continue exiting open positions during a downtrend.
            if grid_level.is_buy && grid_result.circuit_breaker_triggered {
                warn!(
                    grid_id     = on_chain_grid.id,
                    level_index = grid_level.index,
                    level_price = level_price_f64,
                    cb_price    = grid_result.circuit_breaker_price,
                    "Buy blocked by Fibonacci circuit breaker (price below 78.6% level)"
                );
                continue;
            }

            info!(
                grid_id     = on_chain_grid.id,
                level_index = grid_level.index,
                side        = if grid_level.is_buy { "BUY" } else { "SELL" },
                level_price = level_price_f64,
                current_price,
                "🎯 Trade trigger"
            );

            match state.chain_client.execute_grid(on_chain_grid.id, grid_level.index).await {
                Ok(result) => {
                    info!(
                        tx_hash  = %result.tx_hash,
                        gas_used = result.gas_used,
                        "✅ Trade executed"
                    );
                    state.ws_server.broadcast(create_update_payload("TRANSACTION", json!({
                        "id":        format!("{}-{}", on_chain_grid.id, grid_level.index),
                        "hash":      result.tx_hash.to_string(),
                        "type":      if grid_level.is_buy { "BUY" } else { "SELL" },
                        "price":     current_price,
                        "level_price": level_price_f64,
                        "gas_used":  result.gas_used,
                        "status":    "SUCCESS",
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })));
                }
                Err(e) => {
                    error!(
                        grid_id     = on_chain_grid.id,
                        level_index = grid_level.index,
                        error       = %e,
                        "❌ Trade execution failed"
                    );
                    state.ws_server.broadcast(create_update_payload("TRANSACTION", json!({
                        "id":        format!("{}-{}", on_chain_grid.id, grid_level.index),
                        "type":      if grid_level.is_buy { "BUY" } else { "SELL" },
                        "price":     current_price,
                        "level_price": level_price_f64,
                        "status":    "FAILED",
                        "error":     e.to_string(),
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })));
                }
            }
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────
//  Health monitor
// ─────────────────────────────────────────────────────────────

#[instrument(skip(state))]
async fn run_health_monitor(state: Arc<KeeperState>) {
    let mut ticker = tokio::time::interval(Duration::from_secs(30));

    loop {
        ticker.tick().await;

        let health = state.chain_client.get_health_report().await;

        // FIX [LOW]: atomic load — no async read lock
        let last_secs = state.last_execution_unix_sec.load(Ordering::Relaxed);
        let now_secs  = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let elapsed = if last_secs == 0 { 999 } else { now_secs.saturating_sub(last_secs) };

        let report = json!({
            "chain_client":              health,
            "last_execution_seconds_ago": elapsed,
            "ohlc_fresh":                state.ohlc.read().await.is_fresh(900),
            "timestamp":                 chrono::Utc::now().to_rfc3339()
        });

        state.ws_server.broadcast(create_update_payload("HEALTH", report.clone()));

        if elapsed > 300 {
            warn!(elapsed_secs = elapsed, "No successful execution in 5 minutes");
        }

        debug!(report = %report, "Health check");
    }
}

// ─────────────────────────────────────────────────────────────
//  Graceful shutdown
// ─────────────────────────────────────────────────────────────

async fn handle_shutdown(
    state:   Arc<KeeperState>,
    handles: Vec<tokio::task::JoinHandle<()>>,
) -> Result<()> {
    let mut sigint  = signal::unix::signal(signal::unix::SignalKind::interrupt())?;
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;

    tokio::select! {
        _ = sigint.recv()  => info!("SIGINT received — shutting down"),
        _ = sigterm.recv() => info!("SIGTERM received — shutting down"),
    }

    for h in &handles { h.abort(); }
    for h in handles  { let _ = h.await; }

    state.ws_server.broadcast(create_update_payload("SHUTDOWN", json!({
        "message":   "Keeper shutting down gracefully",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })));

    Ok(())
}

// ─────────────────────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────────────────────

fn setup_logging() -> Result<()> {
    let use_json = std::env::var("JSON_LOGGING")
        .map(|v| v == "true")
        .unwrap_or(false);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    if use_json {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().with_target(false))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().with_target(false).compact())
            .init();
    }

    Ok(())
}

async fn wait_for_first_ohlc_update(shared: &SharedOhlc, initial_high: f64) {
    let start   = std::time::Instant::now();
    let timeout = Duration::from_secs(60);

    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;

        if (shared.read().await.high - initial_high).abs() > f64::EPSILON {
            return;
        }
        if start.elapsed() > timeout {
            warn!("OHLC timeout — proceeding with placeholder values");
            return;
        }
    }
}

/// Convert a U256 wei value to a human-readable f64 (÷ 10¹⁸).
fn format_units(value: alloy::primitives::U256) -> f64 {
    let wei: u128 = value.to();
    wei as f64 / 1e18
}
