use anyhow::Result;
use tracing::info;

#[derive(Debug, Clone)]
pub struct PriceData {
    pub price: f64,
    pub timestamp: u64,
}

pub struct PriceFeed {
    pub rpc_url: String,
}

impl PriceFeed {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }

    pub async fn get_celo_price(&self) -> Result<PriceData> {
        let client = reqwest::Client::builder()
            .user_agent("celo-grid-keeper/1.0")
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        // Binance public API - لا يحتاج API key
        let resp = client
            .get("https://api.binance.com/api/v3/ticker/price?symbol=CELOUSDT")
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let price_str = resp["price"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Unexpected response: {}", resp))?;

        let price: f64 = price_str.parse()?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        info!("CELO/USDT: ${:.4}", price);

        Ok(PriceData { price, timestamp })
    }
}
