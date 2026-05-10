// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "../test/GridTradingV2Test.sol"; 
import "../src/GridTradingV2.sol";

contract DeployAll is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        address keeper = vm.envAddress("KEEPER_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT");
        
        vm.startBroadcast(deployerPrivateKey);

        MockERC20 celo = new MockERC20("CELO");
        MockERC20 cusd = new MockERC20("cUSD");
        MockMento mento = new MockMento();
        MockPriceFeed feed = new MockPriceFeed();
        MockMoola moola = new MockMoola();

        feed.set(0.30e8);

        GridTradingV2 grid = new GridTradingV2(
            keeper,
            address(mento),
            address(feed),
            address(moola),
            feeRecipient
        );

        console2.log("--- MOCKS ---");
        console2.log("CELO_MOCK=", address(celo));
        console2.log("CUSD_MOCK=", address(cusd));
        console2.log("MENTO_MOCK=", address(mento));
        console2.log("FEED_MOCK=", address(feed));
        console2.log("MOOLA_MOCK=", address(moola));
        console2.log("--- CONTRACT ---");
        console2.log("GRID_V2=", address(grid));

        vm.stopBroadcast();
    }
}
