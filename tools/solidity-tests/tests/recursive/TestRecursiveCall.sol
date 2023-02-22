// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "../../libraries/Test.sol";
import "../../libraries/ErrLib.sol";
import "../../libraries/FilUtils.sol";
import "../../libraries/deployable/Dummy.sol";
import "../../libraries/deployable/Lifecycle.sol";
import "../../libraries/deployable/Nested.sol";

contract TestRecursiveCall {

    using FilUtils for *;
    using Test for *;
    using ErrLib for *;

    address creator = msg.sender;

    constructor() payable { }

    function run() public returns (string[] memory results) {
        return Test.getRunner()
            .addM(this.test__Recursive_Call.named("test__Recursive_Call"))
            .addM(this.test__Recursive_Delegatecall.named("test__Recursive_Delegatecall"))
            .addV(this.test__Recursive_Staticcall.named("test__Recursive_Staticcall"))
            .run();
    }

    /**
     * The call stack depth limit is technically 1024, but we can't
     * actually reach that in practice, because each call only sends
     * up to 63/64 available gas. With a gas limit of 10B, we end up failing
     * somewhere in the 200's.
     */
    uint constant RECURSE_COUNT = 200;

    /**
     * Each of these tests tries to recurse RECURSE_COUNT times,
     * each using a different call type.
     *
     * We'd expect to be able to approach the call stack depth limit of 1024 for each.
     */

    function test__Recursive_Call() external {
        try this.recurse(RECURSE_COUNT, false) returns (uint val) {
            Test.expect("should return 42").eq(val, 42);
        } catch Error(string memory reason) {
            revert(reason);
        } catch (bytes memory data) {
            Test.fail("should not have reached this point; got data: ".concat(string(data)));
        }
    }

    function test__Recursive_Delegatecall() external {
        try this.recurse(RECURSE_COUNT, true) returns (uint val) {
            Test.expect("should return 42").eq(val, 42);
        } catch Error(string memory reason) {
            revert(reason);
        } catch (bytes memory data) {
            Test.fail("should not have reached this point; got data: ".concat(string(data)));
        }
    }

    function test__Recursive_Staticcall() external view {
        try this.recurseView(RECURSE_COUNT) returns (uint val) {
            Test.expect("should return 42").eq(val, 42);
        } catch Error(string memory reason) {
            revert(reason);
        } catch (bytes memory data) {
            Test.fail("should not have reached this point; got data: ".concat(string(data)));
        }
    }

    /**
     * The error catching here is a little weird -
     * 
     * Basically, at some point we'll probably fail because
     * we hit the call stack depth, or some other limitation of the FEVM.
     * 
     * When that happens, it's treated like a revert with empty data.
     * The try/catch statements below have 2 different "catches:"
     * 1. If we get zero revert data, we've just hit the FEVM limit.
     *    Create an error string and revert with that.
     * 2. If we get nonzero revert data, that's probably our error string.
     *    Just bubble that string up.
     */

    function recurse(uint val, bool delegate) public returns (uint) {
        if (val == 0) {
            return 42;
        } 
        
        uint gasLeftBefore = gasleft();

        if (delegate) {
            bool success;
            bytes memory data = abi.encodeWithSelector(this.recurse.selector, val-1, delegate);
            (success, data) = address(this).delegatecall(data);
            if (success) {
                assembly { return(add(32, data), mload(data)) }
            } else if (data.length != 0) {
                assembly { revert(add(32, data), mload(data)) }
            } else {
                revert(
                    "call failed at depth: "
                        .concat(RECURSE_COUNT - val)
                        .concat(string("; gas before call: "))
                        .concat(gasLeftBefore)
                        .concat(string("; gas after: "))
                        .concat(gasleft())
                );
            }
        } else {
            try this.recurse(val - 1, delegate) returns (uint v) {
                return v;
            } catch Error(string memory reason) {
                revert(reason);
            } catch {
                revert(
                    "call failed at depth: "
                        .concat(RECURSE_COUNT - val)
                        .concat(string("; gas before call: "))
                        .concat(gasLeftBefore)
                        .concat(string("; gas after: "))
                        .concat(gasleft())
                );
            }
        }
    }

    function recurseView(uint val) public view returns (uint) {
        if (val == 0) {
            return 42;
        }

        uint gasLeftBefore = gasleft();

        try this.recurseView(val - 1) returns (uint v) {
            return v;
        } catch Error(string memory reason) {
            revert(reason);
        } catch {
            revert(
                "call failed at depth: "
                    .concat(RECURSE_COUNT - val)
                    .concat(string("; gas before call: "))
                    .concat(gasLeftBefore)
                    .concat(string("; gas after: "))
                    .concat(gasleft())
            );
        }
    }
}