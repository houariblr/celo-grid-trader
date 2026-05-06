// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IChainlinkFeed {
    // هذه هي الدالة التي كانت مفقودة وتسببت في الخطأ
    function decimals() external view returns (uint8);

    function latestRoundData()
        external
        view
        returns (
            uint80 roundId,
            int256 answer,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        );
}
