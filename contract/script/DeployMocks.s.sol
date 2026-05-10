// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "../test/GridTradingV2Test.sol"; // Reusing the mocks we just wrote

contract DeployMocks is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(deployerPrivateKey);

        MockERC20 celo = new MockERC20("CELO");
        MockERC20 cusd = new MockERC20("cUSD");
        MockMento mento = new MockMento();
        MockPriceFeed feed = new MockPriceFeed();
        MockMoola moola = new MockMoola();

        // Set initial price to $0.30
        feed.set(0.30e8);

        console2.log("Mock CELO deployed to:", address(celo));
        console2.log("Mock cUSD deployed to:", address(cusd));
        console2.log("Mock Mento deployed to:", address(mento));
        console2.log("Mock PriceFeed deployed to:", address(feed));
        console2.log("Mock Moola deployed to:", address(moola));

        vm.stopBroadcast();
    }
}
