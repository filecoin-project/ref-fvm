// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "../../libraries/Test.sol";
import "../../libraries/ErrLib.sol";
import "../../libraries/FilUtils.sol";
import "../../libraries/deployable/Dummy.sol";
import "../../libraries/deployable/Lifecycle.sol";
import "../../libraries/deployable/Nested.sol";

contract TestRecursiveCreate2 {

    using FilUtils for *;
    using Test for *;
    using ErrLib for *;

    address creator = msg.sender;

    constructor() payable { }

    function run() public returns (string[] memory results) {
        return Test.getRunner()
            .addM(this.test__Recursive_Create2.named("test__Recursive_Create2"))
            .run();
    }

    // Nested2 has an identical copy of this variable, so if you change the value... :(
    bytes32 constant SALT = bytes32("saltysaltysalt");

    // Test properties of contracts that deploy themselves
    // We have to give this its own "run" invocation, because
    // we're going to use a ton of gas.
    function test__Recursive_Create2() external {
        // deep recursive CREATE2
        Nested2 n = new Nested2{ salt: SALT }(100);
        
        (address curChild, uint curCount, bytes32 initcodeHash) = n.getChildCountAndHash();
        address calcedAddr = calculateChildCreate2(SALT, initcodeHash, address(n));
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
            
            // Get the next child
            (curChild, curCount, initcodeHash) = Nested2(curChild).getChildCountAndHash();
            calcedAddr = calculateChildCreate2(SALT, initcodeHash, prevChild);
            (success, curId) = prevChild.getActorID();
            Test.expect("should have successfully fetched id at count: ".concat(prevCount)).success(success);
            Test.expect("resolved id should be valid at count: ".concat(prevCount)).gte(curId, 100);
            Test.expect("id should increment by 1").eq(curId, prevId + 1);
        }

        Test.expect("should have ended with count 0").iszero(curCount);
        Test.expect("should have ended with child 0").iszero(curChild);
    }

    // Calculate the address for the child n should deploy
    function calculateChildCreate2(bytes32 salt, bytes32 bytecodeHash, address deployer) internal pure returns (address addr) {
        return address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xFF), deployer, salt, bytecodeHash)))));
    }
}