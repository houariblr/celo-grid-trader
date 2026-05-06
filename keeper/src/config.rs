use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub ws_url: String,
    pub private_key: String,
    pub contract_address: String,
    pub keeper_address: String,
    pub poll_interval_ms: u64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            rpc_url: env::var("RPC_URL")
                .unwrap_or("https://forno.celo.org".to_string()),
            ws_url: env::var("WS_URL")
                .unwrap_or("wss://forno.celo.org/ws".to_string()),
            private_key: env::var("PRIVATE_KEY")?,
            contract_address: env::var("CONTRACT_ADDRESS")?,
            keeper_address: env::var("KEEPER_ADDRESS")?,
            poll_interval_ms: env::var("POLL_INTERVAL_MS")
                .unwrap_or("5000".to_string())
                .parse()?,
        })
    }
}
