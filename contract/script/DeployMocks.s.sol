// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {MockChainlinkFeed, MockMento, MockERC20} from "../src/mocks/MockContracts.sol";
import {GridTradingV2} from "../src/GridTrading.sol";

contract DeployMocks is Script {
    function run() external {
        uint256 deployerKey  = vm.envUint("PRIVATE_KEY");
        address keeper       = vm.envAddress("KEEPER_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT");

        vm.startBroadcast(deployerKey);

        // 1. Deploy mocks
        MockChainlinkFeed feed  = new MockChainlinkFeed();
        MockMento         mento = new MockMento();
        MockERC20         cUSD  = new MockERC20("Celo Dollar", "cUSD");
        MockERC20         celo  = new MockERC20("Celo", "CELO");

        // 2. Mint test tokens للـ deployer
        cUSD.mint(keeper, 10_000e18); // 10,000 cUSD
        celo.mint(keeper, 10_000e18); // 10,000 CELO

        // 3. Deploy GridTradingV2 مع Mock addresses
        GridTradingV2 grid = new GridTradingV2(
            keeper,
            address(mento),
            address(feed),
            address(0),      // no Moola on Sepolia
            feeRecipient
        );

        vm.stopBroadcast();

        console.log("=== Celo Sepolia Deployment ===");
        console.log("MockChainlinkFeed:", address(feed));
        console.log("MockMento:        ", address(mento));
        console.log("MockcUSD:         ", address(cUSD));
        console.log("MockCELO:         ", address(celo));
        console.log("GridTradingV2:    ", address(grid));
    }
}
