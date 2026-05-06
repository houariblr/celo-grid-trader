// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface IMento {
    function swapIn(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 amountOutMin
    ) external returns (uint256 amountOut);
}