// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "../libraries/Test.sol";
import "../libraries/ErrLib.sol";
import "../libraries/FilUtils.sol";
import "../libraries/deployable/Dummy.sol";
import "../libraries/deployable/Lifecycle.sol";
import "../libraries/deployable/Nested.sol";

// We're going to be delegatecalling Dummy, so inheriting it
// means we mimic its storage layout and callable functions.
contract TestStaticContext is Dummy {

    using FilUtils for *;
    using Test for *;
    using ErrLib for *;

    address creator = msg.sender;
    address dummy = DummyLib.newDummy();

    constructor() payable { }

    function run() public returns (string[] memory results) {
        return Test.getRunner()
            .addV(this.test__Call_Within_Staticcall.named("test__Call_Within_Staticcall"))
            .addV(this.test__Delegatecall_Within_Staticcall.named("test__Delegatecall_Within_Staticcall"))
            .run();
    }

    /**
     * We're marking these "view" and using "addV" to add it
     * to the runner above, which means these methods will get
     * called using STATICCALL.
     * 
     * So everything here is in a static context already.
     * 
     * However, even though the EVM allows CALL/DELEGATECALL
     * in a static context, Solidity doesn't like it and
     * won't let you mark a function "view" if it contains
     * a CALL or DELEGATECALL. So the weirdness below is
     * because we're using function pointers to trick the
     * compiler.
     */

    function test__Call_Within_Staticcall() external view {
        function() internal view returns (bool, bytes memory) callFn;

        // CALL incrementValue - tries to modify state via SSTORE
        callFn = cast(call_incrementValue);
        (bool success, bytes memory data) = callFn();
        Test.expect("modifying state in a static context should fail").fail(success);
        Test.expect("there should be no return data").iszero(data.length);

        // CALL logValue - tries to modify state via LOG
        callFn = cast(call_logValue);
        (success, data) = callFn();
        Test.expect("logging in a static context should fail").fail(success);
        Test.expect("there should be no return data").iszero(data.length);

        // CALL getValueView
        callFn = cast(call_getValueView);
        (success, data) = callFn();
        uint valueRead = abi.decode(data, (uint));
        Test.expect("using CALL to read state should be fine").success(success);
        Test.expect("return should decode to uint(100)").eq(valueRead, 100);

        // CALL getValuePure
        callFn = cast(call_getValuePure);
        (success, data) = callFn();
        valueRead = abi.decode(data, (uint));
        Test.expect("using CALL to read bytecode should be fine").success(success);
        Test.expect("return should decode to uint(42)").eq(valueRead, 42);
    }

    function test__Delegatecall_Within_Staticcall() external view {
        function() internal view returns (bool, bytes memory) callFn;

        // DELEGATECALL incrementValue - tries to modify state via SSTORE
        callFn = cast(dcall_incrementValue);
        (bool success, bytes memory data) = callFn();
        Test.expect("modifying state in a static context should fail").fail(success);
        Test.expect("there should be no return data").iszero(data.length);

        // DELEGATECALL logValue - tries to modify state via LOG
        callFn = cast(dcall_logValue);
        (success, data) = callFn();
        Test.expect("logging in a static context should fail").fail(success);
        Test.expect("there should be no return data").iszero(data.length);

        // DELEGATECALL getValueView
        callFn = cast(dcall_getValueView);
        (success, data) = callFn();
        uint valueRead = abi.decode(data, (uint));
        Test.expect("using DELEGATECALL to read state should be fine").success(success);
        Test.expect("return should decode to uint(100)").eq(valueRead, 100);

        // DELEGATECALL getValuePure
        callFn = cast(dcall_getValuePure);
        (success, data) = callFn();
        valueRead = abi.decode(data, (uint));
        Test.expect("using DELEGATECALL to read bytecode should be fine").success(success);
        Test.expect("return should decode to uint(42)").eq(valueRead, 42);
    }

    /**
     * CALL methods:
     */

    // Use CALL to increment and retrieve a value from Dummy.
    // This tries to modify state via SSTORE
    function call_incrementValue() internal returns (bool success, bytes memory data) {
        bytes memory input = abi.encodeWithSelector(Dummy.incrementValue.selector, "");
        (success, data) = dummy.call(input);
    }

    // Use CALL to LOG and retrieve a value from Dummy.
    // This tries to modify state via LOG
    function call_logValue() internal returns (bool success, bytes memory data) {
        bytes memory input = abi.encodeWithSelector(Dummy.logValue.selector, "");
        (success, data) = dummy.call(input);
    }

    // Use CALL to read a value from Dummy state without attempting to change state
    function call_getValueView() internal returns (bool success, bytes memory data) {
        bytes memory input = abi.encodeWithSelector(Dummy.getValueView.selector, "");
        (success, data) = dummy.call(input);
    }

    // Use CALL to read a value from Dummy bytecode without attempting to change state
    function call_getValuePure() internal returns (bool success, bytes memory data) {
        bytes memory input = abi.encodeWithSelector(Dummy.getValuePure.selector, "");
        (success, data) = dummy.call(input);
    }

    /**
     * DELEGATECALL methods:
     */

    // Use DELEGATECALL to increment and retrieve a value from our state
    // This one tries to modify state.
    function dcall_incrementValue() internal returns (bool success, bytes memory data) {
        bytes memory input = abi.encodeWithSelector(Dummy.incrementValue.selector, "");
        (success, data) = dummy.delegatecall(input);
    }

    // Use DELEGATECALL to LOG and retrieve a value from Dummy.
    // This tries to modify state via LOG
    function dcall_logValue() internal returns (bool success, bytes memory data) {
        bytes memory input = abi.encodeWithSelector(Dummy.logValue.selector, "");
        (success, data) = dummy.delegatecall(input);
    }

    // Use DELEGATECALL to read a value from our state without attempting to change state
    function dcall_getValueView() internal returns (bool success, bytes memory data) {
        bytes memory input = abi.encodeWithSelector(Dummy.getValueView.selector, "");
        (success, data) = dummy.delegatecall(input);
    }

    // Use DELEGATECALL to read a value from our bytecode without attempting to change state
    function dcall_getValuePure() internal returns (bool success, bytes memory data) {
        bytes memory input = abi.encodeWithSelector(Dummy.getValuePure.selector, "");
        (success, data) = dummy.delegatecall(input);
    }

    // Use assembly to cast a non-view function to "view"
    // This rigmarole is to fool the compiler into letting us use CALL
    // in a STATIC context.
    function cast(function() internal returns (bool, bytes memory) target) 
        internal 
        pure 
        returns (function() internal view returns (bool, bytes memory) callFn) 
    {
        assembly { callFn := target }
    }
}