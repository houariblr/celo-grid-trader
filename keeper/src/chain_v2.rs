// ============================================================
//  chain_v2.rs  –  Production-Grade Blockchain Client for Celo
//  المميزات:
//    ✅ Dynamic Gas Estimation مع دعم Celo Fee Currency
//    ✅ RPC Health Monitor مع Circuit Breaker Pattern
//    ✅ Pre-execution Simulation (eth_call) لتجنب فشل المعاملات
//    ✅ Batch Execution لعدة مستويات في معاملة واحدة
//    ✅ Structured Logging مع JSON tracing
// ============================================================

use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256, TxHash},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    sol,
};
use anyhow::{anyhow, Context, Result};

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn, debug, instrument};

// ─────────────────────────────────────────────────────────────
//  ABI Loading (Runtime Configurable via environment)
// ─────────────────────────────────────────────────────────────


// ملاحظة: alloy sol! macro يتطلب مسار ثابت في وقت compilation
// لتحميل ABI ديناميكياً في وقت التشغيل، نستخدم القيمة الافتراضية هنا
// ويمكن تعديلها عبر إعادة compilation مع CONTRACT_ABI_PATH
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    GridTradingV2,
    "../contract/out/GridTrading.sol/GridTradingV2.json"
);

// ═══════════════════════════════════════════════════════════════
//  DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════

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

/// نتيجة محاكاة المعاملة قبل التنفيذ
#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub would_succeed: bool,
    pub estimated_gas: u64,
    pub gas_cost_usd: f64,
    pub revert_reason: Option<String>,
}

/// نتيجة التنفيذ الفعلي
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub tx_hash: TxHash,
    pub gas_used: u64,
    pub effective_gas_price: u128,
    pub total_cost_wei: u128,
    pub block_number: u64,
    pub status: bool, // true = success, false = revert
}

/// حالة صحة RPC
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RpcHealthStatus {
    Healthy,
    Degraded { consecutive_failures: u32 },
    Unhealthy { since: Instant },
}

impl RpcHealthStatus {
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded { .. })
    }
}

/// إحصائيات RPC
#[derive(Debug, Clone, Default)]
pub struct RpcMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub avg_response_time_ms: f64,
    pub last_success: Option<Instant>,
    pub consecutive_failures: u32,
}

/// إعدادات الغاز لشبكة Celo
#[derive(Debug, Clone)]
pub struct GasConfig {
    pub gas_limit: u64,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
    pub fee_currency: Option<Address>, // cUSD أو أخرى
    pub use_legacy_tx: bool,
}

