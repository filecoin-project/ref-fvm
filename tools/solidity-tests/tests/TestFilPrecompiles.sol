// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "../libraries/Test.sol";
import "../libraries/ErrLib.sol";
import "../libraries/FilUtils.sol";
import "../libraries/deployable/Dummy.sol";
import "../libraries/deployable/Lifecycle.sol";

contract TestFilPrecompiles {

    using FilUtils for *;
    using Test for *;
    using ErrLib for *;

    address creator = msg.sender;

    constructor() payable { }

    function run() public returns (string[] memory results) {
        return Test.getRunner()
            .addV(this.test__Resolve_Roundtrip.named("test__Resolve_Roundtrip"))
            .addM(this.test__Resolve_New_Actors.named("test__Resolve_New_Actors"))
            .addM(this.test__Call_Unknown_Precompile.named("test__Call_Unknown_Precompile"))
            .run();
    }

    // Test resolve_address -> lookup_delegated_address roundtrip
    function test__Resolve_Roundtrip() external view {
        (bool success, uint64 id) = address(this).getActorID();
        Test.expect("resolve_address reverted or returned empty").success(success);
        Test.expect("resolved actor id should be valid").gte(id, 100);

        address ethAddress;
        (success, ethAddress) = id.getEthAddress();
        Test.expect("lookup_delegated_address reverted or returned empty").success(success);
        Test.expect("did not roundtrip").eq(ethAddress, address(this));
    }

    // Test resolve_address on fresh actors
    function test__Resolve_New_Actors() external {
        address a = DummyLib.newDummy();
        (bool success, uint64 curId) = a.getActorID();
        Test.expect("resolve_address reverted or returned empty").success(success);
        Test.expect("resolved actor id should be valid").gte(curId, 100);

        // Deploy contract in a loop and check that the ID we retrieve
        // is incremented each time
        uint64 nextId;
        for (uint i = 0; i < 5; i++) {
            a = DummyLib.newDummy();
            
            (success, nextId) = a.getActorID();
            Test.expect("resolve_address reverted or returned empty").success(success);
            Test.expect("actor id should increment").eq(nextId, curId + 1);

            curId = nextId;
        }
    }

    // Test properties of calls to addresses that are formatted like precompiles
    // ... but do not exist
    function test__Call_Unknown_Precompile() external {
        // Unknown EVM precompile - all CALL types
        (bool success, bytes memory data) = address(uint160(100)).call("");
        Test.expect("call should succeed when calling unknown EVM precompile").success(success);
        Test.expect("call should not return anything for unknown EVM precompile").iszero(data.length);
        
        (success, data) = address(uint160(100)).staticcall("");
        Test.expect("staticcall should succeed when calling unknown EVM precompile").success(success);
        Test.expect("staticcall should not return anything for unknown EVM precompile").iszero(data.length);

        (success, data) = address(uint160(100)).delegatecall("");
        Test.expect("delegatecall should succeed when calling unknown EVM precompile").success(success);
        Test.expect("delegatecall should not return anything for unknown EVM precompile").iszero(data.length);

        // Deprecated FIL precompile - all CALL types
        address target = 0xFe00000000000000000000000000000000000004;
        (success, data) = target.call("");
        Test.expect("call should succeed when calling deprecated precompile").success(success);
        Test.expect("call should not return anything for deprecated precompile").iszero(data.length);

        (success, data) = target.staticcall("");
        Test.expect("staticcall should succeed when calling deprecated precompile").success(success);
        Test.expect("staticcall should not return anything for deprecated precompile").iszero(data.length);
        
        (success, data) = target.delegatecall("");
        Test.expect("delegatecall should succeed when calling deprecated precompile").success(success);
        Test.expect("delegatecall should not return anything for deprecated precompile").iszero(data.length);

        // Unknown FIL precompile - all CALL types
        target = uint64(100).toIDAddress();
        (success, data) = target.call("");
        Test.expect("call should succeed when calling unknown precompile").success(success);
        Test.expect("call should not return anything for unknown precompile").iszero(data.length);

        (success, data) = target.staticcall("");
        Test.expect("staticcall should succeed when calling unknown precompile").success(success);
        Test.expect("staticcall should not return anything for unknown precompile").iszero(data.length);
        
        (success, data) = target.delegatecall("");
        Test.expect("delegatecall should succeed when calling unknown precompile").success(success);
        Test.expect("delegatecall should not return anything for unknown precompile").iszero(data.length);
    }
}