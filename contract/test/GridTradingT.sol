// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {GridTradingV2} from "../src/GridTrading.sol";

// --- الملحقات المطلوبة للاختبار ---

contract MockERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    uint8 public decimals = 18;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        if (allowance[from][msg.sender] != type(uint256).max) {
            allowance[from][msg.sender] -= amount;
        }
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }
}

contract MockPriceFeed {
    int256 private _price;
    uint8 public decimals = 8;

    function setPrice(int256 price) external { _price = price; }

    function latestRoundData() external view returns (uint80, int256, uint256, uint256 updatedAt, uint80) {
        return (0, _price, 0, block.timestamp, 0);
    }
}

contract MockMoola {
    function deposit(address token, uint256 amount, address onBehalfOf, uint16) external {}
    function withdraw(address token, uint256 amount, address to) external returns (uint256) { return amount; }
}

contract MockMento {
    function swapIn(address tokenIn, address tokenOut, uint256 amountIn, uint256) external returns (uint256) {
        MockERC20(tokenIn).transferFrom(msg.sender, address(this), amountIn);
        MockERC20(tokenOut).mint(msg.sender, amountIn);
        return amountIn;
    }
}

// --- ملف الاختبار الرئيسي ---

contract GridTradingTest is Test {
    GridTradingV2 public gridTrading;
    MockERC20 public celo;
    MockERC20 public cusd;
    MockMento public mento;
    MockPriceFeed public priceFeed;
    MockMoola public moola;

    address public keeper = address(0x1);
    address public user = address(0x2);
    address public feeRecipient = address(0x3);

    function setUp() public {
        celo = new MockERC20();
        cusd = new MockERC20();
        mento = new MockMento();
        priceFeed = new MockPriceFeed();
        moola = new MockMoola();

        // تحديث السعر الابتدائي لـ Chainlink (مثلاً 0.9 دولار)
        priceFeed.setPrice(0.9e8); 

        // تمرير الـ 5 بارامترات المطلوبة في V2
        gridTrading = new GridTradingV2(
            keeper, 
            address(mento), 
            address(priceFeed), 
            address(moola), 
            feeRecipient
        );

        cusd.mint(user, 1000e18);
        celo.mint(address(mento), 1000e18); // لتوفير سيولة للمبادلة الوهمية
    }

    function test_CreateGrid() public {
        vm.startPrank(user);
        cusd.approve(address(gridTrading), 100e18);

        uint256 gridId = gridTrading.createGrid(
            address(celo),
            address(cusd),
            1e18,
            2e18,
            5,
            100e18,
            true,  // yieldEnabled
            100    // slippageBps
        );
        vm.stopPrank();

        (address owner,,,,,,, uint256 qBal,, bool active,,,) = gridTrading.grids(gridId);

        assertEq(owner, user);
        assertEq(qBal, 100e18);
        assertTrue(active);
    }

    function test_GridLevels() public {
        vm.startPrank(user);
        cusd.approve(address(gridTrading), 100e18);
        uint256 gridId = gridTrading.createGrid(address(celo), address(cusd), 1e18, 2e18, 5, 100e18, true, 100);
        vm.stopPrank();

        GridTradingV2.GridLevel[] memory levels = gridTrading.getGridLevels(gridId);
        assertEq(levels.length, 5);
        assertEq(levels[0].price, 1e18);
    }

    function test_ExecuteGrid_Buy() public {
        vm.startPrank(user);
        cusd.approve(address(gridTrading), 100e18);
        uint256 gridId = gridTrading.createGrid(address(celo), address(cusd), 1e18, 2e18, 5, 100e18, true, 100);
        vm.stopPrank();

        // تعيين السعر في الاوراكل ليكون مناسباً للتنفيذ
        priceFeed.setPrice(0.9e8); 

        vm.prank(keeper);
        gridTrading.executeGrid(gridId, 0); // حذف البارامتر الثالث الزائد

        GridTradingV2.GridLevel[] memory levels = gridTrading.getGridLevels(gridId);
        assertTrue(levels[0].filled);
    }

    function test_CloseGrid() public {
        vm.startPrank(user);
        cusd.approve(address(gridTrading), 100e18);
        uint256 gridId = gridTrading.createGrid(address(celo), address(cusd), 1e18, 2e18, 5, 100e18, true, 100);

        gridTrading.closeGrid(gridId);
        vm.stopPrank();

        (,,,,,,,,, bool active,,,) = gridTrading.grids(gridId);
        assertFalse(active);
    }

    function test_OnlyKeeper() public {
        vm.startPrank(user);
        cusd.approve(address(gridTrading), 100e18);
        uint256 gridId = gridTrading.createGrid(address(celo), address(cusd), 1e18, 2e18, 5, 100e18, true, 100);
        vm.stopPrank();

        vm.prank(user);
        vm.expectRevert("GridV2: not keeper");
        gridTrading.executeGrid(gridId, 0); 
    }
}
