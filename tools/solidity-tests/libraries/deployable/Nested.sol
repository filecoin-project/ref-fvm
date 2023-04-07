// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "../EVMUtils.sol";

contract Nested {

    address child;
    uint currentCount;

    // Deploys a copy of itself with recurseCount - 1
    // When recurseCount hits 0, returns without deploying
    constructor(uint recurseCount) {
        if (recurseCount == 0) return;

        // Get our initcode and decrement recurseCount for the next CREATE
        bytes memory code = EVMUtils.getOwnCode();
        address result;
        assembly {
            // This points to the last 32 bytes of code
            // ... which is where recurseCount is stored
            let ptr := add(code, mload(code))
            mstore(ptr, sub(recurseCount, 1))

            // Deploy with recurseCount - 1
            result := create(0, add(32, code), mload(code))
        }

        // Post deployment - record child and current recurseCount
        child = result;
        currentCount = recurseCount;
    }

    function getChildAndCount() public view returns (address, uint) {
        return (child, currentCount);
    }
}

contract Nested2 {

    address child;
    uint currentCount;
    bytes32 initcodeHash;

    bytes32 constant SALT = bytes32("saltysaltysalt");

    // Using CREATE2, deploys a copy of itself with recurseCount - 1
    // When recurseCount hits 0, returns without deploying
    constructor(uint recurseCount) {
        if (recurseCount == 0) return;

        // Get our initcode and decrement recurseCount for the next CREATE
        bytes memory code = EVMUtils.getOwnCode();
        bytes32 salt = SALT;
        address result;
        assembly {
            // This points to the last 32 bytes of code
            // ... which is where recurseCount is stored
            let ptr := add(code, mload(code))
            mstore(ptr, sub(recurseCount, 1))

            // Deploy with recurseCount - 1
            result := create2(0, add(32, code), mload(code), salt)
        }

        // Post deployment - record child and current recurseCount
        child = result;
        currentCount = recurseCount;
        // Record initcode hash for CREATE2 calc
        initcodeHash = keccak256(code);
    }

    function getChildCountAndHash() public view returns (address, uint, bytes32) {
        return (child, currentCount, initcodeHash);
    }
}