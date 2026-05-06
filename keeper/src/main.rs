mod config;
mod price_feed;
mod grid;
mod chain;

use alloy::primitives::U256;
use anyhow::Result;
use tracing::{info, error, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("keeper=info".parse()?)
        )
        .init();

    dotenv::dotenv().ok();

    info!("🚀 Celo Grid Trading Keeper starting...");

    let config = config::Config::from_env()?;
    info!("📋 Contract : {}", config.contract_address);
    info!("👤 Keeper   : {}", config.keeper_address);
    info!("⏱️  Interval : {}ms", config.poll_interval_ms);

    let price_feed = price_feed::PriceFeed::new(config.rpc_url.clone());

    let chain_client = chain::ChainClient::new(
        config.contract_address.clone(),
        config.rpc_url.clone(),
        config.private_key.clone(),
    )?;

    info!("✅ Components initialized — starting main loop");

    loop {
        let price_data = match price_feed.get_celo_price().await {
            Ok(p) => p,
            Err(e) => {
                warn!("⚠️  Price feed error: {}", e);
                tokio::time::sleep(
                    tokio::time::Duration::from_millis(config.poll_interval_ms)
                ).await;
                continue;
            }
        };

        let current_price = price_data.price;
        info!("💰 CELO/USD: ${:.4}", current_price);

        // تحويل السعر إلى U256 مباشرة
        let price_u256 = U256::from(grid::GridEngine::f64_to_wei(current_price));

        let grids = match chain_client.get_active_grids().await {
            Ok(g) => g,
            Err(e) => {
                error!("❌ Failed to fetch grids: {}", e);
                tokio::time::sleep(
                    tokio::time::Duration::from_millis(config.poll_interval_ms)
                ).await;
                continue;
            }
        };

        if grids.is_empty() {
            info!("📭 No active grids");
        }

        for on_chain_grid in grids {
            info!("🔍 Checking Grid #{}", on_chain_grid.id);

            let grid = grid::Grid {
                id: on_chain_grid.id,
                owner: on_chain_grid.owner.to_string(),
                lower_price: grid::GridEngine::wei_to_f64(
                    on_chain_grid.lower_price.to::<u128>()
                ),
                upper_price: grid::GridEngine::wei_to_f64(
                    on_chain_grid.upper_price.to::<u128>()
                ),
                grid_count: on_chain_grid.grid_count.to::<u64>(),
                active: on_chain_grid.active,
                levels: on_chain_grid.levels.iter().map(|l| grid::GridLevel {
                    index: l.index as u64,
                    price: grid::GridEngine::wei_to_f64(l.price.to::<u128>()),
                    filled: l.filled,
                    is_buy: l.is_buy,
                }).collect(),
            };

            let executable = grid::GridEngine::find_executable_levels(
                &grid,
                current_price,
            );

            if executable.is_empty() {
                info!("  ↳ No levels to execute at ${:.4}", current_price);
                continue;
            }

            for level_index in executable {
                info!("  ⚡ Executing level {} on Grid #{}", level_index, on_chain_grid.id);

                match chain_client.execute_grid(
                    on_chain_grid.id,
                    level_index as usize,
                    price_u256,
                ).await {
                    Ok(tx_hash) => info!("  ✅ TX: {}", tx_hash),
                    Err(e) => error!("  ❌ Execute failed: {}", e),
                }
            }
        }

        tokio::time::sleep(
            tokio::time::Duration::from_millis(config.poll_interval_ms)
        ).await;
    }
}
