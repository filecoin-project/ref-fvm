// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "./ErrLib.sol";

library Test {

    using ErrLib for *;

    struct TestRunner {
        TestFn[] tests;
    }

    struct TestFn {
        string name;
        Mut mutability;
        function() external test;
    }

    enum Mut {
        VIEW,
        MUTABLE,
        PAYABLE
    }

    // Storing TestRunner at slot[some-hash] means test contracts
    // can implement whatever storage footprint they want and there
    // won't be any overwriting.
    bytes32 constant STORAGE_SLOT = keccak256(bytes("test-storage-location"));

    function getRunner() internal pure returns (TestRunner storage t) {
        bytes32 slot = STORAGE_SLOT;
        assembly { t.slot := slot }
    }

    function run(TestRunner storage tr) internal returns (string[] memory results) {
        results = new string[](tr.tests.length + 1);
        uint passCount;

        // Iterate over added tests and run each. Tests that revert should
        // return an error message. These are tracked in the results array, 
        // which we return from this method.

        uint failCount = 1;
        for (uint i = 0; i < tr.tests.length; i++) {
            TestFn storage t = tr.tests[i];

            // Run test. If target is payable or view, we
            // cast the test function.
            // view fns will be called with staticcall
            // payable fns will be called with some amount of FIL

            if (t.mutability == Mut.PAYABLE) {
                function() external payable payFn = toPayable(t.test);
                try payFn{ value: 100 }() {
                    passCount++;
                    continue;
                } catch Error(string memory reason) {
                    results[failCount] = getFailureString(i, t.name, reason);
                } catch Panic(uint err) {
                    results[failCount] = getPanicString(i, t.name, err);
                } catch {
                    results[failCount] = getUnknownErrString(i, t.name);
                }
            } else if (t.mutability == Mut.VIEW) {
                function() external view viewFn = toView(t.test);
                try viewFn() {
                    passCount++;
                    continue;
                } catch Error(string memory reason) {
                    results[failCount] = getFailureString(i, t.name, reason);
                } catch Panic(uint err) {
                    results[failCount] = getPanicString(i, t.name, err);
                } catch {
                    results[failCount] = getUnknownErrString(i, t.name);
                }
            } else {
                function() external mutFn = t.test;
                try mutFn() {
                    passCount++;
                    continue;
                } catch Error(string memory reason) {
                    results[failCount] = getFailureString(i, t.name, reason);
                } catch Panic(uint err) {
                    results[failCount] = getPanicString(i, t.name, err);
                } catch {
                    results[failCount] = getUnknownErrString(i, t.name);
                }
            }

            failCount++;
        }

        results[0] = getResultsString(passCount, tr.tests.length);

        // Manually update the length of results
        assembly { mstore(results, failCount) }
    }

    /**
     * Use these methods to add tests to the runner. 
     * - addV: If your test won't need any state changes (test fn is view)
     * - addM: If your test will need state changes (test fn is default - mutable)
     * - addP: If your test wants some Fil to play with (test fn is payable)
     * 
     * These all return the passed-in TestRunner, so you can use this syntax:
     * - testRunner.add(test1).add(test2).add(test3)...
     *
     * Unfortunately, a bug in solc means we can't overload a function
     * accepts function types with different mutability requirements.
     * So while I'd like to refactor these to be called "add" and accept
     * a raw function type, we'll have to make do with "addM", "addP", and "addV" 
     * for mutable, payable, and view functions, respectively. Sorry.
     * 
     * https://github.com/ethereum/solidity/issues/13879
     */

    function addV(TestRunner storage tr, TestFn memory t) internal returns (TestRunner storage) {
        t.mutability = Mut.VIEW;
        tr.tests.push(t);
        return tr;
    }

    function addM(TestRunner storage tr, TestFn memory t) internal returns (TestRunner storage) {
        t.mutability = Mut.MUTABLE;
        tr.tests.push(t);
        return tr;
    }

    function addP(TestRunner storage tr, TestFn memory t) internal returns (TestRunner storage) {
        t.mutability = Mut.PAYABLE;
        tr.tests.push(t);
        return tr;
    }

    /**
     * This method allows easy construction of TestFn structs, and should be used
     * with the "add" methods above to easily add named tests to the runner.
     * 
     * If a test fails, the name you give it will be printed to the console along
     * with the error message it failed with. 
     * e.g. "test__Create2 failed with: expected nonzero addr"
     *
     * Together with the "add" methods, adding tests to the runner looks something like:
     * - testRunner.addV(this.testFuncA.named("Test1")).addM(this.testFuncB.named("Test2"))...
     */

    function named(function() external fn, string memory name) internal pure returns (TestFn memory t) {
        t.name = name;
        t.test = fn;
    }

    /**
     * Conversions between function types with different mutability requirements
     */
    
    function toView(function() external fn) internal pure returns (function() external view viewFn) {
        assembly {
            viewFn.address := fn.address
            viewFn.selector := fn.selector
        }
    }

    function toMut(function() external view viewFn) internal pure returns (function() external fn) {
        assembly {
            fn.address := viewFn.address
            fn.selector := viewFn.selector
        }
    }

    function toPayable(function() external fn) internal pure returns (function() external payable payFn) {
        assembly {
            payFn.address := fn.address
            payFn.selector := fn.selector
        }
    }

    function getResultsString(uint passCount, uint totalTests) internal pure returns (string memory) {
        string memory result = string("\"").concat(passCount).concat(string(" out of ")).concat(totalTests);
        if (passCount == totalTests) {
            return result.concat(string(" tests passing.\""));
        } else {
            return result.concat(string(" tests passing. Failures:\""));
        }
    }
    
    function getFailureString(uint testNo, string memory name, string memory reason) private pure returns (string memory) {
        return string("\"Test ").concat(testNo+1).concat(string(" (")).concat(name).concat(string(") failed with: ")).concat(reason).concat(string("\""));
    }

    function getPanicString(uint testNo, string memory name, uint err) private pure returns (string memory) {
        return string("\"Test ").concat(testNo+1).concat(string(" (")).concat(name).concat(string(") paniced with errNo: ")).concat(err).concat(string("\""));
    }

    function getUnknownErrString(uint testNo, string memory name) private pure returns (string memory) {
        return string("\"Test ").concat(testNo+1).concat(string(" (")).concat(name).concat(string(") failed with unknown error\""));
    }

    /**
     * Assertions
     */

    // -- both values are: $a
    string constant NEQ = " -- both values are: ";
    // -- got nonzero value: $a
    string constant IS_ZERO = " -- got nonzero value: ";

    // Used with separators for error messages with 2 values
    string constant EXPECTED = " -- expected ";
    string constant SEP_EQ = " == ";  // -- expected $a == $b
    string constant SEP_GT = " > ";   // -- expected $a > $b
    string constant SEP_GTE = " >= "; // -- expected $a >= $b
    string constant SEP_LT = " < ";   // -- expected $a < $b
    string constant SEP_LTE = " <= "; // -- expected $a <= $b

    // bool
    string constant EXPECTED_TRUE = " -- expected true, got false";
    string constant EXPECTED_FALSE = " -- expected false, got true";

    string constant ASSERT_FAIL = " -- assertion failure: ";

    function expect(string memory str) internal pure returns (string memory) {
        return str;
    }

    function fail(string memory str) internal pure {
        revert(ASSERT_FAIL.concat(str));
    }

    /**
     * address assertions:
     * - eq
     * - neq
     * - iszero
     */

    function eq(string memory str, address a, address b) internal pure {
        if (a == b) return;

        revert(str.concat(EXPECTED).concat(a).concat(SEP_EQ).concat(b));
    }

    function neq(string memory str, address a, address b) internal pure {
        if (a != b) return;

        revert(str.concat(NEQ).concat(a));
    }

    function iszero(string memory str, address a) internal pure {
        if (a == address(0)) return;

        revert(str.concat(IS_ZERO).concat(a));
    }

    /**
     * uint assertions:
     * - eq
     * - neq
     * - iszero
     * - gt
     * - gte
     * - lt
     * - lte
     */

    function eq(string memory str, uint a, uint b) internal pure {
        if (a == b) return;

        revert(str.concat(EXPECTED).concat(a).concat(SEP_EQ).concat(b));
    }

    function neq(string memory str, uint a, uint b) internal pure {
        if (a != b) return;

        revert(str.concat(NEQ).concat(a));
    }

    function iszero(string memory str, uint a) internal pure {
        if (a == uint(0)) return;

        revert(str.concat(IS_ZERO).concat(a));
    }

    function gt(string memory str, uint a, uint b) internal pure {
        if (a > b) return;

        revert(str.concat(EXPECTED).concat(a).concat(SEP_GT).concat(b));
    }

    function gte(string memory str, uint a, uint b) internal pure {
        if (a >= b) return;

        revert(str.concat(EXPECTED).concat(a).concat(SEP_GTE).concat(b));
    }

    function lt(string memory str, uint a, uint b) internal pure {
        if (a < b) return;

        revert(str.concat(EXPECTED).concat(a).concat(SEP_LT).concat(b));
    }

    function lte(string memory str, uint a, uint b) internal pure {
        if (a <= b) return;

        revert(str.concat(EXPECTED).concat(a).concat(SEP_LTE).concat(b));
    }

    /**
     * bytes32 assertions:
     * - eq
     * - neq
     * - iszero
     */

    function eq(string memory str, bytes32 a, bytes32 b) internal pure {
        if (a == b) return;

        revert(str.concat(EXPECTED).concat(a).concat(SEP_EQ).concat(b));
    }

    function neq(string memory str, bytes32 a, bytes32 b) internal pure {
        if (a != b) return;

        revert(str.concat(NEQ).concat(a));
    }

    function iszero(string memory str, bytes32 a) internal pure {
        if (a == bytes32(0)) return;

        revert(str.concat(IS_ZERO).concat(a));
    }

    /**
     * bool assertions
     * - success
     * - fail
     */

    function success(string memory str, bool cond) internal pure {
        if (cond) return;

        revert(str.concat(EXPECTED_TRUE));
    }

    function fail(string memory str, bool cond) internal pure {
        if (!cond) return;

        revert(str.concat(EXPECTED_FALSE));
    }
}