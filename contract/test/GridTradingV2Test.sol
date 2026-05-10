// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console2} from "forge-std/Test.sol";
import {GridTradingV2}  from "../src/GridTradingV2.sol";

// ── Mocks ────────────────────────────────────────────────────────────────────

contract MockERC20 {
    string  public name;
    uint8   public decimals = 18;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    constructor(string memory _name) { name = _name; }

    function mint(address to, uint256 amount) external { balanceOf[to] += amount; }

    function transfer(address to, uint256 amount) external returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[to]         += amount;
        return true;
    }
    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        if (allowance[from][msg.sender] != type(uint256).max)
            allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to]   += amount;
        return true;
    }
    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }
}

contract MockPriceFeed {
    int256  private _price;
    uint256 private _updatedAt;
    uint80  private _roundId;
    uint8   public  decimals = 8;

    function set(int256 price) external {
        _price     = price;
        _updatedAt = block.timestamp;
        _roundId++;
    }
    // NOTE: call vm.warp(block.timestamp + 31 minutes) BEFORE calling set() to simulate staleness
    function setAt(int256 price, uint256 timestamp) external {
        _price     = price;
        _updatedAt = timestamp;
        _roundId++;
    }
    function latestRoundData() external view returns (
        uint80 roundId, int256 answer, uint256, uint256 updatedAt, uint80 answeredInRound
    ) {
        return (_roundId, _price, 0, _updatedAt, _roundId);
    }
}

contract MockMoola {
    // Simulates 10% yield on withdrawal by minting extra tokens
    function deposit(address, uint256, address, uint16) external {}
    function withdraw(address token, uint256 amount, address to) external returns (uint256) {
        uint256 withYield = amount + (amount / 10); // 10% simulated yield
        MockERC20(token).mint(to, withYield);
        return withYield;
    }
}

contract MockMento {
    // 1:1 swap, ignores minOut so tests can control slippage separately
    function swapIn(address tokenIn, address tokenOut, uint256 amountIn, uint256)
        external returns (uint256 amountOut)
    {
        amountOut = amountIn;
        MockERC20(tokenIn).transferFrom(msg.sender, address(this), amountIn);
        MockERC20(tokenOut).mint(msg.sender, amountOut);
    }
}

// ── Test Suite ────────────────────────────────────────────────────────────────

