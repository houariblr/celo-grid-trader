// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IAutomationCompatible {
    function checkUpkeep(bytes calldata checkData) external returns (bool upkeepNeeded, bytes memory performData);
    function performUpkeep(bytes calldata performData) external;
}
