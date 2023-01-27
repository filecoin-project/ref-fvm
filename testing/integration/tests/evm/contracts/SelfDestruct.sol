// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract SelfDestructOnCreate {
    // Mark the constructor as payable to be able to receive tokens.
    constructor(address _beneficiary) payable {
        // If there is no non-zero address given, try to refund to self, which should fail.
        address beneficiary = _beneficiary ==
            0x0000000000000000000000000000000000000000
            ? address(this)
            : _beneficiary;

        selfdestruct(payable(beneficiary));
    }
}

// We'll set up multiple contracts, then set up a self
// destruct chain where they all send the refunds to
// the caller in the chain. The outermost account
// should get all the funds.
contract SelfDestructChain {
    // Mark the constructor as payable to be able to receive tokens.
    constructor() payable {}

    // Pass an array of contracts to be destroyed and
    // the current index. If the index is not beyond
    // the size of the address array, pick the address
    // under the current index and call destroy on it.
    // Finally self destruct ourselves with the caller
    // as the beneficiary.
    function destroy(
        // List of contracts to destroy.
        address[] calldata _addresses,
        // The current call depth in the chain.
        uint32 _curr_depth
    ) public {
        // TODO: I think we can call selfdestruct here already.
        if (_curr_depth < _addresses.length) {
            (bool success, ) = _addresses[_curr_depth].call(
                abi.encodeWithSignature(
                    "destroy(address[],uint32)",
                    _addresses,
                    _curr_depth + 1
                )
            );
            require(success);
        }
        selfdestruct(payable(msg.sender));
    }
}

/** Metamorphic contracts. */

interface MetamorphicInterface {
    // Each contract has a description that we can use to assert which one is living
    // at the metamorphic address.
    function description() external pure returns (string memory);
}

contract Cocoon {
    function description() external pure returns (string memory) {
        return "Cocoon";
    }

    /// Self destruct so it can be resurrected as a Butterfly.
    /// Keep the money, if possible.
    function die() public {
        selfdestruct(payable(address(this)));
    }
}

contract Bufferfly {
    function description() external pure returns (string memory) {
        return "Butterfly";
    }
}
