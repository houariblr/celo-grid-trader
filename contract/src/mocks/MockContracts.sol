// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// ─── Mock Chainlink Feed ─────────────────────────────────────────────────────
// يرجع سعر CELO/USD ثابت = $0.45 للـ testnet
contract MockChainlinkFeed {
    int256  private _price     = 45_000_000; // $0.45 with 8 decimals
    uint8   private _decimals  = 8;
    uint256 private _updatedAt = block.timestamp;

    function latestRoundData() external view returns (
        uint80 roundId,
        int256 answer,
        uint256 startedAt,
        uint256 updatedAt,
        uint80 answeredInRound
    ) {
        return (1, _price, block.timestamp, _updatedAt, 1);
    }

    function decimals() external view returns (uint8) {
        return _decimals;
    }

    // للاختبار — تغيير السعر
    function setPrice(int256 newPrice) external {
        _price     = newPrice;
        _updatedAt = block.timestamp;
    }
}

// ─── Mock Mento Exchange ─────────────────────────────────────────────────────
// يعمل 1:1 swap بين أي tokens للـ testnet
contract MockMento {
    mapping(address => uint256) public balances;

    function swapIn(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minAmountOut
    ) external returns (uint256 amountOut) {
        // سحب tokenIn من المرسل
        (bool ok1,) = tokenIn.call(
            abi.encodeWithSignature("transferFrom(address,address,uint256)", msg.sender, address(this), amountIn)
        );
        require(ok1, "MockMento: transferFrom failed");

        // 1:1 swap (للـ testnet فقط)
        amountOut = amountIn;
        require(amountOut >= minAmountOut, "MockMento: slippage");

        // إرسال tokenOut للمرسل — نحتاج mint إذا لم يكن عندنا رصيد
        // نستخدم MockERC20 mint function
        (bool ok2,) = tokenOut.call(
            abi.encodeWithSignature("mint(address,uint256)", msg.sender, amountOut)
        );
        require(ok2, "MockMento: mint failed");

        return amountOut;
    }
}

// ─── Mock cUSD / CELO ERC20 ──────────────────────────────────────────────────
contract MockERC20 {
    string  public name;
    string  public symbol;
    uint8   public decimals = 18;
    uint256 public totalSupply;

    mapping(address => uint256)                     public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    event Transfer(address indexed from, address indexed to, uint256 amount);
    event Approval(address indexed owner, address indexed spender, uint256 amount);

    constructor(string memory _name, string memory _symbol) {
        name   = _name;
        symbol = _symbol;
    }

    function mint(address to, uint256 amount) external {
        totalSupply      += amount;
        balanceOf[to]    += amount;
        emit Transfer(address(0), to, amount);
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[to]         += amount;
        emit Transfer(msg.sender, to, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        allowance[from][msg.sender] -= amount;
        balanceOf[from]             -= amount;
        balanceOf[to]               += amount;
        emit Transfer(from, to, amount);
        return true;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }
}
