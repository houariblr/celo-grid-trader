// ============================================================
//  config.rs  –  إعدادات النظام الكاملة
// ============================================================

use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    // ── إعدادات الشبكة ──────────────────────────────────────
    pub rpc_url: String,
    pub ws_server_bind_address: String,
    pub private_key: String,
    pub contract_address: String,
    pub keeper_address: String,

    // ── دورة تشغيل الـ Keeper (بالمللي ثانية) ─────────────
    pub poll_interval_ms: u64,

    // ── إعدادات OHLC الديناميكية ────────────────────────────
    /// رمز الزوج على Binance (مثلاً "CELOUSDT")
    pub ohlc_symbol: String,
    /// الفترة الزمنية للشمعة ("15m" | "1h" | "4h" | "1d")
    pub ohlc_interval: String,
    /// عدد الشموع الأخيرة لحساب القمة/القاع
    pub ohlc_candle_limit: u32,
    /// كم ثانية بين كل تحديث لـ OHLC (900 = 15 دقيقة)
    pub ohlc_refresh_secs: u64,
    /// الحد الأقصى لمحاولات إعادة الجلب عند الفشل
    pub ohlc_max_retries: u32,

    // ── القيم الابتدائية (تُستخدم حتى أول جلب ناجح) ────────
    pub initial_high: f64,
    pub initial_low: f64,

    // ── إعدادات الغاز ────────────────────────────────────────
    pub gas_limit: u64,

    // ── إعدادات ChainClient V2 (Production-Grade) ────────────
    /// مسار ملف ABI للعقد (افتراضي: ../contract/out/GridTrading.sol/GridTradingV2.json)
    pub contract_abi_path: String,
    /// URLs احتياطية للـ RPC (مفصولة بفواصل)
    pub backup_rpc_urls: Vec<String>,
    /// عنوان عملة رسوم الغاز (cUSD) - None = CELO
    pub fee_currency_address: Option<String>,
    /// تفعيل المحاكاة قبل التنفيذ
    pub simulate_before_execute: bool,
    /// الحد الأدنى للربح بالدولار لتبرير تنفيذ الصفقة
    pub min_profit_threshold_usd: f64,
    /// الحد الأقصى لمحاولات إعادة الاتصال بالـ RPC
    pub max_rpc_retries: u32,
    /// استخدام معاملات Legacy بدلاً من EIP-1559
    pub use_legacy_tx: bool,
    
    /// وضع المحاكاة فقط (بدون إرسال معاملات فعلية)
    pub dry_run_mode: bool,
}

impl Config {
    /// يحمّل الإعدادات من متغيرات البيئة (.env)
    pub fn from_env() -> Result<Self> {
        // تحميل ملف .env إن وُجد
        let _ = dotenvy::dotenv();

        Ok(Self {
            // ── الشبكة ──────────────────────────────────────
            rpc_url: require("RPC_URL")?,
            // BUG-17 FIX: The local WebSocket UI server needs a bind address (e.g., 0.0.0.0:8080).
            // Previously we reused the WS_URL variable, which caused it to attempt to bind to
            // wss://celo-sepolia.drpc.org leading to an 'invalid port value' crash.
            ws_server_bind_address: env_or("WS_SERVER_BIND_ADDRESS", "0.0.0.0:8080"),
            private_key: require("PRIVATE_KEY")?,
            contract_address: require("CONTRACT_ADDRESS")?,
            keeper_address: require("KEEPER_ADDRESS")?,

            // ── دورة الـ Keeper ──────────────────────────────
            poll_interval_ms: parse_or("POLL_INTERVAL_MS", 5_000)?,

            // ── OHLC ─────────────────────────────────────────
            ohlc_symbol: env_or("OHLC_SYMBOL", "CELOUSDT"),
            ohlc_interval: env_or("OHLC_INTERVAL", "15m"),
            ohlc_candle_limit: parse_or("OHLC_CANDLE_LIMIT", 50)?,
            ohlc_refresh_secs: parse_or("OHLC_REFRESH_SECS", 900)?, // 15 دقيقة
            ohlc_max_retries: parse_or("OHLC_MAX_RETRIES", 3)?,

            // ── قيم ابتدائية آمنة (يستبدلها أول جلب) ────────
            initial_high: parse_or("INITIAL_HIGH", 1.0)?,
            initial_low: parse_or("INITIAL_LOW", 0.5)?,

            // ── الغاز ─────────────────────────────────────────
            gas_limit: parse_or("GAS_LIMIT", 300_000)?,

            // ── إعدادات ChainClient V2 ───────────────────────
            contract_abi_path: env_or("CONTRACT_ABI_PATH", "../contract/out/GridTrading.sol/GridTradingV2.json"),
            backup_rpc_urls: parse_comma_list("BACKUP_RPC_URLS"),
            fee_currency_address: env::var("FEE_CURRENCY_ADDRESS").ok(),
            simulate_before_execute: parse_or("SIMULATE_BEFORE_EXECUTE", true)?,
            min_profit_threshold_usd: parse_or("MIN_PROFIT_THRESHOLD_USD", 0.5)?,
            max_rpc_retries: parse_or("MAX_RPC_RETRIES", 3)?,
            use_legacy_tx: parse_or("USE_LEGACY_TX", false)?,
            dry_run_mode: parse_or("DRY_RUN_MODE", false)?,
        })
    }
}

// ─────────────────────────────────────────────────────────────
//  دوال مساعدة
// ─────────────────────────────────────────────────────────────

fn require(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("متغير البيئة '{}' مفقود في ملف .env", key))
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_or<T>(key: &str, default: T) -> Result<T>
where
    T: std::str::FromStr + Copy,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(val) => val.parse::<T>().map_err(|e| {
            anyhow::anyhow!("فشل تحويل '{}' إلى الرقم المطلوب: {}", key, e)
        }),
        Err(_) => Ok(default),
    }
}

/// تحليل قائمة مفصولة بفواصل (للـ BACKUP_RPC_URLS)
fn parse_comma_list(key: &str) -> Vec<String> {
    env::var(key)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
