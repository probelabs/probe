// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "../contracts/GovernorExample.sol";

contract GovernorExampleTest {
    GovernorExample private governor;

    function setUp() public {
        governor = GovernorExample(address(0));
    }

    function testVotingDelay() public view {
        require(governor.votingDelay() == 1);
    }
}
