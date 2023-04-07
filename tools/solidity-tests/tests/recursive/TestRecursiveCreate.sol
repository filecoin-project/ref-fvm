// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "../../libraries/Test.sol";
import "../../libraries/ErrLib.sol";
import "../../libraries/FilUtils.sol";
import "../../libraries/deployable/Dummy.sol";
import "../../libraries/deployable/Lifecycle.sol";
import "../../libraries/deployable/Nested.sol";

contract TestRecursiveCreate {

    using FilUtils for *;
    using Test for *;
    using ErrLib for *;

    address creator = msg.sender;

    constructor() payable { }

    function run() public returns (string[] memory results) {
        return Test.getRunner()
            .addM(this.test__Recursive_Create.named("test__Recursive_Create"))
            .run();
    }

    // Test properties of contracts that deploy themselves
    // We have to give this its own "run" invocation, because
    // we're going to use a ton of gas.
    function test__Recursive_Create() external {
        // deep recursive CREATE
        Nested n = new Nested(100);
        (address curChild, uint curCount) = n.getChildAndCount();
        address calcedAddr = calculateChild(address(n));

        (bool success, uint64 curId) = address(n).getActorID();
        Test.expect("should have successfully fetched id at top level").success(success);
        Test.expect("top level resolved id should be valid").gte(curId, 100);
        
        address prevChild;
        uint64 prevId;
        uint prevCount;
        while (curChild != address(0)) {
            Test.expect("should correctly compute the next address for count: ".concat(curCount)).eq(curChild, calcedAddr);

            prevChild = curChild;
            prevId = curId;
            prevCount = curCount;

            // Update curChild and curCount
            (curChild, curCount) = Nested(curChild).getChildAndCount();
            calcedAddr = calculateChild(prevChild);
            (success, curId) = prevChild.getActorID();
            Test.expect("should have successfully fetched id at count: ".concat(prevCount)).success(success);
            Test.expect("resolved id should be valid at count: ".concat(prevCount)).gte(curId, 100);
            Test.expect("id should increment by 1").eq(curId, prevId + 1);
        }

        Test.expect("should have ended with count 0").iszero(curCount);
        Test.expect("should have ended with child 0").iszero(curChild);
    }

    // Calculate the address for the child n should deploy
    function calculateChild(address n) internal pure returns (address) {
        return address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xd6), bytes1(0x94), n, bytes1(0x01))))));
    }
}