#[derive(Debug, Clone)]
pub struct GridLevel {
    pub index: u64,
    pub price: f64,
    pub filled: bool,
    pub is_buy: bool,
}

#[derive(Debug, Clone)]
pub struct Grid {
    pub id: u64,
    pub owner: String,
    pub lower_price: f64,
    pub upper_price: f64,
    pub grid_count: u64,
    pub active: bool,
    pub levels: Vec<GridLevel>,
}

pub struct GridEngine;

impl GridEngine {
    pub fn find_executable_levels(
        grid: &Grid,
        current_price: f64,
    ) -> Vec<u64> {
        let mut to_execute = Vec::new();

        for level in &grid.levels {
            if level.filled {
                continue;
            }

            let should_execute = if level.is_buy {
                current_price <= level.price
            } else {
                current_price >= level.price
            };

            if should_execute {
                tracing::info!(
                    "Grid {} Level {}: {} at ${:.4} (current: ${:.4})",
                    grid.id,
                    level.index,
                    if level.is_buy { "BUY" } else { "SELL" },
                    level.price,
                    current_price
                );
                to_execute.push(level.index);
            }
        }

        to_execute
    }

    pub fn wei_to_f64(wei: u128) -> f64 {
        wei as f64 / 1e18
    }

    pub fn f64_to_wei(price: f64) -> u128 {
        (price * 1e18) as u128
    }
}