contract GridTradingV2Test is Test {
    GridTradingV2 public grid;
    MockERC20     public celo;
    MockERC20     public cusd;
    MockMento     public mento;
    MockPriceFeed public feed;
    MockMoola     public moola;

    address keeper       = makeAddr("keeper");
    address user         = makeAddr("user");
    address feeRecipient = makeAddr("feeRecipient");
    address attacker     = makeAddr("attacker");

    // Grid params
    uint256 constant LOWER  = 0.20e18;
    uint256 constant UPPER  = 0.40e18;
    uint256 constant LEVELS = 5;
    uint256 constant AMOUNT = 100e18;

    function setUp() public {
        celo  = new MockERC20("CELO");
        cusd  = new MockERC20("cUSD");
        mento = new MockMento();
        feed  = new MockPriceFeed();
        moola = new MockMoola();

        feed.set(0.30e8); // $0.30 — inside the grid range

        grid = new GridTradingV2(
            keeper,
            address(mento),
            address(feed),
            address(moola),
            feeRecipient
        );

        cusd.mint(user, 1_000e18);
        celo.mint(address(mento), 10_000e18);
        cusd.mint(address(mento), 10_000e18);
    }

    // ── Helper ──────────────────────────────────────────────────────────────

    function _createGrid(bool withYield) internal returns (uint256 gridId) {
        vm.startPrank(user);
        cusd.approve(address(grid), AMOUNT);
        gridId = grid.createGrid(
            address(celo), address(cusd),
            LOWER, UPPER, LEVELS, AMOUNT,
            withYield, 100
        );
        vm.stopPrank();
    }

    // ── CREATE GRID ─────────────────────────────────────────────────────────

    function test_CreateGrid_SetsStateCorrectly() public {
        uint256 id = _createGrid(false);
        (address owner,,,,,, uint256 apg, uint256 qBal,, bool active,,,) = grid.grids(id);
        assertEq(owner,  user);
        assertEq(qBal,   AMOUNT);
        assertEq(apg,    AMOUNT / LEVELS);
        assertTrue(active);
    }

    function test_CreateGrid_LevelsCount() public {
        uint256 id = _createGrid(false);
        GridTradingV2.GridLevel[] memory levels = grid.getGridLevels(id);
        assertEq(levels.length, LEVELS);
        assertEq(levels[0].price, LOWER);
        assertEq(levels[LEVELS - 1].price, UPPER);
    }

    function test_CreateGrid_InvalidRange_Reverts() public {
        vm.startPrank(user);
        cusd.approve(address(grid), AMOUNT);
        vm.expectRevert("GridV2: invalid price range");
        grid.createGrid(address(celo), address(cusd), UPPER, LOWER, LEVELS, AMOUNT, false, 100);
        vm.stopPrank();
    }

    function test_CreateGrid_RangeTooWide_Reverts() public {
        vm.startPrank(user);
        cusd.approve(address(grid), AMOUNT);
        vm.expectRevert("GridV2: range too wide (>100x)");
        grid.createGrid(address(celo), address(cusd), 1, UPPER, LEVELS, AMOUNT, false, 100);
        vm.stopPrank();
    }

    function test_CreateGrid_SameTokens_Reverts() public {
        vm.startPrank(user);
        cusd.approve(address(grid), AMOUNT);
        vm.expectRevert("GridV2: same tokens");
        grid.createGrid(address(cusd), address(cusd), LOWER, UPPER, LEVELS, AMOUNT, false, 100);
        vm.stopPrank();
    }

    // ── EXECUTE GRID — BUY ──────────────────────────────────────────────────

    function test_ExecuteBuy_FillsLevel() public {
        uint256 id = _createGrid(false);
        // price = $0.30 which is >= LOWER ($0.20) so level[0] at $0.20 should be a buy
        // Set oracle price BELOW level[0].price to trigger buy
        feed.set(0.19e8); // $0.19 < $0.20 (level 0 price)
        vm.prank(keeper);
        grid.executeGrid(id, 0);
        GridTradingV2.GridLevel[] memory levels = grid.getGridLevels(id);
        assertTrue(levels[0].filled);
        assertFalse(levels[0].isBuy); // should flip to sell after fill
    }

    function test_ExecuteBuy_PriceConditionNotMet_Reverts() public {
        uint256 id = _createGrid(false);
        // oracle $0.30 > level[0].price $0.20 — buy condition NOT met
        feed.set(0.30e8);
        vm.prank(keeper);
        vm.expectRevert("GridV2: price condition not met");
        grid.executeGrid(id, 0);
    }

    function test_Execute_AccumulatesFees() public {
        uint256 id = _createGrid(false);
        feed.set(0.19e8);
        vm.prank(keeper);
        grid.executeGrid(id, 0);
        uint256 celoFees = grid.accumulatedFees(address(celo));
        assertGt(celoFees, 0, "No fees accumulated");
    }

    // ── ACCESS CONTROL ──────────────────────────────────────────────────────

    function test_NonKeeper_ExecuteReverts() public {
        uint256 id = _createGrid(false);
        vm.prank(attacker);
        vm.expectRevert("GridV2: not keeper");
        grid.executeGrid(id, 0); 
    }

    function test_PerformUpkeep_NonKeeper_Reverts() public {
        uint256 id = _createGrid(false);
        bytes memory data = abi.encode(id, uint256(0));
        vm.prank(attacker);
        vm.expectRevert("GridV2: performUpkeep caller not keeper");
        grid.performUpkeep(data);
    }

    function test_PerformUpkeep_AsKeeper_Works() public {
        uint256 id = _createGrid(false);
        feed.set(0.19e8); // trigger buy on level 0
        bytes memory data = abi.encode(id, uint256(0));
        vm.prank(keeper);
        grid.performUpkeep(data);
        assertTrue(grid.getGridLevels(id)[0].filled);
    }

    // ── ORACLE STALENESS ────────────────────────────────────────────────────

    function test_StaleOracle_Reverts() public {
        // Must warp AFTER createGrid so the grid's _getOraclePrice call has a fresh timestamp
        uint256 id = _createGrid(false);
        // Now warp forward so current time is well past the oracle's updatedAt
        vm.warp(block.timestamp + 31 minutes + 1);
        // Update the oracle's price but leave its updatedAt in the PAST (before the warp)
        // We do this by calling set() which records block.timestamp AS OF BEFORE THE WARP
        // Actually: set a price whose updatedAt is 31+ minutes ago relative to new block.timestamp
        // We need to directly write the timestamp: use setAt with the pre-warp time
        uint256 staleTime = block.timestamp - 31 minutes - 1;
        feed.setAt(0.19e8, staleTime);
        vm.prank(keeper);
        vm.expectRevert("Oracle: price too old");
        grid.executeGrid(id, 0);
    }

    // ── MOOLA YIELD ─────────────────────────────────────────────────────────

    function test_CloseGrid_WithYield_ReturnsExtra() public {
        uint256 id = _createGrid(true); // yieldEnabled=true
        // Record user cusd balance before close
        uint256 before = cusd.balanceOf(user);
        vm.prank(user);
        grid.closeGrid(id);
        uint256 returned = cusd.balanceOf(user) - before;
        // MockMoola adds 10% yield; user deposited 100e18 so should get ~110e18
        assertGe(returned, AMOUNT, "Should return at least deposited amount");
    }

    // ── CLOSE GRID ──────────────────────────────────────────────────────────

    function test_CloseGrid_DeactivatesGrid() public {
        uint256 id = _createGrid(false);
        vm.prank(user);
        grid.closeGrid(id);
        (,,,,,,,,, bool active,,,) = grid.grids(id);
        assertFalse(active);
    }

    function test_CloseGrid_NonOwner_Reverts() public {
        uint256 id = _createGrid(false);
        vm.prank(attacker);
        vm.expectRevert("GridV2: not owner");
        grid.closeGrid(id);
    }

    function test_CloseGrid_ReturnsQuoteBalance() public {
        uint256 id = _createGrid(false);
        uint256 before = cusd.balanceOf(user);
        vm.prank(user);
        grid.closeGrid(id);
        assertEq(cusd.balanceOf(user) - before, AMOUNT);
    }

    // ── FEE COLLECTION ──────────────────────────────────────────────────────

    function test_CollectFees_Works() public {
        uint256 id = _createGrid(false);
        feed.set(0.19e8);
        vm.prank(keeper);
        grid.executeGrid(id, 0);

        uint256 fees = grid.accumulatedFees(address(celo));
        assertGt(fees, 0);

        vm.prank(grid.owner());
        grid.collectFees(address(celo));

        assertEq(celo.balanceOf(feeRecipient), fees);
        assertEq(grid.accumulatedFees(address(celo)), 0);
    }

    function test_CollectFees_NoFees_Reverts() public {
        vm.prank(grid.owner());
        vm.expectRevert("GridV2: no fees for token");
        grid.collectFees(address(celo));
    }

    // ── CHECK UPKEEP ────────────────────────────────────────────────────────

    function test_CheckUpkeep_ReturnsTrueWhenExecutable() public {
        uint256 id = _createGrid(false);
        feed.set(0.19e8); // triggers buy
        bytes memory checkData = abi.encode(uint256(0), uint256(10));
        (bool needed, bytes memory perfData) = grid.checkUpkeep(checkData);
        assertTrue(needed);
        (uint256 gId, uint256 lIdx) = abi.decode(perfData, (uint256, uint256));
        assertEq(gId,  id);
        assertEq(lIdx, 0);
    }

    function test_CheckUpkeep_ReturnsFalseWhenNotExecutable() public {
        _createGrid(false);
        // Price $0.50 is ABOVE upper bound ($0.40) — all levels are buys (isBuy=true)
        // A buy only executes when currentPrice <= level.price.
        // $0.50 > any level price in [$0.20..$0.40], so nothing is executable.
        feed.set(0.50e8);
        bytes memory checkData = abi.encode(uint256(0), uint256(10));
        (bool needed,) = grid.checkUpkeep(checkData);
        assertFalse(needed);
    }

    // ── FUZZ ────────────────────────────────────────────────────────────────

    /// @dev Fuzz: any valid price range should create successfully
    function testFuzz_CreateGrid_ValidRange(
        uint256 lower,
        uint256 upper,
        uint256 levels
    ) public {
        lower  = bound(lower,  1e15, 0.99e18);
        upper  = bound(upper,  lower + 1, lower * 99); // < 100x, > lower
        levels = bound(levels, 2, 50);

        uint256 amount = levels * 2; // >gridCount per requirement
        cusd.mint(user, amount);
        feed.set(int256(lower / 1e10)); // set oracle in range (8 decimals)

        vm.startPrank(user);
        cusd.approve(address(grid), amount);
        uint256 id = grid.createGrid(
            address(celo), address(cusd),
            lower, upper, levels, amount, false, 100
        );
        vm.stopPrank();

        (,,,,, uint256 gc,,,,bool active,,,) = grid.grids(id);
        assertEq(gc, levels);
        assertTrue(active);
    }
}
