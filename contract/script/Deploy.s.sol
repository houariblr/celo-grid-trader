// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "../src/GridTradingV2.sol"; // تأكد من مسار ملف العقد

contract DeployScript is Script {
    function run() external {
        // قراءة المتغيرات من ملف .env
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        
        address keeper = vm.envAddress("KEEPER_ADDRESS");
        address mento = vm.envAddress("MENTO_EXCHANGE_ADDRESS");
        address priceFeed = vm.envAddress("PRICE_FEED_ADDRESS");
        address moola = vm.envAddress("MOOLA_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT");

        // بدء المعاملة
        vm.startBroadcast(deployerPrivateKey);

        // نشر العقد مع تمرير العناوين الخمسة
        GridTradingV2 gridContract = new GridTradingV2(
            keeper,
            mento,
            priceFeed,
            moola,
            feeRecipient
        );

        // إنهاء المعاملة
        vm.stopBroadcast();

        // طباعة عنوان العقد الجديد في التيرمينال
        console.log("GridTradingV2 Deployed At:", address(gridContract));
    }
}
