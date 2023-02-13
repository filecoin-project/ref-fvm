// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "../libraries/Test.sol";
import "../libraries/ErrLib.sol";
import "../libraries/FilUtils.sol";
import "../libraries/deployable/Dummy.sol";
import "../libraries/EVMUtils.sol";

contract TestEVMPrecompiles {

    using FilUtils for *;
    using Test for *;

    address creator = msg.sender;

    constructor() payable { }

    function run() public returns (string[] memory results) {
        return Test.getRunner()
            .addV(this.test__Identity.named("test__Identity"))
            .run();
    }

    
    // Test identity precompile
    function test__Identity() external view {
        // Empty data
        bytes memory empty = new bytes(0);
        (bool success, bytes memory copy) = EVMUtils.copyData(empty);
        Test.expect("identity precompile reverted or returned invalid data").success(success);
        Test.expect("copy should equal original").eq(hash(empty), hash(copy));

        // One byte
        bytes memory single = abi.encodePacked(uint8(42));
        (success, copy) = EVMUtils.copyData(single);
        Test.expect("identity precompile reverted or returned invalid data").success(success);
        Test.expect("copy should equal original").eq(hash(single), hash(copy));

        // Lotsa bytes
        bytes memory multi = abi.encodePacked(creator, msg.sender, block.timestamp, tx.origin);
        (success, copy) = EVMUtils.copyData(multi);
        Test.expect("identity precompile reverted or returned invalid data").success(success);
        Test.expect("copy should equal original").eq(hash(multi), hash(copy));
    }

    function hash(bytes memory b) internal pure returns (bytes32) {
        return keccak256(b);
    }
}