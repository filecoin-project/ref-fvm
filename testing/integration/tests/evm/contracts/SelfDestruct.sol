// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract SelfDestructOnCreate {
    constructor(address _beneficiary) {
        // If there is no non-zero address given, try to refund to self, which should fail.
        address beneficiary = _beneficiary ==
            0x0000000000000000000000000000000000000000
            ? address(this)
            : _beneficiary;

        // WIP: Trying to figure out how to pass constructor arguments.
        require(beneficiary == 0x0000000000000000000000000000000000000000);

        // TODO: How to trigger self-destruct?
    }
}
