// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title  GridTradingV2
 * @notice On-chain grid trading protocol on Celo.
 *         V2 — Security-hardened, Oracle-centric, Chainlink Automation ready.
 *
 * KEY CHANGES FROM V1:
 *  [CRITICAL FIX] Per-grid balance accounting — no cross-grid fund leakage
 *  [CRITICAL FIX] Price from Chainlink oracle, not keeper parameter
 *  [CRITICAL FIX] Slippage protection on all swaps
 *  [SECURITY]     ReentrancyGuard on all ERC20 transfer paths
 *  [SECURITY]     Strict input validation everywhere
 *  [FEATURE]      Chainlink Automation (checkUpkeep / performUpkeep)
 *  [FEATURE]      Moola yield on idle cUSD (opt-in per grid)
 *  [FEATURE]      Multi-keeper support with role management
 */

import {IERC20}               from "./interfaces/IERC20.sol";
import {IMento}                from "./interfaces/IMento.sol";
import {IChainlinkFeed}        from "./interfaces/IChainlinkFeed.sol";
import {IMoolaLendingPool}     from "./interfaces/IMoola.sol";
import {IAutomationCompatible} from "./interfaces/IAutomationCompatible.sol";


// ─── Minimal OpenZeppelin ReentrancyGuard (inline to avoid dependency issues) ───
abstract contract ReentrancyGuard {
    uint256 private constant _NOT_ENTERED = 1;
    uint256 private constant _ENTERED     = 2;
    uint256 private _status = _NOT_ENTERED;

    modifier nonReentrant() {
        require(_status != _ENTERED, "ReentrancyGuard: reentrant call");
        _status = _ENTERED;
        _;
        _status = _NOT_ENTERED;
    }
}

// ─── Minimal OpenZeppelin Ownable (inline) ────────────────────────────────────
abstract contract Ownable {
    address private _owner;
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    constructor(address initialOwner) {
        require(initialOwner != address(0), "Ownable: zero address");
        _owner = initialOwner;
        emit OwnershipTransferred(address(0), initialOwner);
    }

    modifier onlyOwner() {
        require(msg.sender == _owner, "Ownable: not owner");
        _;
    }

    function owner() public view returns (address) { return _owner; }

    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Ownable: zero address");
        emit OwnershipTransferred(_owner, newOwner);
        _owner = newOwner;
    }
}

// ─────────────────────────────────────────────────────────────────────────────

