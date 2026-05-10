// ============================================================
//  create_grid.rs  –  إنشاء Grid جديد على Celo Sepolia
//  يتفاعل مباشرة مع العقد الذكي
// ============================================================

use celo_grid_keeper_v2::chain_v2::{ChainClient, GasConfig};
use celo_grid_keeper_v2::config::Config;

use anyhow::{Context, Result};
use colored::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("{}", "╔════════════════════════════════════════════════════════════╗".bright_blue());
    println!("{}", "║     Create New Grid - Celo Grid Trading V2                 ║".bright_blue());
    println!("{}", "╚════════════════════════════════════════════════════════════╝".bright_blue());

    // تحميل الإعدادات
    let config = Config::from_env().context("Failed to load .env")?;
    
    println!("\n{}", "📋 Current Configuration:".bold());
    println!("   RPC URL: {}", config.rpc_url.dimmed());
    println!("   Keeper: {}", config.keeper_address);
    println!("   Contract: {}", config.contract_address);
    
    // إعداد ChainClient
    let gas_config = GasConfig {
        gas_limit: 500000, // أعلى للـ createGrid
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        fee_currency: config.fee_currency_address.as_ref()
            .and_then(|addr| addr.parse().ok()),
        use_legacy_tx: config.use_legacy_tx,
    };
    
    let chain = ChainClient::new(
        config.contract_address.clone(),
        config.keeper_address.clone(),
        config.rpc_url.clone(),
        config.backup_rpc_urls.clone(),
        config.private_key.clone(),
        3,
        gas_config,
        true,  // simulate_before_execute
        0.0,   // min_profit_usd (لا يهم للـ create)
        false, // DRY_RUN_MODE = false (تنفيذ حقيقي)
    ).context("Failed to initialize ChainClient")?;
    
    println!("\n{}", "✅ Connected to Celo Sepolia".green());
    
    // جلب الـ Grids الحالية
    match chain.get_active_grids().await {
        Ok(grids) => {
            println!("\n{}", format!("📊 You have {} active grid(s)", grids.len()).cyan());
            for g in &grids {
                let lower = format_units(g.lower_price);
                let upper = format_units(g.upper_price);
                println!("   Grid #{}: {:.4} - {:.4} USDT ({} levels)", 
                    g.id, lower, upper, g.levels.len());
            }
        }
        Err(e) => {
            println!("{} Warning: Could not fetch grids: {}", "⚠️".yellow(), e);
        }
    }
    
    // اقتراح نطاق Grid جديد
    let current_price = 0.0932f64; // يمكن جلبه ديناميكياً
    let suggested_lower = current_price * 0.95; // -5%
    let suggested_upper = current_price * 1.05; // +5%
    
    println!("\n{}", "🎯 Suggested Grid Range (based on current price ~$0.0932):".bold());
    println!("   Lower: {:.4} USDT", suggested_lower);
    println!("   Upper: {:.4} USDT", suggested_upper);
    println!("   Grid Count: 5 levels");
    
    // تأكيد من المستخدم
    println!("\n{}", "⚠️  This will create a NEW grid on Sepolia (costs gas)".yellow().bold());
    print!("{} Do you want to proceed? (yes/no): ", "➤".cyan());
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    if input.trim().to_lowercase() != "yes" {
        println!("{} Aborted.", "❌".red());
        return Ok(());
    }
    
    // TODO: تنفيذ createGrid (يتطلب ABI function)
    println!("\n{}", "📝 Note: createGrid function needs to be implemented".yellow());
    println!("   This requires calling the contract's createGrid function:");
    println!("   - baseToken: CELO address on Sepolia");
    println!("   - quoteToken: cUSD address on Sepolia");
    println!("   - lowerPrice: {} (wei)", (suggested_lower * 1e18) as u64);
    println!("   - upperPrice: {} (wei)", (suggested_upper * 1e18) as u64);
    println!("   - gridCount: 5");
    
    println!("\n{}", "🔗 You can create grid via:".cyan());
    println!("   1. Frontend dApp");
    println!("   2. Celo Sepolia Explorer");
    println!("   3. Cast CLI: cast send <contract> \"createGrid(...)\" ...");
    
    Ok(())
}

fn format_units(value: alloy::primitives::U256) -> f64 {
    let wei: u128 = value.to();
    wei as f64 / 1e18
}
