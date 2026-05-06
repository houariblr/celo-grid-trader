use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    sol,
    rpc::types::Filter,
};
use anyhow::Result;
use std::str::FromStr;
use tracing::info;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    GridTrading,
    "../contract/out/GridTrading.sol/GridTradingV2.json"
);

#[derive(Debug, Clone)]
pub struct GridLevelOnChain {
    pub index: usize,
    pub price: U256,
    pub filled: bool,
    pub is_buy: bool,
}

#[derive(Debug, Clone)]
pub struct GridOnChain {
    pub id: u64,
    pub owner: Address,
    pub base_token: Address,
    pub quote_token: Address,
    pub lower_price: U256,
    pub upper_price: U256,
    pub grid_count: U256,
    pub active: bool,
    pub levels: Vec<GridLevelOnChain>,
}

pub struct ChainClient {
    pub contract_address: Address,
    pub provider_url: String,
    pub private_key: String,
}

impl ChainClient {
    pub fn new(
        contract_address: String,
        provider_url: String,
        private_key: String,
    ) -> Result<Self> {
        Ok(Self {
            contract_address: Address::from_str(&contract_address)?,
            provider_url,
            private_key,
        })
    }

    pub async fn get_active_grids(&self) -> Result<Vec<GridOnChain>> {
        let provider = ProviderBuilder::new()
            .connect_http(self.provider_url.parse()?);

        let contract = GridTrading::new(self.contract_address, &provider);

        let latest_block = provider.get_block_number().await?;
        let from_block = latest_block.saturating_sub(10_000);

        let filter = Filter::new()
            .address(self.contract_address)
            .from_block(from_block)
            .to_block(latest_block);

        let logs = provider.get_logs(&filter).await?;

        let mut grid_ids: Vec<u64> = Vec::new();
        for log in logs {
            if let Ok(event) = log.log_decode::<GridTrading::GridCreated>() {
                let id = event.data().gridId.to::<u64>();
                if !grid_ids.contains(&id) {
                    grid_ids.push(id);
                }
            }
        }

        info!("Found {} grid(s) from events", grid_ids.len());

        let mut grids = Vec::new();
        for id in grid_ids {
            let uid = U256::from(id);

            let grid_data = match contract.grids(uid).call().await {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!("Failed to fetch grid {}: {}", id, e);
                    continue;
                }
            };

            if !grid_data.active {
                continue;
            }

            let levels_data = match contract.getGridLevels(uid).call().await {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("Failed to fetch levels for grid {}: {}", id, e);
                    continue;
                }
            };

            let levels: Vec<GridLevelOnChain> = levels_data
                .iter()
                .enumerate()
                .map(|(i, l)| GridLevelOnChain {
                    index: i,
                    price: l.price,
                    filled: l.filled,
                    is_buy: l.isBuy,
                })
                .collect();

            grids.push(GridOnChain {
                id,
                owner: grid_data.owner,
                base_token: grid_data.baseToken,
                quote_token: grid_data.quoteToken,
                lower_price: grid_data.lowerPrice,
                upper_price: grid_data.upperPrice,
                grid_count: grid_data.gridCount,
                active: grid_data.active,
                levels,
            });
        }

        Ok(grids)
    }

    pub async fn execute_grid(
        &self,
        grid_id: u64,
        level_index: usize,
        current_price_wei: U256,
    ) -> Result<String> {
        let signer: PrivateKeySigner = self.private_key.parse()?;
        let wallet = EthereumWallet::from(signer);

        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(self.provider_url.parse()?);

        let contract = GridTrading::new(self.contract_address, provider);

        let tx_hash = contract
            .executeGrid(
                U256::from(grid_id),
                U256::from(level_index),
                
            )
            .send()
            .await?
            .watch()
            .await?;

        let hash = tx_hash.to_string();
        info!("✅ executeGrid tx: {}", hash);
        Ok(hash)
    }
}