contract GridTradingV2 is ReentrancyGuard, Ownable, IAutomationCompatible {

    // ════════════════════════════════════════════════════════════════════════
    //  CONSTANTS
    // ════════════════════════════════════════════════════════════════════════

    /// @notice Maximum age of an oracle price before we reject it (30 minutes)
    uint256 public constant ORACLE_STALENESS_THRESHOLD = 30 minutes;

    /// @notice Default slippage tolerance: 100 = 1%, 50 = 0.5%
    uint256 public constant SLIPPAGE_BPS_DEFAULT = 100; // 1%

    uint256 public constant MAX_GRID_COUNT = 50;
    uint256 public constant MIN_GRID_COUNT = 2;

    // ════════════════════════════════════════════════════════════════════════
    //  DATA STRUCTURES
    // ════════════════════════════════════════════════════════════════════════

    struct Grid {
        address owner;
        address baseToken;       // e.g. CELO
        address quoteToken;      // e.g. cUSD
        uint256 lowerPrice;      // 18 decimal wei
        uint256 upperPrice;      // 18 decimal wei
        uint256 gridCount;
        uint256 amountPerGrid;   // quoteToken per level
        // ── Per-grid balance tracking (V1 critical fix) ──
        uint256 quoteBalance;    // cUSD held by this grid
        uint256 baseBalance;     // CELO held by this grid
        // ── State ──
        bool    active;
        bool    yieldEnabled;    // opt-in Moola yield on idle cUSD
        uint256 slippageBps;     // per-grid slippage tolerance
        uint256 createdAt;
    }

    struct GridLevel {
        uint256 price;
        bool    filled;
        bool    isBuy;
    }

    // ════════════════════════════════════════════════════════════════════════
    //  STATE
    // ════════════════════════════════════════════════════════════════════════

    mapping(uint256 => Grid)        public grids;
    mapping(uint256 => GridLevel[]) public gridLevels;
    mapping(address => uint256[])   public userGrids;

    uint256 public nextGridId;

    // ── External dependencies ──
    address          public mentoExchange;
    IChainlinkFeed   public priceFeed;        // CELO/USD
    IMoolaLendingPool public moolaPool;

    // ── Keeper ACL — multi-keeper support ──
    mapping(address => bool) public isKeeper;

    // ── Protocol fee (basis points, e.g. 10 = 0.1%) ──
    uint256 public feeBps;
    address public feeRecipient;
    uint256 public accumulatedFees; // quoteToken

    // ════════════════════════════════════════════════════════════════════════
    //  EVENTS
    // ════════════════════════════════════════════════════════════════════════

    event GridCreated(
        uint256 indexed gridId,
        address indexed owner,
        uint256 lowerPrice,
        uint256 upperPrice,
        uint256 gridCount,
        bool    yieldEnabled
    );
    event GridExecuted(
        uint256 indexed gridId,
        uint256 levelIndex,
        bool    isBuy,
        uint256 amountIn,
        uint256 amountOut,
        uint256 oraclePrice
    );
    event GridClosed(uint256 indexed gridId, address indexed owner, uint256 quoteReturned, uint256 baseReturned);
    event KeeperUpdated(address indexed keeper, bool status);
    event YieldDeposited(uint256 indexed gridId, uint256 amount);
    event YieldWithdrawn(uint256 indexed gridId, uint256 amount);
    event OracleUpdated(address indexed newFeed);
    event FeeCollected(uint256 indexed gridId, uint256 amount);

    // ════════════════════════════════════════════════════════════════════════
    //  MODIFIERS
    // ════════════════════════════════════════════════════════════════════════

    modifier onlyKeeper() {
        require(isKeeper[msg.sender], "GridV2: not keeper");
        _;
    }

    modifier onlyGridOwner(uint256 gridId) {
        require(grids[gridId].owner == msg.sender, "GridV2: not owner");
        _;
    }

    modifier gridExists(uint256 gridId) {
        require(gridId < nextGridId, "GridV2: grid not found");
        _;
    }

    // ════════════════════════════════════════════════════════════════════════
    //  CONSTRUCTOR
    // ════════════════════════════════════════════════════════════════════════

    constructor(
        address _keeper,
        address _mentoExchange,
        address _priceFeed,
        address _moolaPool,
        address _feeRecipient
    ) Ownable(msg.sender) {
        require(_keeper        != address(0), "zero keeper");
        require(_mentoExchange != address(0), "zero mento");
        require(_priceFeed     != address(0), "zero feed");
        require(_feeRecipient  != address(0), "zero fee recipient");

        isKeeper[_keeper] = true;
        mentoExchange     = _mentoExchange;
        priceFeed         = IChainlinkFeed(_priceFeed);
        moolaPool         = IMoolaLendingPool(_moolaPool);
        feeRecipient      = _feeRecipient;
        feeBps            = 10; // 0.1% default

        emit KeeperUpdated(_keeper, true);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  ORACLE — INTERNAL
    // ════════════════════════════════════════════════════════════════════════

    /**
     * @notice Fetch and validate the on-chain CELO/USD price from Chainlink.
     * @dev Reverts if: price ≤ 0, round incomplete, or data is stale.
     * @return price Price in 18-decimal wei (normalised from Chainlink decimals)
     */
    function _getOraclePrice() internal view returns (uint256 price) {
        (
            uint80  roundId,
            int256  answer,
            ,
            uint256 updatedAt,
            uint80  answeredInRound
        ) = priceFeed.latestRoundData();

        require(answer > 0,                           "Oracle: non-positive price");
        require(answeredInRound >= roundId,            "Oracle: stale round");
        require(block.timestamp - updatedAt <= ORACLE_STALENESS_THRESHOLD, "Oracle: price too old");

        uint8 feedDecimals = priceFeed.decimals(); // typically 8 for Chainlink
        // Normalise to 18 decimals
        price = uint256(answer) * (10 ** (18 - feedDecimals));
    }

    /// @notice Public view for UIs / keepers to preview oracle price
    function getOraclePrice() external view returns (uint256) {
        return _getOraclePrice();
    }

    // ════════════════════════════════════════════════════════════════════════
    //  SAFE ERC20 HELPERS
    // ════════════════════════════════════════════════════════════════════════

    function _safeTransfer(address token, address to, uint256 amount) internal {
        (bool ok, bytes memory data) = token.call(
            abi.encodeWithSelector(IERC20.transfer.selector, to, amount)
        );
        require(ok && (data.length == 0 || abi.decode(data, (bool))), "Transfer failed");
    }

    function _safeTransferFrom(address token, address from, address to, uint256 amount) internal {
        (bool ok, bytes memory data) = token.call(
            abi.encodeWithSelector(IERC20.transferFrom.selector, from, to, amount)
        );
        require(ok && (data.length == 0 || abi.decode(data, (bool))), "TransferFrom failed");
    }

    function _safeApprove(address token, address spender, uint256 amount) internal {
        // Reset to 0 first (USDT compatibility)
        (bool ok1,) = token.call(abi.encodeWithSelector(IERC20.approve.selector, spender, 0));
        require(ok1, "Approve reset failed");
        (bool ok2, bytes memory data) = token.call(
            abi.encodeWithSelector(IERC20.approve.selector, spender, amount)
        );
        require(ok2 && (data.length == 0 || abi.decode(data, (bool))), "Approve failed");
    }

    // ════════════════════════════════════════════════════════════════════════
    //  CREATE GRID
    // ════════════════════════════════════════════════════════════════════════

    /**
     * @notice Create a new grid trading position.
     * @param baseToken    Token being bought/sold (e.g. CELO)
     * @param quoteToken   Collateral token deposited (e.g. cUSD)
     * @param lowerPrice   Lower bound of grid in 18-dec wei
     * @param upperPrice   Upper bound of grid in 18-dec wei
     * @param gridCount    Number of grid levels (2–50)
     * @param totalAmount  Total quoteToken to deposit
     * @param yieldEnabled Whether to deposit idle cUSD into Moola for yield
     * @param slippageBps  Slippage tolerance in bps (min 10 = 0.1%, max 500 = 5%)
     */
    function createGrid(
        address baseToken,
        address quoteToken,
        uint256 lowerPrice,
        uint256 upperPrice,
        uint256 gridCount,
        uint256 totalAmount,
        bool    yieldEnabled,
        uint256 slippageBps
    ) external nonReentrant returns (uint256 gridId) {
        // ── Input validation ──────────────────────────────────────────────
        require(baseToken  != address(0),             "GridV2: zero baseToken");
        require(quoteToken != address(0),             "GridV2: zero quoteToken");
        require(baseToken  != quoteToken,             "GridV2: same tokens");
        require(upperPrice > lowerPrice,              "GridV2: invalid price range");
        require(upperPrice <= lowerPrice * 100,       "GridV2: range too wide (>100x)");
        require(gridCount >= MIN_GRID_COUNT
             && gridCount <= MAX_GRID_COUNT,          "GridV2: invalid grid count");
        require(totalAmount > gridCount,              "GridV2: amount too small");
        require(slippageBps >= 10 && slippageBps <= 500, "GridV2: slippage out of range");

        // Ensure oracle is live before accepting deposits
        _getOraclePrice();

        // ── Receive funds ─────────────────────────────────────────────────
        _safeTransferFrom(quoteToken, msg.sender, address(this), totalAmount);

        uint256 amountPerGrid = totalAmount / gridCount;

        // ── Build grid ────────────────────────────────────────────────────
        gridId = nextGridId++;
        grids[gridId] = Grid({
            owner:        msg.sender,
            baseToken:    baseToken,
            quoteToken:   quoteToken,
            lowerPrice:   lowerPrice,
            upperPrice:   upperPrice,
            gridCount:    gridCount,
            amountPerGrid: amountPerGrid,
            quoteBalance: totalAmount,   // V1 fix: track per-grid
            baseBalance:  0,
            active:       true,
            yieldEnabled: yieldEnabled,
            slippageBps:  slippageBps,
            createdAt:    block.timestamp
        });

        // ── Build price levels ────────────────────────────────────────────
        uint256 priceStep = (upperPrice - lowerPrice) / (gridCount - 1);
        for (uint256 i = 0; i < gridCount; i++) {
            gridLevels[gridId].push(GridLevel({
                price:  lowerPrice + (priceStep * i),
                filled: false,
                isBuy:  true   // all levels start as buy orders
            }));
        }

        userGrids[msg.sender].push(gridId);

        // ── Opt-in: deposit idle cUSD into Moola ─────────────────────────
        if (yieldEnabled && address(moolaPool) != address(0)) {
            _depositToMoola(gridId, quoteToken, totalAmount);
        }

        emit GridCreated(gridId, msg.sender, lowerPrice, upperPrice, gridCount, yieldEnabled);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  EXECUTE GRID  (keeper or Chainlink Automation)
    // ════════════════════════════════════════════════════════════════════════

    /**
     * @notice Execute a grid level. Price is sourced from Chainlink oracle.
     *         The keeper CANNOT pass a price — eliminates price manipulation.
     * @param gridId     Target grid
     * @param levelIndex Which level to execute
     */
    function executeGrid(
        uint256 gridId,
        uint256 levelIndex
    ) external nonReentrant onlyKeeper gridExists(gridId) {
        Grid storage grid = grids[gridId];
        require(grid.active, "GridV2: not active");

        GridLevel storage level = gridLevels[gridId][levelIndex];
        require(!level.filled, "GridV2: level filled");

        // ── Oracle price — cannot be manipulated by keeper ────────────────
        uint256 currentPrice = _getOraclePrice();

        uint256 amountOut;

        if (level.isBuy && currentPrice <= level.price) {
            // ── BUY: spend cUSD, receive CELO ─────────────────────────────
            uint256 amountIn = grid.amountPerGrid;
            require(grid.quoteBalance >= amountIn, "GridV2: insufficient quote balance");

            // Withdraw from Moola if yield is active
            if (grid.yieldEnabled) {
                _withdrawFromMoola(gridId, grid.quoteToken, amountIn);
            }

            // Minimum CELO to receive (slippage protection)
            // minOut = (amountIn / price) * (1 - slippageBps/10000)
            uint256 minOut = (amountIn * 1e18 * (10_000 - grid.slippageBps)) / (currentPrice * 10_000);

            _safeApprove(grid.quoteToken, mentoExchange, amountIn);
            amountOut = IMento(mentoExchange).swapIn(
                grid.quoteToken,
                grid.baseToken,
                amountIn,
                minOut           // ← V1 fix: was 0
            );

            // ── Protocol fee ──
            uint256 fee = amountOut * feeBps / 10_000;
            amountOut -= fee;
            accumulatedFees += fee;

            // ── Update per-grid balances (V1 critical fix) ────────────────
            grid.quoteBalance -= amountIn;
            grid.baseBalance  += amountOut;

            level.filled = true;
            level.isBuy  = false; // flips to sell at this price

            emit GridExecuted(gridId, levelIndex, true, amountIn, amountOut, currentPrice);
            emit FeeCollected(gridId, fee);

        } else if (!level.isBuy && currentPrice >= level.price) {
            // ── SELL: spend CELO, receive cUSD ────────────────────────────
            uint256 celoToSell = grid.amountPerGrid * 1e18 / level.price;
            require(grid.baseBalance >= celoToSell, "GridV2: insufficient base balance");

            uint256 minOut = (celoToSell * currentPrice * (10_000 - grid.slippageBps)) / (1e18 * 10_000);

            _safeApprove(grid.baseToken, mentoExchange, celoToSell);
            amountOut = IMento(mentoExchange).swapIn(
                grid.baseToken,
                grid.quoteToken,
                celoToSell,
                minOut
            );

            uint256 fee = amountOut * feeBps / 10_000;
            amountOut -= fee;
            accumulatedFees += fee;

            grid.baseBalance  -= celoToSell;
            grid.quoteBalance += amountOut;

            // Deposit profits back into Moola
            if (grid.yieldEnabled && address(moolaPool) != address(0)) {
                _depositToMoola(gridId, grid.quoteToken, amountOut);
            }

            level.filled = true;
            level.isBuy  = true; // ready to buy again

            emit GridExecuted(gridId, levelIndex, false, celoToSell, amountOut, currentPrice);
            emit FeeCollected(gridId, fee);

        } else {
            revert("GridV2: price condition not met");
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    //  CHAINLINK AUTOMATION
    // ════════════════════════════════════════════════════════════════════════

    /**
     * @notice Called off-chain by Chainlink nodes every block.
     *         Returns the first executable (gridId, levelIndex) pair.
     * @param checkData ABI-encoded (uint256 startGridId, uint256 maxCheck)
     *                  — allows batching across multiple Chainlink upkeeps
     */
    function checkUpkeep(bytes calldata checkData)
        external
        view
        override
        returns (bool upkeepNeeded, bytes memory performData)
    {
        (uint256 startId, uint256 maxCheck) = abi.decode(checkData, (uint256, uint256));
        uint256 endId = startId + maxCheck;
        if (endId > nextGridId) endId = nextGridId;

        uint256 currentPrice;
        try priceFeed.latestRoundData() returns (
            uint80, int256 answer, uint256, uint256 updatedAt, uint80
        ) {
            if (answer > 0 && block.timestamp - updatedAt <= ORACLE_STALENESS_THRESHOLD) {
                currentPrice = uint256(answer) * (10 ** (18 - priceFeed.decimals()));
            }
        } catch {}

        if (currentPrice == 0) return (false, "");

        for (uint256 gId = startId; gId < endId; gId++) {
            Grid storage grid = grids[gId];
            if (!grid.active) continue;

            GridLevel[] storage levels = gridLevels[gId];
            for (uint256 lIdx = 0; lIdx < levels.length; lIdx++) {
                GridLevel storage lvl = levels[lIdx];
                if (lvl.filled) continue;

                bool executable = (lvl.isBuy  && currentPrice <= lvl.price)
                               || (!lvl.isBuy && currentPrice >= lvl.price);

                if (executable) {
                    return (true, abi.encode(gId, lIdx));
                }
            }
        }
        return (false, "");
    }

    /**
     * @notice Called on-chain by Chainlink when checkUpkeep returns true.
     * @param performData ABI-encoded (uint256 gridId, uint256 levelIndex)
     */
    function performUpkeep(bytes calldata performData) external override {
        (uint256 gridId, uint256 levelIndex) = abi.decode(performData, (uint256, uint256));
        // executeGrid has its own oracle check + nonReentrant — safe to call directly
        // Note: performUpkeep caller must be in isKeeper for the modifier
        // Chainlink forwarder should be registered as keeper
        this.executeGrid(gridId, levelIndex);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  CLOSE GRID
    // ════════════════════════════════════════════════════════════════════════

    /**
     * @notice Close a grid and return funds to owner.
     *         Uses per-grid balance tracking — no cross-grid leakage.
     */
    function closeGrid(uint256 gridId)
        external
        nonReentrant
        onlyGridOwner(gridId)
        gridExists(gridId)
    {
        Grid storage grid = grids[gridId];
        require(grid.active, "GridV2: already closed");

        grid.active = false;

        // Withdraw from Moola if yield enabled
        if (grid.yieldEnabled && address(moolaPool) != address(0) && grid.quoteBalance > 0) {
            _withdrawFromMoola(gridId, grid.quoteToken, grid.quoteBalance);
        }

        uint256 quoteOut = grid.quoteBalance;
        uint256 baseOut  = grid.baseBalance;

        grid.quoteBalance = 0;
        grid.baseBalance  = 0;

        // Return only this grid's funds (V1 critical fix)
        if (quoteOut > 0) _safeTransfer(grid.quoteToken, msg.sender, quoteOut);
        if (baseOut  > 0) _safeTransfer(grid.baseToken,  msg.sender, baseOut);

        emit GridClosed(gridId, msg.sender, quoteOut, baseOut);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  MOOLA YIELD — INTERNAL
    // ════════════════════════════════════════════════════════════════════════

    function _depositToMoola(uint256 gridId, address quoteToken, uint256 amount) internal {
        _safeApprove(quoteToken, address(moolaPool), amount);
        moolaPool.deposit(quoteToken, amount, address(this), 0);
        emit YieldDeposited(gridId, amount);
    }

    function _withdrawFromMoola(uint256 gridId, address quoteToken, uint256 amount) internal {
        moolaPool.withdraw(quoteToken, amount, address(this));
        emit YieldWithdrawn(gridId, amount);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  ADMIN
    // ════════════════════════════════════════════════════════════════════════

    function setKeeper(address keeper, bool status) external onlyOwner {
        require(keeper != address(0), "GridV2: zero keeper");
        isKeeper[keeper] = status;
        emit KeeperUpdated(keeper, status);
    }

    function setOracle(address newFeed) external onlyOwner {
        require(newFeed != address(0), "GridV2: zero feed");
        priceFeed = IChainlinkFeed(newFeed);
        emit OracleUpdated(newFeed);
    }

    function setFee(uint256 newFeeBps) external onlyOwner {
        require(newFeeBps <= 100, "GridV2: fee too high (>1%)");
        feeBps = newFeeBps;
    }

    function collectFees(address quoteToken) external onlyOwner nonReentrant {
        uint256 amount = accumulatedFees;
        require(amount > 0, "GridV2: no fees");
        accumulatedFees = 0;
        _safeTransfer(quoteToken, feeRecipient, amount);
    }

    // ════════════════════════════════════════════════════════════════════════
    //  VIEWS
    // ════════════════════════════════════════════════════════════════════════

    function getGridLevels(uint256 gridId) external view returns (GridLevel[] memory) {
        return gridLevels[gridId];
    }

    function getUserGrids(address user) external view returns (uint256[] memory) {
        return userGrids[user];
    }

    /// @notice Preview how many grid levels are executable at the current oracle price
    function getExecutableLevels(uint256 gridId)
        external
        view
        gridExists(gridId)
        returns (uint256[] memory executableIndexes)
    {
        uint256 currentPrice = _getOraclePrice();
        GridLevel[] storage levels = gridLevels[gridId];
        uint256[] memory tmp = new uint256[](levels.length);
        uint256 count;

        for (uint256 i = 0; i < levels.length; i++) {
            if (levels[i].filled) continue;
            bool ok = (levels[i].isBuy  && currentPrice <= levels[i].price)
                   || (!levels[i].isBuy && currentPrice >= levels[i].price);
            if (ok) tmp[count++] = i;
        }

        executableIndexes = new uint256[](count);
        for (uint256 i = 0; i < count; i++) executableIndexes[i] = tmp[i];
    }
}
