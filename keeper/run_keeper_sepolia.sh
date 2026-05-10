#!/bin/bash
# ============================================================
#  run_keeper_sepolia.sh - تشغيل الـ Keeper على Celo Sepolia
#  مع إعدادات آمنة للاختبار
# ============================================================

set -e

echo "🚀 Celo Grid Keeper V2 - Sepolia Testnet"
echo "=========================================="

# التحقق من وجود ملف البيئة
if [ ! -f .env ]; then
    echo "❌ Error: .env file not found!"
    echo "   Run: cp env.example .env"
    exit 1
fi

# تحميل متغيرات البيئة
export $(grep -v '^#' .env | xargs)

# التحقق من RPC URL
if [[ "$RPC_URL" != *"sepolia"* && "$RPC_URL" != *"alfajores"* ]]; then
    echo "⚠️  Warning: RPC_URL doesn't appear to be Sepolia testnet!"
    echo "   Current: $RPC_URL"
    echo "   Should be: https://alfajores-forno.celo-testnet.org"
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# تعطيل DRY_RUN_MODE للتنفيذ الحقيقي
export DRY_RUN_MODE=false

echo ""
echo "📋 Configuration:"
echo "   RPC URL: $RPC_URL"
echo "   Keeper: $KEEPER_ADDRESS"
echo "   Contract: $CONTRACT_ADDRESS"
echo "   Fee Currency: ${FEE_CURRENCY_ADDRESS:-None (using CELO)}"
echo "   Dry Run: $DRY_RUN_MODE"
echo ""
echo "⏳ Starting Keeper in 3 seconds... (Ctrl+C to cancel)"
sleep 3

echo ""
echo "🔄 Running Keeper..."
echo "=========================================="

# تشغيل الـ Keeper
cargo run --bin keeper-v2 --release
