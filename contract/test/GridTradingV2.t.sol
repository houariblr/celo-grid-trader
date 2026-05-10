// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title  GridTradingV2Test
 * @notice Foundry test suite validating every critical fix listed in the
 *         pre-deadline checklist.
 *
 * Test groups
 * ───────────
 *  A. ACL / performUpkeep bypass
 *  B. Oracle decimal normalisation (8-dec Chainlink feed)
 *  C. Slippage protection
 *  D. Per-grid balance isolation (no cross-grid leakage)
 *  E. Moola yield capture on closeGrid
 *  F. Fee collection
 *  G. Input validation edge cases
 *
 * Run with:
 *   forge test --match-path test/GridTradingV2.t.sol -vvv
 *
 * Dependencies expected in your foundry.toml:
 *   [dependencies]
 *   forge-std = { version = "1.9.1" }
 */

import "forge-std/Test.sol";
import "../src/GridTrading.sol";

// ─── Minimal interface stubs ──────────────────────────────────────────────────

interface IERC20Mint {
    function mint(address to, uint256 amount) external;
    function approve(address spender, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

// ─── Mock contracts ───────────────────────────────────────────────────────────

/// @dev ERC-20 with public mint — stands in for cUSD and CELO in tests
contract MockERC20 is IERC20 {
    string  public name;
    string  public symbol;
    uint8   public decimals = 18;
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    constructor(string memory _name, string memory _symbol) {
        name   = _name;
        symbol = _symbol;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply    += amount;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        require(balanceOf[msg.sender] >= amount, "ERC20: insufficient");
        balanceOf[msg.sender] -= amount;
        balanceOf[to]         += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        require(balanceOf[from]            >= amount, "ERC20: insufficient");
        require(allowance[from][msg.sender] >= amount, "ERC20: allowance");
        balanceOf[from]              -= amount;
        balanceOf[to]                += amount;
        allowance[from][msg.sender]  -= amount;
        return true;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }
}

/// @dev Chainlink feed mock — configurable decimals, price, and staleness
contract MockChainlinkFeed is IChainlinkFeed {
    int256  public latestAnswer;
    uint8   public decimals;
    uint256 public updatedAt;
    uint80  public roundId = 1;

    constructor(int256 _answer, uint8 _decimals) {
        latestAnswer = _answer;
        decimals     = _decimals;
        updatedAt    = block.timestamp;
    }

    function setPrice(int256 _answer) external { latestAnswer = _answer; }
    function setUpdatedAt(uint256 ts)  external { updatedAt    = ts; }

    function latestRoundData()
        external view
        returns (uint80, int256, uint256, uint256, uint80)
    {
        return (roundId, latestAnswer, 0, updatedAt, roundId);
    }
}

/// @dev Mento mock — swapIn returns a fixed amount so tests stay deterministic
contract MockMento is IMento {
    /// rate: quoteOut = amountIn * swapRate / 1e18   (for buy)
    uint256 public swapRate; // e.g. 2e18 means 1 cUSD → 2 CELO at price 0.5

    MockERC20 public baseToken;
    MockERC20 public quoteToken;

    constructor(MockERC20 _base, MockERC20 _quote, uint256 _rate) {
        baseToken  = _base;
        quoteToken = _quote;
        swapRate   = _rate;
    }

    function swapIn(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minOut
    ) external returns (uint256 amountOut) {
        amountOut = amountIn * swapRate / 1e18;
        require(amountOut >= minOut, "MockMento: slippage");

        // Pull tokenIn from caller
        MockERC20(tokenIn).transferFrom(msg.sender, address(this), amountIn);
        // Give tokenOut (mint fresh so we don't need pre-funding)
        MockERC20(tokenOut).mint(msg.sender, amountOut);
    }
}

/// @dev Moola mock — tracks deposits; accrues 10% synthetic yield on withdraw
contract MockMoolaPool is IMoolaLendingPool {
    mapping(address => mapping(address => uint256)) public deposited; // token → depositor → amount
    uint256 public yieldBps = 1000; // 10%

    function deposit(address asset, uint256 amount, address onBehalfOf, uint16) external {
        MockERC20(asset).transferFrom(msg.sender, address(this), amount);
        deposited[asset][onBehalfOf] += amount;
    }

    function withdraw(address asset, uint256 amount, address to) external returns (uint256) {
        uint256 withYield = amount + (amount * yieldBps / 10_000);
        deposited[asset][msg.sender] -= amount; // reduce by principal
        MockERC20(asset).mint(to, withYield);   // return principal + yield
        return withYield;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Test contract
// ─────────────────────────────────────────────────────────────────────────────

contract GridTradingV2Test is Test {

    // ── Actors ──────────────────────────────────────────────────────────────
    address internal owner      = makeAddr("owner");
    address internal keeper     = makeAddr("keeper");
    address internal user       = makeAddr("user");
    address internal attacker   = makeAddr("attacker");
    address internal feeWallet  = makeAddr("feeWallet");

    // ── Contracts ───────────────────────────────────────────────────────────
    GridTradingV2     internal grid;
    MockERC20         internal celo;
    MockERC20         internal cusd;
    MockChainlinkFeed internal feed;
    MockMento         internal mento;
    MockMoolaPool     internal moola;

    // ── Common grid params ──────────────────────────────────────────────────
    /// CELO/USD = $0.50  (8-dec Chainlink: 50_000_000)
    int256  constant PRICE_8DEC    = 50_000_000;          // $0.50 with 8 decimals
    uint256 constant PRICE_18DEC   = 0.50e18;             // $0.50 with 18 decimals

    uint256 constant LOWER  = 0.30e18;
    uint256 constant UPPER  = 0.70e18;
    uint256 constant N      = 5;                           // grid levels
    uint256 constant AMOUNT = 100e18;                      // 100 cUSD deposit

    // swapRate for MockMento: 1 cUSD → 2 CELO (price $0.50)
    uint256 constant SWAP_RATE = 2e18;

    // ── Setup ────────────────────────────────────────────────────────────────

    function setUp() public {
        vm.startPrank(owner);

        // Deploy mock tokens
        celo = new MockERC20("CELO", "CELO");
        cusd = new MockERC20("cUSD", "cUSD");

        // Deploy mock oracle: $0.50 price, 8 decimals (standard Chainlink USD feed)
        feed = new MockChainlinkFeed(PRICE_8DEC, 8);

        // Deploy mock Mento: 1 cUSD in → 2 CELO out
        mento = new MockMento(celo, cusd, SWAP_RATE);

        // Deploy mock Moola
        moola = new MockMoolaPool();

        // Deploy main contract
        grid = new GridTradingV2(
            keeper,
            address(mento),
            address(feed),
            address(moola),
            feeWallet
        );

        vm.stopPrank();

        // Fund user with cUSD
        cusd.mint(user, 1_000e18);

        // User approves contract
        vm.prank(user);
        cusd.approve(address(grid), type(uint256).max);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  A. ACL / performUpkeep bypass
    // ════════════════════════════════════════════════════════════════════════

    /// @notice Attacker who is NOT a keeper must NOT be able to call executeGrid
    function test_acl_nonKeeperCannotExecuteGrid() public {
        uint256 gId = _createBasicGrid(false);

        vm.prank(attacker);
        vm.expectRevert("GridV2: not keeper");
        grid.executeGrid(gId, 0);
    }

    /// @notice Registered keeper CAN call executeGrid
    function test_acl_registeredKeeperCanExecute() public {
        uint256 gId = _createBasicGrid(false);

        // Push price to level[0] so execution condition is met
        feed.setPrice(int256(LOWER / 1e10)); // convert 18-dec → 8-dec

        vm.prank(keeper);
        grid.executeGrid(gId, 0); // should not revert
    }

    /// @notice performUpkeep is only callable by a registered keeper.
    ///         An attacker calling it directly must revert.
    function test_acl_performUpkeepBlocksAttacker() public {
        uint256 gId = _createBasicGrid(false);

        // Move price so level[0] would be executable
        feed.setPrice(int256(LOWER / 1e10));

        bytes memory data = abi.encode(gId, uint256(0));

        vm.prank(attacker);
        vm.expectRevert("GridV2: not keeper");
        grid.performUpkeep(data);
    }

    /// @notice Owner can add a new keeper dynamically
    function test_acl_ownerCanAddKeeper() public {
        address newKeeper = makeAddr("newKeeper");

        vm.prank(owner);
        grid.setKeeper(newKeeper, true);

        assertTrue(grid.isKeeper(newKeeper));
    }

    /// @notice Owner can revoke a keeper
    function test_acl_ownerCanRevokeKeeper() public {
        vm.prank(owner);
        grid.setKeeper(keeper, false);

        assertFalse(grid.isKeeper(keeper));

        uint256 gId = _createBasicGrid(false);
        feed.setPrice(int256(LOWER / 1e10));

        vm.prank(keeper);
        vm.expectRevert("GridV2: not keeper");
        grid.executeGrid(gId, 0);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  B. Oracle decimal normalisation
    // ════════════════════════════════════════════════════════════════════════

    /// @notice getOraclePrice must return an 18-decimal value regardless of
    ///         the feed's native decimal count.
    function test_oracle_8DecimalFeedNormalisedTo18() public view {
        uint256 price = grid.getOraclePrice();
        // $0.50 stored as 50_000_000 (8 dec) → should come back as 0.50e18
        assertApproxEqAbs(price, PRICE_18DEC, 1e6, "Price not normalised correctly");
    }

    /// @notice Stale oracle data must cause every execution to revert.
    function test_oracle_staleDataReverts() public {
        uint256 gId = _createBasicGrid(false);

        // Wind timestamp forward past ORACLE_STALENESS_THRESHOLD (30 min)
        vm.warp(block.timestamp + 31 minutes);

        vm.prank(keeper);
        vm.expectRevert("Oracle: price too old");
        grid.executeGrid(gId, 0);
    }

    /// @notice Non-positive oracle answer must revert.
    function test_oracle_zeroPriceReverts() public {
        uint256 gId = _createBasicGrid(false);

        feed.setPrice(0);

        vm.prank(keeper);
        vm.expectRevert("Oracle: non-positive price");
        grid.executeGrid(gId, 0);
    }

    /// @notice createGrid must revert if oracle is already stale at creation time.
    function test_oracle_staleAtCreationReverts() public {
        vm.warp(block.timestamp + 31 minutes);

        vm.prank(user);
        vm.expectRevert("Oracle: price too old");
        grid.createGrid(
            address(celo), address(cusd),
            LOWER, UPPER, N, AMOUNT,
            false, 100
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    //  C. Slippage protection
    // ════════════════════════════════════════════════════════════════════════

    /// @notice If Mento returns less than minOut, the transaction must revert.
    ///         We simulate this by halving the swap rate so amountOut < minOut.
    function test_slippage_buyRevertsWhenSlippageExceeded() public {
        // Deploy a Mento that gives only half the expected CELO
        MockMento badMento = new MockMento(celo, cusd, SWAP_RATE / 4); // 0.5x output

        vm.prank(owner);
        GridTradingV2 grid2 = new GridTradingV2(
            keeper,
            address(badMento),
            address(feed),
            address(moola),
            feeWallet
        );

        cusd.mint(user, 1_000e18);
        vm.prank(user);
        cusd.approve(address(grid2), type(uint256).max);

        vm.prank(user);
        uint256 gId = grid2.createGrid(
            address(celo), address(cusd),
            LOWER, UPPER, N, AMOUNT,
            false, 100   // 1% slippage tolerance
        );

        // Set oracle price at/below level[0].price so buy condition is met
        feed.setPrice(int256(LOWER / 1e10));

        vm.prank(keeper);
        vm.expectRevert("MockMento: slippage"); // propagated from Mento mock
        grid2.executeGrid(gId, 0);
    }

    /// @notice With correct swap rate, a buy executes without revert.
    function test_slippage_buySucceedsWithinTolerance() public {
        uint256 gId = _createBasicGrid(false);
        feed.setPrice(int256(LOWER / 1e10));

        vm.prank(keeper);
        grid.executeGrid(gId, 0); // must not revert
    }

    // ════════════════════════════════════════════════════════════════════════
    //  D. Per-grid balance isolation
    // ════════════════════════════════════════════════════════════════════════

    /// @notice Two grids must not share balances. Closing grid A must not
    ///         drain funds from grid B.
    function test_balance_twoGridsAreIsolated() public {
        // User creates two separate grids with 100 cUSD each
        vm.startPrank(user);
        uint256 gA = grid.createGrid(
            address(celo), address(cusd), LOWER, UPPER, N, AMOUNT, false, 100
        );
        uint256 gB = grid.createGrid(
            address(celo), address(cusd), LOWER, UPPER, N, AMOUNT, false, 100
        );
        vm.stopPrank();

        // Execute one level on grid A
        feed.setPrice(int256(LOWER / 1e10));
        vm.prank(keeper);
        grid.executeGrid(gA, 0);

        // Grid B's quoteBalance must still equal original deposit
        (,,,,,,, uint256 qBalB,,,,,) = grid.grids(gB);
        assertEq(qBalB, AMOUNT, "Grid B balance should be untouched");
    }

    /// @notice closeGrid for grid A must return exactly grid A's funds
    function test_balance_closeGridReturnsOnlyOwnFunds() public {
        address alice = makeAddr("alice");
        address bob   = makeAddr("bob");

        cusd.mint(alice, 200e18);
        cusd.mint(bob,   200e18);

        vm.prank(alice);
        cusd.approve(address(grid), type(uint256).max);
        vm.prank(bob);
        cusd.approve(address(grid), type(uint256).max);

        vm.prank(alice);
        uint256 gA = grid.createGrid(
            address(celo), address(cusd), LOWER, UPPER, N, AMOUNT, false, 100
        );
        vm.prank(bob);
        uint256 gB = grid.createGrid(
            address(celo), address(cusd), LOWER, UPPER, N, AMOUNT, false, 100
        );

        uint256 aliceBefore = cusd.balanceOf(alice);

        vm.prank(alice);
        grid.closeGrid(gA);

        uint256 aliceAfter  = cusd.balanceOf(alice);
        uint256 returned    = aliceAfter - aliceBefore;

        // Alice gets back exactly her 100 cUSD
        assertEq(returned, AMOUNT, "Alice received wrong amount");

        // Bob's grid is untouched
        (,,,,,,, uint256 qBalB,,,,,) = grid.grids(gB);
        assertEq(qBalB, AMOUNT);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  E. Moola yield capture
    // ════════════════════════════════════════════════════════════════════════

    /// @notice When yield is enabled, closing the grid must return MORE than
    ///         the original deposit (principal + accrued interest).
    function test_moola_yieldCapturedOnClose() public {
        vm.prank(user);
        uint256 gId = grid.createGrid(
            address(celo), address(cusd),
            LOWER, UPPER, N, AMOUNT,
            true,  // yieldEnabled
            100
        );

        uint256 before = cusd.balanceOf(user);

        vm.prank(user);
        grid.closeGrid(gId);

        uint256 returned = cusd.balanceOf(user) - before;

        // MockMoola gives 10% yield, so we expect > AMOUNT
        assertGt(returned, AMOUNT, "Yield was not captured");
    }

    /// @notice Without yield, user gets back exactly what they deposited.
    function test_moola_noYieldReturnsPrincipalOnly() public {
        uint256 gId = _createBasicGrid(false);

        uint256 before = cusd.balanceOf(user);
        vm.prank(user);
        grid.closeGrid(gId);

        uint256 returned = cusd.balanceOf(user) - before;
        assertEq(returned, AMOUNT, "Expected exact principal back");
    }

    // ════════════════════════════════════════════════════════════════════════
    //  F. Fee collection
    // ════════════════════════════════════════════════════════════════════════

    /// @notice After 5 buy trades, the owner must be able to withdraw
    ///         accumulated protocol fees to feeRecipient.
    function test_fees_accumulateAndWithdraw() public {
        uint256 gId = _createBasicGrid(false);

        // Set price at lower bound so all 5 buy levels trigger
        feed.setPrice(int256(LOWER / 1e10));

        uint256 levelCount = N; // grid has N levels
        vm.startPrank(keeper);
        for (uint256 i = 0; i < levelCount; i++) {
            // Ignore if price condition not met for some levels
            try grid.executeGrid(gId, i) {} catch {}
        }
        vm.stopPrank();

        uint256 accumulated = grid.accumulatedFees(address(quoteToken));
        assertGt(accumulated, 0, "No fees accumulated");

        uint256 walletBefore = cusd.balanceOf(feeWallet);

        vm.prank(owner);
        grid.collectFees(address(cusd));

        uint256 walletAfter = cusd.balanceOf(feeWallet);
        assertGt(walletAfter, walletBefore, "Fee wallet balance did not increase");
        assertEq(grid.accumulatedFees(address(quoteToken)), 0, "accumulatedFees not cleared");
    }

    /// @notice Calling collectFees when there are none must revert cleanly.
    function test_fees_revertOnEmptyCollect() public {
        vm.prank(owner);
        vm.expectRevert("GridV2: no fees");
        grid.collectFees(address(cusd));
    }

    // ════════════════════════════════════════════════════════════════════════
    //  G. Input validation edge cases
    // ════════════════════════════════════════════════════════════════════════

    function test_validation_samePairReverts() public {
        vm.prank(user);
        vm.expectRevert("GridV2: same tokens");
        grid.createGrid(address(celo), address(celo), LOWER, UPPER, N, AMOUNT, false, 100);
    }

    function test_validation_invertedRangeReverts() public {
        vm.prank(user);
        vm.expectRevert("GridV2: invalid price range");
        grid.createGrid(address(celo), address(cusd), UPPER, LOWER, N, AMOUNT, false, 100);
    }

    function test_validation_gridCountTooLowReverts() public {
        vm.prank(user);
        vm.expectRevert("GridV2: invalid grid count");
        grid.createGrid(address(celo), address(cusd), LOWER, UPPER, 1, AMOUNT, false, 100);
    }

    function test_validation_gridCountTooHighReverts() public {
        vm.prank(user);
        vm.expectRevert("GridV2: invalid grid count");
        grid.createGrid(address(celo), address(cusd), LOWER, UPPER, 51, AMOUNT, false, 100);
    }

    function test_validation_slippageBelowMinReverts() public {
        vm.prank(user);
        vm.expectRevert("GridV2: slippage out of range");
        grid.createGrid(address(celo), address(cusd), LOWER, UPPER, N, AMOUNT, false, 5);
    }

    function test_validation_slippageAboveMaxReverts() public {
        vm.prank(user);
        vm.expectRevert("GridV2: slippage out of range");
        grid.createGrid(address(celo), address(cusd), LOWER, UPPER, N, AMOUNT, false, 501);
    }

    function test_validation_executingFilledLevelReverts() public {
        uint256 gId = _createBasicGrid(false);
        feed.setPrice(int256(LOWER / 1e10));

        vm.prank(keeper);
        grid.executeGrid(gId, 0); // first execution succeeds

        vm.prank(keeper);
        vm.expectRevert("GridV2: level filled");
        grid.executeGrid(gId, 0); // same level — must revert
    }

    function test_validation_closingInactiveGridReverts() public {
        uint256 gId = _createBasicGrid(false);

        vm.prank(user);
        grid.closeGrid(gId); // first close OK

        vm.prank(user);
        vm.expectRevert("GridV2: already closed");
        grid.closeGrid(gId); // second close must revert
    }

    function test_validation_onlyOwnerCanClose() public {
        uint256 gId = _createBasicGrid(false);

        vm.prank(attacker);
        vm.expectRevert("GridV2: not owner");
        grid.closeGrid(gId);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  H. Chainlink checkUpkeep smoke test
    // ════════════════════════════════════════════════════════════════════════

    function test_checkUpkeep_returnsTrueWhenExecutable() public {
        _createBasicGrid(false);

        // Drive price to lower bound
        feed.setPrice(int256(LOWER / 1e10));

        bytes memory checkData = abi.encode(uint256(0), uint256(10));
        (bool needed, bytes memory performData) = grid.checkUpkeep(checkData);

        assertTrue(needed, "checkUpkeep should return true");
        (uint256 gId, uint256 lIdx) = abi.decode(performData, (uint256, uint256));
        assertEq(gId, 0);
        assertEq(lIdx, 0);
    }

    function test_checkUpkeep_returnsFalseWhenStaleOracle() public {
        _createBasicGrid(false);
        feed.setPrice(int256(LOWER / 1e10));

        vm.warp(block.timestamp + 31 minutes);

        bytes memory checkData = abi.encode(uint256(0), uint256(10));
        (bool needed,) = grid.checkUpkeep(checkData);

        assertFalse(needed, "checkUpkeep should return false on stale oracle");
    }

    // ════════════════════════════════════════════════════════════════════════
    //  Helpers
    // ════════════════════════════════════════════════════════════════════════

    /// @dev Creates a basic grid for user with standard params
    function _createBasicGrid(bool yieldEnabled) internal returns (uint256 gId) {
        vm.prank(user);
        gId = grid.createGrid(
            address(celo),
            address(cusd),
            LOWER,
            UPPER,
            N,
            AMOUNT,
            yieldEnabled,
            100  // 1% slippage
        );
    }
}
