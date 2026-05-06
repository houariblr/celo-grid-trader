// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import {GridTradingV2} from "../src/GridTrading.sol";

contract DeployScript is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");

        address keeper        = vm.envAddress("KEEPER_ADDRESS");
        address mentoExchange = vm.envAddress("MENTO_EXCHANGE_ADDRESS");
        address priceFeedAddr = vm.envAddress("PRICE_FEED_ADDRESS");
        address moolaPoolAddr = vm.envOr("MOOLA_ADDRESS", address(0));
        address feeRecipient  = vm.envAddress("FEE_RECIPIENT");

        // FEE_RECIPIENT cannot be zero — use KEEPER_ADDRESS as fallback
        if (feeRecipient == address(0)) {
            feeRecipient = keeper;
        }

        vm.startBroadcast(deployerPrivateKey);

        GridTradingV2 grid = new GridTradingV2(
            keeper,
            mentoExchange,
            priceFeedAddr,
            moolaPoolAddr,
            feeRecipient
        );

        console.log("Deployed at:", address(grid));

        vm.stopBroadcast();
    }
}