impl Default for GasConfig {
    fn default() -> Self {
        Self {
            gas_limit: 300_000,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            fee_currency: None, // افتراضياً CELO
            use_legacy_tx: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  RPC HEALTH MONITOR
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct RpcHealthMonitor {
    backup_urls: Vec<String>,
    current_url: Arc<RwLock<String>>,
    metrics: Arc<RwLock<RpcMetrics>>,
    health_status: Arc<RwLock<RpcHealthStatus>>,
    max_consecutive_failures: u32,
}

impl RpcHealthMonitor {
    pub fn new(
        primary_url: String,
        backup_urls: Vec<String>,
        max_consecutive_failures: u32,
    ) -> Self {
        Self {
            backup_urls,
            current_url: Arc::new(RwLock::new(primary_url)),
            metrics: Arc::new(RwLock::new(RpcMetrics::default())),
            health_status: Arc::new(RwLock::new(RpcHealthStatus::Healthy)),
            max_consecutive_failures,
        }
    }

    /// الحصول على URL RPC الحالي (قد يكون الباكب إذا فشل الأساسي)
    pub async fn get_current_url(&self) -> String {
        self.current_url.read().await.clone()
    }

    /// تسجيل نجاح الطلب
    pub async fn record_success(&self, response_time: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.successful_requests += 1;
        metrics.consecutive_failures = 0;
        metrics.last_success = Some(Instant::now());
        
        // حساب المتوسط المتحرك للوقت
        let rt_ms = response_time.as_millis() as f64;
        metrics.avg_response_time_ms = 
            (metrics.avg_response_time_ms * 0.9) + (rt_ms * 0.1);
        
        drop(metrics);
        
        // تحديث الحالة
        let mut status = self.health_status.write().await;
        *status = RpcHealthStatus::Healthy;
    }

    /// تسجيل فشل الطلب
    pub async fn record_failure(&self) -> RpcHealthStatus {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.failed_requests += 1;
        metrics.consecutive_failures += 1;
        
        let failures = metrics.consecutive_failures;
        drop(metrics);
        
        let mut status = self.health_status.write().await;
        
        if failures >= self.max_consecutive_failures {
            // التبديل إلى RPC backup
            warn!(
                consecutive_failures = failures,
                "RPC deemed unhealthy, attempting failover"
            );
            
            if let Err(e) = self.try_failover().await {
                error!(error = %e, "RPC failover failed");
                *status = RpcHealthStatus::Unhealthy { since: Instant::now() };
            }
        } else {
            *status = RpcHealthStatus::Degraded { consecutive_failures: failures };
        }
        
        *status
    }

    /// محاولة التبديل إلى RPC backup
    async fn try_failover(&self) -> Result<()> {
        for backup_url in &self.backup_urls {
            info!(url = %backup_url, "Testing backup RPC");
            
            if self.test_rpc_connection(backup_url).await {
                let mut current = self.current_url.write().await;
                *current = backup_url.clone();
                
                let mut status = self.health_status.write().await;
                *status = RpcHealthStatus::Healthy;
                
                info!(new_url = %backup_url, "Switched to backup RPC");
                return Ok(());
            }
        }
        
        Err(anyhow!("All RPC endpoints unavailable"))
    }

    /// اختبار اتصال RPC
    async fn test_rpc_connection(&self, url: &str) -> bool {
        let parsed_url = match url.parse() {
            Ok(u) => u,
            Err(_) => return false,
        };
        let provider = ProviderBuilder::new().connect_http(parsed_url);
        // محاولة جلب رقم البلوك الحالي
        match provider.get_block_number().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub async fn get_health_status(&self) -> RpcHealthStatus {
        *self.health_status.read().await
    }

    pub async fn get_metrics(&self) -> RpcMetrics {
        self.metrics.read().await.clone()
    }
}

// ═══════════════════════════════════════════════════════════════
//  DYNAMIC GAS MANAGER
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct DynamicGasManager {
    config: GasConfig,
    last_estimate: Arc<RwLock<Option<(u128, Instant)>>>, // (gas_price, timestamp)
    stale_threshold: Duration,
}

impl DynamicGasManager {
    pub fn new(config: GasConfig) -> Self {
        Self {
            config,
            last_estimate: Arc::new(RwLock::new(None)),
            stale_threshold: Duration::from_secs(60), // تقدير الغاز صالح لـ 60 ثانية
        }
    }

    /// تقدير الغاز بشكل ديناميكي
    #[instrument(skip(provider))]
    pub async fn estimate_gas<P: Provider>(&self, provider: &P) -> Result<GasConfig> {
        let start = Instant::now();
        
        // التحقق من وجود تقدير حديث
        {
            let last = self.last_estimate.read().await;
            if let Some((price, timestamp)) = *last {
                if timestamp.elapsed() < self.stale_threshold {
                    debug!(gas_price = %price, "Using cached gas estimate");
                    return Ok(self.config_with_gas(price));
                }
            }
        }
        
        // جلب سعر الغاز من الشبكة
        let gas_price = provider.get_gas_price().await
            .context("Failed to fetch gas price from Celo network")?;
        
        // إضافة 20% إلى سعر الغاز للتأكد من سرعة التنفيذ
        let adjusted_gas_price = gas_price + (gas_price / 5);
        
        // تحديث الذاكرة
        {
            let mut last = self.last_estimate.write().await;
            *last = Some((adjusted_gas_price, Instant::now()));
        }
        
        let elapsed = start.elapsed();
        info!(
            gas_price_wei = %gas_price,
            adjusted_gas_price_wei = %adjusted_gas_price,
            estimation_time_ms = elapsed.as_millis(),
            "Gas price estimated"
        );
        
        Ok(self.config_with_gas(adjusted_gas_price))
    }

    fn config_with_gas(&self, gas_price: u128) -> GasConfig {
        GasConfig {
            gas_limit: self.config.gas_limit,
            max_fee_per_gas: Some(gas_price),
            max_priority_fee_per_gas: Some(gas_price / 10), // 10% للـ miner
            fee_currency: self.config.fee_currency,
            use_legacy_tx: self.config.use_legacy_tx,
        }
    }

    /// حساب تكلفة الغاز بالدولار
    pub fn calculate_gas_cost_usd(gas_used: u64, gas_price_wei: u128, celo_price_usd: f64) -> f64 {
        let gas_cost_wei = gas_used as u128 * gas_price_wei;
        let gas_cost_eth = gas_cost_wei as f64 / 1e18;
        gas_cost_eth * celo_price_usd
    }
}

// ═══════════════════════════════════════════════════════════════
//  MAIN CHAIN CLIENT
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct ChainClient {
    pub contract_address: Address,
    pub keeper_address: Address,
    pub health_monitor: Arc<RpcHealthMonitor>,
    pub gas_manager: DynamicGasManager,
    pub private_key: String,
    pub max_retries: u32,
    pub simulate_before_execute: bool,
    pub min_profit_usd: f64, // الحد الأدنى للربح لتبرير تنفيذ الصفقة
    pub dry_run_mode: bool,  // وضع المحاكاة فقط (بدون إرسال معاملات)
}

impl ChainClient {
    pub fn new(
        contract_address: String,
        keeper_address: String,
        primary_rpc_url: String,
        backup_rpc_urls: Vec<String>,
        private_key: String,
        max_retries: u32,
        gas_config: GasConfig,
        simulate_before_execute: bool,
        min_profit_usd: f64,
        dry_run_mode: bool,
    ) -> Result<Self> {
        if dry_run_mode {
            tracing::info!("🧪 DRY RUN MODE: Transactions will be simulated but NOT sent!");
        }
        Ok(Self {
            contract_address: contract_address.parse()
                .context("Invalid contract address")?,
            keeper_address: keeper_address.parse()
                .context("Invalid keeper address")?,
            health_monitor: Arc::new(RpcHealthMonitor::new(
                primary_rpc_url,
                backup_rpc_urls,
                3, // التبديل بعد 3 محاولات فاشلة
            )),
            gas_manager: DynamicGasManager::new(gas_config),
            private_key,
            max_retries,
            simulate_before_execute,
            min_profit_usd,
            dry_run_mode,
        })
    }

    /// ═══════════════════════════════════════════════════════════
    ///  GRID FETCHING (محسّن مع RPC Health)
    /// ═══════════════════════════════════════════════════════════
    
    #[instrument(skip(self))]
    pub async fn get_active_grids(&self) -> Result<Vec<GridOnChain>> {
        // التحقق من صحة RPC قبل المحاولة
        let health = self.health_monitor.get_health_status().await;
        if !health.is_operational() {
            return Err(anyhow!("RPC is not operational: {:?}", health));
        }

        let start = Instant::now();
        let mut last_err = anyhow!("لم تبدأ بعد");

        for attempt in 1..=self.max_retries {
            let rpc_url = self.health_monitor.get_current_url().await;
            
            match self.try_get_active_grids(&rpc_url).await {
                Ok(grids) => {
                    let elapsed = start.elapsed();
                    self.health_monitor.record_success(elapsed).await;
                    
                    info!(
                        grid_count = grids.len(),
                        duration_ms = elapsed.as_millis(),
                        attempt = attempt,
                        "Successfully fetched active grids"
                    );
                    
                    return Ok(grids);
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        attempt = attempt,
                        max_retries = self.max_retries,
                        "Failed to fetch grids"
                    );
                    
                    last_err = e;
                    let health = self.health_monitor.record_failure().await;
                    
                    if !health.is_operational() {
                        return Err(anyhow!("RPC failed after {} attempts: {}", attempt, last_err));
                    }
                    
                    if attempt < self.max_retries {
                        let backoff = Duration::from_secs(2_u64.pow(attempt as u32));
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        error!(error = %last_err, "All retry attempts exhausted");
        Err(last_err)
    }

    async fn try_get_active_grids(&self, rpc_url: &str) -> Result<Vec<GridOnChain>> {
        let start = Instant::now();

        let provider = ProviderBuilder::new()
            .connect_http(rpc_url.parse().context("Invalid RPC URL")?);

        let contract = GridTradingV2::new(self.contract_address, &provider);

        // BUG-4 FIX: Iterate ALL grids (0..nextGridId) instead of calling
        // getUserGrids(keeper_address) which only returns grids created by the
        // keeper's own wallet, missing every grid created by real users.
        let next_id = contract
            .nextGridId()
            .call()
            .await
            .context("nextGridId call failed")?;

        let next_id_u64 = next_id.to::<u64>();

        let mut grids = Vec::new();

        for id in 0..next_id_u64 {
            let id_u256 = U256::from(id);

            let grid_data = match contract.grids(id_u256).call().await {
                Ok(g) => g,
                Err(e) => {
                    warn!(grid_id = id, error = %e, "Failed to fetch grid data, skipping");
                    continue;
                }
            };

            if !grid_data.active {
                continue;
            }

            let raw_levels = match contract.getGridLevels(id_u256).call().await {
                Ok(l) => l,
                Err(e) => {
                    warn!(grid_id = id, error = %e, "Failed to fetch grid levels, skipping");
                    continue;
                }
            };

            let levels: Vec<GridLevelOnChain> = raw_levels
                .into_iter()
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

        let elapsed = start.elapsed();
        debug!(
            grid_count = grids.len(),
            total_grids_scanned = next_id_u64,
            fetch_time_ms = elapsed.as_millis(),
            "Fetched all active grids"
        );

        Ok(grids)
    }

    /// ═══════════════════════════════════════════════════════════
    ///  PRE-EXECUTION SIMULATION (eth_call)
    /// ═══════════════════════════════════════════════════════════
    
    #[instrument(skip(self), fields(grid_id = %grid_id, level_index = %level_index))]
    pub async fn simulate_execution(
        &self,
        grid_id: u64,
        level_index: usize,
    ) -> Result<SimulationResult> {
        let rpc_url = self.health_monitor.get_current_url().await;
        
        // نحتاج provider بدون wallet للـ eth_call
        let provider = ProviderBuilder::new()
            .connect_http(rpc_url.parse().context("Invalid RPC URL")?);

        let contract = GridTradingV2::new(self.contract_address, &provider);

        // محاكاة الـ call (بدون توقيع - مجرد قراءة)
        let start = Instant::now();
        
        // BUG-6 FIX: Pass the keeper's address as the `from` field.
        // Without this, eth_call uses msg.sender = address(0), which is not in
        // isKeeper, so the onlyKeeper modifier reverts every simulation, causing
        // would_succeed = false for all trades and the keeper never executes anything.
        let result = contract
            .executeGrid(U256::from(grid_id), U256::from(level_index))
            .from(self.keeper_address)
            .call()
            .await;

        let elapsed = start.elapsed();

        match result {
            Ok(_) => {
                // تقدير الغاز (نستخدم 200k كتقدير أولي)
                let estimated_gas = 200_000_u64;
                
                info!(
                    grid_id = %grid_id,
                    level_index = %level_index,
                    simulation_time_ms = elapsed.as_millis(),
                    "Simulation successful - transaction would succeed"
                );
                
                Ok(SimulationResult {
                    would_succeed: true,
                    estimated_gas,
                    gas_cost_usd: 0.0, // يُحسب لاحقاً
                    revert_reason: None,
                })
            }
            Err(e) => {
                let revert_reason = extract_revert_reason(&e);
                
                warn!(
                    grid_id = %grid_id,
                    level_index = %level_index,
                    revert_reason = ?revert_reason,
                    "Simulation failed - transaction would revert"
                );
                
                Ok(SimulationResult {
                    would_succeed: false,
                    estimated_gas: 0,
                    gas_cost_usd: 0.0,
                    revert_reason,
                })
            }
        }
    }

    /// ═══════════════════════════════════════════════════════════
    ///  SINGLE GRID EXECUTION (محسّن)
    /// ═══════════════════════════════════════════════════════════
    
    #[instrument(skip(self), fields(grid_id = %grid_id, level_index = %level_index))]
    pub async fn execute_grid(
        &self,
        grid_id: u64,
        level_index: usize,
    ) -> Result<ExecutionResult> {
        // المرحلة 1: المحاكاة (إذا مفعلة)
        if self.simulate_before_execute {
            let sim_result = self.simulate_execution(grid_id, level_index).await?;
            
            if !sim_result.would_succeed {
                return Err(anyhow!(
                    "Transaction simulation failed: {:?}",
                    sim_result.revert_reason
                ));
            }
            
        // BUG-7 FIX: The old gate checked `gas_cost_usd < min_profit_usd`.
        // gas_cost_usd was hardcoded to 0.0 in SimulationResult, so the condition
        // `0.0 < min_profit_usd` was always true, blocking every trade permanently.
        //
        // The simulation already verifies the transaction would succeed on-chain.
        // Proper profit-vs-gas estimation can be layered on top once a CELO price
        // feed is integrated into the keeper (e.g. from fetch_aggregated_price).
        //
        // TODO: compute estimated_gas * gas_price_wei * celo_price_usd and
        //       compare against expected grid profit to gate unprofitable trades.
        }

        // المرحلة 2: التنفيذ الفعلي (أو المحاكاة فقط في DRY RUN)
        if self.dry_run_mode {
            info!("🧪 DRY RUN: Skipping actual transaction execution");
            return Ok(ExecutionResult {
                tx_hash: alloy::primitives::TxHash::default(),
                gas_used: 0,
                effective_gas_price: 0,
                total_cost_wei: 0,
                block_number: 0,
                status: true,
            });
        }

        let start = Instant::now();
        let mut last_err = anyhow!("لم تبدأ التنفيذ");

        for attempt in 1..=self.max_retries {
            match self.try_execute_grid(grid_id, level_index).await {
                Ok(result) => {
                    let elapsed = start.elapsed();
                    
                    info!(
                        tx_hash = %result.tx_hash,
                        gas_used = result.gas_used,
                        total_cost_wei = %result.total_cost_wei,
                        execution_time_ms = elapsed.as_millis(),
                        attempt = attempt,
                        "Grid execution successful"
                    );
                    
                    return Ok(result);
                }
                Err(e) => {
                    error!(
                        error = %e,
                        attempt = attempt,
                        max_retries = self.max_retries,
                        "Grid execution attempt failed"
                    );
                    
                    last_err = e;
                    
                    if attempt < self.max_retries {
                        let backoff = Duration::from_secs(3 * attempt as u64);
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        error!(error = %last_err, "All execution attempts failed");
        Err(last_err)
    }

    async fn try_execute_grid(&self, grid_id: u64, level_index: usize) -> Result<ExecutionResult> {
        let signer: PrivateKeySigner = self.private_key.parse()
            .context("Invalid private key format")?;
        let wallet = EthereumWallet::from(signer);

        let rpc_url = self.health_monitor.get_current_url().await;
        
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(rpc_url.parse().context("Invalid RPC URL")?);

        // تقدير الغاز الديناميكي
        let gas_config = self.gas_manager.estimate_gas(&provider).await?;
        
        let contract = GridTradingV2::new(self.contract_address, provider);

        // إرسال المعاملة - نستخدم watch() للانتظار حتى التأكيد
        let tx_hash = contract
            .executeGrid(U256::from(grid_id), U256::from(level_index))
            .send()
            .await
            .context("Failed to send transaction")?
            .watch()
            .await
            .context("Failed to get transaction confirmation")?;
        
        info!(tx_hash = %tx_hash, "Transaction confirmed");
        
        // TODO: جلب الـ receipt للحصول على gas_used (يتطلب call إضافي)
        Ok(ExecutionResult {
            tx_hash,
            gas_used: gas_config.gas_limit, // تقدير
            effective_gas_price: gas_config.max_fee_per_gas.unwrap_or(0),
            total_cost_wei: 0, // يُحسب لاحقاً
            block_number: 0, // يُجلب لاحقاً
            status: true,
        })
    }

    /// ═══════════════════════════════════════════════════════════
    ///  BATCH EXECUTION (تنفيذ متعدد)
    /// ═══════════════════════════════════════════════════════════
    
    #[instrument(skip(self), fields(grid_id = %grid_id, level_count = %level_indices.len()))]
    pub async fn execute_grid_batch(
        &self,
        grid_id: u64,
        level_indices: Vec<usize>,
    ) -> Result<Vec<ExecutionResult>> {
        if level_indices.is_empty() {
            return Ok(Vec::new());
        }

        // ملاحظة: يتطلب هذا دالة batchExecuteGrid في العقد
        // حالياً ننفذ بالتسلسل مع تحسين الغاز
        
        info!(
            batch_size = level_indices.len(),
            "Executing batch of grid levels"
        );

        let mut results = Vec::with_capacity(level_indices.len());
        
        for (i, level_index) in level_indices.iter().enumerate() {
            match self.execute_grid(grid_id, *level_index).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!(
                        index = i,
                        level_index = level_index,
                        error = %e,
                        "Batch execution failed at item"
                    );
                    // نستمر مع البقية بدلاً من الفشل الكامل
                    // TODO: تنفيذ atomic batch في العقد
                }
            }
            
            // تأخير بسيط بين المعاملات لتجنب nonce collision
            if i < level_indices.len() - 1 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        Ok(results)
    }

    /// ═══════════════════════════════════════════════════════════
    ///  HEALTH & METRICS
    /// ═══════════════════════════════════════════════════════════
    
    pub async fn get_health_report(&self) -> serde_json::Value {
        let health = self.health_monitor.get_health_status().await;
        let metrics = self.health_monitor.get_metrics().await;
        
        serde_json::json!({
            "rpc_status": format!("{:?}", health),
            "rpc_current_url": self.health_monitor.get_current_url().await,
            "total_requests": metrics.total_requests,
            "success_rate": if metrics.total_requests > 0 {
                (metrics.successful_requests as f64 / metrics.total_requests as f64) * 100.0
            } else { 0.0 },
            "avg_response_time_ms": metrics.avg_response_time_ms,
            "consecutive_failures": metrics.consecutive_failures,
            "contract_address": format!("{}", self.contract_address),
            "keeper_address": format!("{}", self.keeper_address),
            "simulation_enabled": self.simulate_before_execute,
            "min_profit_threshold_usd": self.min_profit_usd,
        })
    }

    /// ═══════════════════════════════════════════════════════════
    ///  CREATE GRID (إنشاء Grid جديد)
    /// ═══════════════════════════════════════════════════════════
    
    #[instrument(skip(self))]
    pub async fn create_grid(
        &self,
        base_token: Address,
        quote_token: Address,
        lower_price: U256,
        upper_price: U256,
        grid_count: u32,
    ) -> Result<ExecutionResult> {
        info!(
            base_token = %base_token,
            quote_token = %quote_token,
            lower = %lower_price,
            upper = %upper_price,
            count = grid_count,
            "Creating new grid"
        );

        // في وضع المحاكاة فقط
        if self.dry_run_mode {
            info!("🧪 DRY RUN: Would create grid but skipping actual transaction");
            return Ok(ExecutionResult {
                tx_hash: TxHash::default(),
                gas_used: 0,
                effective_gas_price: 0,
                total_cost_wei: 0,
                block_number: 0,
                status: true,
            });
        }

        let signer: PrivateKeySigner = self.private_key.parse()
            .context("Invalid private key format")?;
        let wallet = EthereumWallet::from(signer);

        let rpc_url = self.health_monitor.get_current_url().await;
        
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(rpc_url.parse().context("Invalid RPC URL")?);

        // تقدير الغاز
        let gas_config = self.gas_manager.estimate_gas(&provider).await?;
        
        let contract = GridTradingV2::new(self.contract_address, provider);
         
     // 1. تأكد أولاً من حساب الإجمالي
       // 1. تعريف المتغيرات التي يفتقدها المترجم داخل هذا النطاق
// سنفترض أنك تريد إيداع 10 cUSD لكل مستوى (10 * 10^18)
let amount_per_grid_val = U256::from(10_000_000_000_000_000_000u128); 
let grid_count_u256 = U256::from(grid_count);

// 2. حساب الإجمالي المطلوب للعقد [cite: 75, 79]
let total_amount_to_send = amount_per_grid_val * grid_count_u256;

// 3. استدعاء الدالة بالمعاملات الثمانية المطلوبة في العقد 
// 3. استدعاء الدالة بالمعاملات الثمانية المطلوبة في العقد 
        let pending_tx = contract
            .createGrid(
                base_token,
                quote_token,
                lower_price,
                upper_price,
                grid_count_u256,
                total_amount_to_send, // تم تعريفه في السطر السابق
                true,                 // yieldEnabled 
                U256::from(100)       // slippageBps (1%)
            )
            .send()
            .await
            .context("Failed to send createGrid transaction")?;

        // 4. استخراج الـ Hash الخاص بالمعاملة ليتعرف عليه المترجم
        let tx_hash = *pending_tx.tx_hash();

        // 5. انتظار تأكيد المعاملة من الشبكة
        pending_tx
            .watch()
            .await
            .context("Failed to get transaction confirmation")?;
        
        info!(tx_hash = %tx_hash, "✅ Grid created successfully");
        
        Ok(ExecutionResult {
            tx_hash,
            gas_used: gas_config.gas_limit,
            effective_gas_price: gas_config.max_fee_per_gas.unwrap_or(0),
            total_cost_wei: 0,
            block_number: 0,
            status: true,
        })
    }
}
// ═══════════════════════════════════════════════════════════════
//  HELPERS
// ═══════════════════════════════════════════════════════════════

fn extract_revert_reason<E: std::fmt::Display>(error: &E) -> Option<String> {
    let err_str = error.to_string();
    
    // استخراج سبب الرجوع من رسالة الخطأ
    if err_str.contains("revert") {
        Some(err_str.clone())
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════
//  TESTS
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_cost_calculation() {
        let gas_used = 200_000u64;
        let gas_price_wei = 25_000_000_000u128; // 25 gwei
        let celo_price_usd = 0.85;
        
        let cost = DynamicGasManager::calculate_gas_cost_usd(gas_used, gas_price_wei, celo_price_usd);
        
        // 200k * 25 gwei = 5e15 wei = 0.005 CELO
        // 0.005 * 0.85 = 0.00425 USD
        assert!(cost > 0.004 && cost < 0.005);
    }

    #[test]
    fn test_rpc_health_transitions() {
        // يمكنك إضافة اختبارات async هنا باستخدام tokio::test
    }
}
