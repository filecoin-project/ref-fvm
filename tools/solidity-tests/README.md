# Alex Wade's Solidity Tests
This directory includes a test suite, developed by Alex Wade, using fvm-bench.

## Running Solidity Tests

### Install

* solc-js: `npm i solc`
* foundry-rs: https://book.getfoundry.sh/getting-started/installation

### Run Tests

* `chmod a+x ./script.sh`

... then `./script.sh` will run tests!

### What this looks like

In order, `./script.sh` does this:

0. Clear the `/contracts-output` folder
1. Compile all `.sol` files located in the `/tests` folder, placing the results in `contracts-output`
2. For each compiled `/tests/*.sol` file, run `fvm-bench` and call the `run()` method on the target. Pass in empty calldata.
3. Parse output using `cast` (a tool in foundry-rs). Errors from failing tests are printed to the console.

Example output:
```
$ ./script.sh
Compiling ./tests/TestEVMPrecompiles.sol
Compiling ./tests/TestFilPrecompiles.sol
Compiling ./tests/TestLifecycle.sol
Testing contracts...

Test results for ./contracts-output/tests_TestEVMPrecompiles_sol_TestEVMPrecompiles.bin:
==========
Gas Used: 9156564
1 out of 1 tests passing.
==========

Test results for ./contracts-output/tests_TestFilPrecompiles_sol_TestFilPrecompiles.bin:
==========
Gas Used: 70394037
2 out of 3 tests passing. Failures:
Test 2 (test__ActorType) failed with: builtin singleton should be system type -- expected 0 == 1
==========

Test results for ./contracts-output/tests_TestLifecycle_sol_TestLifecycle.bin:
==========
Gas Used: 105440457
3 out of 4 tests passing. Failures:
Test 2 (test__Create_Selfdestruct) failed with: should have no codesize after selfdestruct -- got nonzero value: 420
==========

```

### How to add a test

Tests are located in the `/tests` folder. Each test contract defines the same entry point - the `run` method. Example:

```solidity
function run() public returns (string[] memory results) {
  return Test.getRunner()
    .addM(this.test__Create_Codesize.named("test__Create_Codesize"))
    .addP(this.test__Create_Ctx.named("test__Create_Ctx"))
    .addV(this.test__Create_Rdonly.named("test__Create_Rdonly"))
    .run();
}
```

Within the `run` method, we use the `Test` library (`./libraries/Test.sol`) to set up and run tests. As shown in the example above, tests can be added to the `TestRunner` returned by `Test.getRunner()`. Adding tests in this way places them in contract storage until they're ready to be run - which we do at the end using `.run()`.

We can select whether the tests are called using `STATICCALL`, `CALL`, or `CALL` with value by using:
* `.addV` -> "add view," adds a `view` test that will be called with `STATICCALL`
* `.addM` -> "add mut," adds a test that will be called with `CALL`
* `.addP` -> "add payable," adds a test that will be called with `CALL` and sent some Fil.

Tests are defined like this:

```solidity
// Test resolve_address -> lookup_delegated_address roundtrip
function test__ResolveRoundtrip() external view {
  (bool success, uint64 id) = address(this).getActorID();
  Test.expect("resolve_address reverted or returned empty").success(success);
  Test.expect("resolved actor id should be valid").gte(id, 100);

  address ethAddress;
  (success, ethAddress) = id.getEthAddress();
  Test.expect("lookup_delegated_address reverted or returned empty").success(success);
  Test.expect("did not roundtrip").eq(ethAddress, address(this));
}
```

The `Test.expect` method attaches nicely-formatted error messages to assertions. Generally, the syntax is:
* `Test.expect("error message here").gt(1, 5);`

... Which asserts that `1 > 5`, and fails the test with an error message using `REVERT`. This message is caught by the `TestRunner`, and printed out after all tests are run.

There are multiple assertions available in the `Test` library, and it's pretty easy to add more if there's an assertion you need.

**Remember:** Every time you add a test function to a contract, you must also add it to the `TestRunner` or it will not be run!
