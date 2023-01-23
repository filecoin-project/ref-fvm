// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

// The purpose of this contract is to test CALL, DELEGATECALL, STATICCALL,
// eventually interspersed with REVERT and SELFDESTRUCT.
//
// The idea with the recursion is to have a list of addresses to call
// at each depth of the recursion, with a given call type, then check
// the state variables to see if they are as we expected.
contract RecursiveCallInner {
    // NOTE: storage layout must be the same as contract `RecursiveCallOuter`.
    uint32 public depth;
    address public sender;
    uint256 public value;

    // Pass a list of inner contract addresses to call at subsequent depths.
    // If the recursion is deeper than the number of addresses, the last
    // contract should call itself.
    function recurse(
        address[] calldata addresses,
        uint32 max_depth,
        uint32 curr_depth
    ) public payable {
        depth = curr_depth;
        sender = msg.sender;
        value = msg.value;

        if (max_depth > curr_depth) {
            address callee = addresses.length > curr_depth
                ? addresses[curr_depth]
                : address(this);
            (bool success, ) = callee.delegatecall(
                abi.encodeWithSignature(
                    "recurse(address[],uint,uint)",
                    addresses,
                    max_depth,
                    curr_depth + 1
                )
            );
            require(success, "recursive call failed in inner");
        }
    }
}

// This is separate from `RecursiveCallInner` so we have an example of multiple contracts living in the file,
// and for two contracts having the same storage layout.
contract RecursiveCallOuter {
    uint32 public depth;
    address public sender;
    uint256 public value;

    // Pass a list of inner contract addresses to call at subsequent depths.
    // If the recursion is deeper than the number of addresses, the last
    // contract should call itself.
    function recurse(address[] calldata addresses, uint32 max_depth)
        public
        payable
        returns (bool)
    {
        if (max_depth == 0) {
            depth = 0;
            sender = msg.sender;
            value = msg.value;
            return true;
        }
        require(
            addresses.length > 0,
            "need at least 1 address for non-zero depth"
        );
        (bool success, ) = addresses[0].delegatecall(
            abi.encodeWithSignature(
                "recurse(address[],uint,uint)",
                addresses,
                max_depth,
                1
            )
        );
        return success;
    }
}
