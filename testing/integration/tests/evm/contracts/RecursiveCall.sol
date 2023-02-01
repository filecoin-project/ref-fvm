// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

// The purpose of this contract is to test CALL, DELEGATECALL, STATICCALL,
// eventually interspersed with REVERT and SELFDESTRUCT.
//
// The idea with the recursion is to have a list of addresses to call
// at each depth of the recursion, with a given call type, then check
// the state variables to see if they are as we expected.
contract RecursiveCall {
    // What action should be taken at a given depth.
    enum Action {
        DELEGATECALL,
        CALL,
        REVERT
    }

    uint32 public depth;
    address public sender;
    uint256 public value;

    // Pass a list of contract addresses to call at subsequent depths.
    // If the recursion is deeper than the number of addresses, the last
    // contract should call `this`.
    function recurse(
        address[] calldata addresses,
        Action[] calldata actions,
        uint32 max_depth,
        uint32 curr_depth
    ) public payable returns (bool) {
        depth = curr_depth;
        sender = msg.sender;
        value = msg.value;
        bool success = true;

        Action action = curr_depth < actions.length
            ? actions[curr_depth]
            : actions[actions.length - 1];

        address callee = curr_depth < actions.length
            ? addresses[curr_depth]
            : address(this);

        bool can_recurse = max_depth > curr_depth;

        if (action == Action.DELEGATECALL && can_recurse) {
            (success, ) = callee.delegatecall(
                abi.encodeWithSignature(
                    "recurse(address[],uint8[],uint32,uint32)",
                    addresses,
                    actions,
                    max_depth,
                    curr_depth + 1
                )
            );
        } else if (action == Action.CALL && can_recurse) {
            (success, ) = callee.call(
                abi.encodeWithSignature(
                    "recurse(address[],uint8[],uint32,uint32)",
                    addresses,
                    actions,
                    max_depth,
                    curr_depth + 1
                )
            );
        } else if (action == Action.REVERT) {
            revert();
        }

        return success;
    }
}
