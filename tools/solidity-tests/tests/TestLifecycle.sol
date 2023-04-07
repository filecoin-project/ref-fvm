// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "../libraries/Test.sol";
import "../libraries/ErrLib.sol";
import "../libraries/FilUtils.sol";
import "../libraries/deployable/Dummy.sol";
import "../libraries/deployable/Lifecycle.sol";
import "../libraries/deployable/Nested.sol";

contract TestLifecycle {

    using EVMUtils for *;
    using FilUtils for *;
    using Test for *;
    using ErrLib for *;

    address creator = msg.sender;

    constructor() payable { }

    function run() public returns (string[] memory results) {
        return Test.getRunner()
            .addM(this.test__Create_Codesize.named("test__Create_Codesize"))
            .addP(this.test__Create_Ctx.named("test__Create_Ctx"))
            .addP(this.test__Create_Selfdestruct.named("test__Create_Selfdestruct"))
            .addP(this.test__Selfdestruct_After_Create.named("test__Selfdestruct_After_Create"))
            .addP(this.test__Selfdestruct_Multi.named("test__Selfdestruct_Multi"))
            .addP(this.test__Selfdestruct_Prefund_Child.named("test__Selfdestruct_Prefund_Child"))
            .addP(this.test__Send_Precompile_Value.named("test__Send_Precompile_Value"))
            .run();
    }

    // Test expected codesize and codehash values
    function test__Create_Codesize() external {
        Lifecycle l = new Lifecycle();

        // Lifecycle recorded various codesize/hash values during construction:
        (uint selfCodesize, uint extCodesize, bytes32 selfCodehash, bytes32 extCodehash) = l.getRecordedCodeVals();
        Test.expect("self codesize should be nonzero during constructor").neq(selfCodesize, 0);
        Test.expect("extcodesize should be zero during constructor").iszero(extCodesize);
        Test.expect("self codehash should not match empty account").neq(selfCodehash, FilUtils.EVM_EMPTY_CODEHASH);
        Test.expect("extcodehash should match empty account during constructor").eq(extCodehash, FilUtils.EVM_EMPTY_CODEHASH);

        // Compare against values we can calculate here:
        uint calcedSize = type(Lifecycle).creationCode.length;
        bytes32 calcedHash = keccak256(type(Lifecycle).creationCode);
        Test.expect("self codesize should match creation code length").eq(selfCodesize, calcedSize);
        Test.expect("self codehash should match creation code hash").eq(calcedHash, selfCodehash);
        
        // Now update the values and check against prev:
        (uint newSelfCS, uint newExtCS, bytes32 newSelfCH, bytes32 newExtCH) = l.updateCodeVals();
        Test.expect("self codesize and extcodesize should match after construction").eq(newSelfCS, newExtCS);
        Test.expect("self codehash and extcodehash should match after construction").eq(newSelfCH, newExtCH);

        // Compare against values we can calculate here:
        calcedSize = type(Lifecycle).runtimeCode.length;
        calcedHash = keccak256(type(Lifecycle).runtimeCode);
        Test.expect("codesize should match runtime code length").eq(newSelfCS, calcedSize);
        Test.expect("codehash should match runtime code hash").eq(newSelfCH, calcedHash);
    }

    // Test properties of various call-context-related params
    function test__Create_Ctx() external payable {
        Lifecycle l = new Lifecycle();

        // Lifecycle recorded various call context values during construction:
        Lifecycle.Ctx memory ctx = l.getRecordedCallCtxVals();
        Test.expect("should know own address").eq(ctx.self, address(l));
        Test.expect("should agree on tx origin").eq(ctx.origin, tx.origin);
        Test.expect("sender should be this contract").eq(ctx.sender, address(this));
        Test.expect("should not have been sent value").iszero(ctx.callValue);
        Test.expect("balance should be zero").iszero(ctx.balance);
        Test.expect("selfbalance should be zero").iszero(ctx.selfBalance);

        // Update recorded values now that constructor is complete. They should all be the same:
        Lifecycle.Ctx memory newCtx = l.updateCallCtxVals();
        Test.expect("addresses should match").eq(ctx.self, newCtx.self);
        Test.expect("origins should match").eq(ctx.origin, newCtx.origin);
        Test.expect("callers should match").eq(ctx.sender, newCtx.sender);
        Test.expect("callvalues should match").eq(ctx.callValue, newCtx.callValue);
        Test.expect("balances should match").eq(ctx.balance, newCtx.balance);
        Test.expect("selfbalances should match").eq(ctx.selfBalance, newCtx.selfBalance);
   
        // Now try the same thing, but with value sent to constructor:
        uint toSend = msg.value;
        Test.expect("we should have some funds to send").neq(toSend, 0);
        uint prevBalance = address(this).balance;

        l = new Lifecycle{ value: toSend }();
        ctx = l.getRecordedCallCtxVals();
        Test.expect("should know own address").eq(ctx.self, address(l));
        Test.expect("should agree on tx origin").eq(ctx.origin, tx.origin);
        Test.expect("sender should be this contract").eq(ctx.sender, address(this));
        Test.expect("should have been sent value").neq(ctx.callValue, 0);
        Test.expect("balance should be equal to value sent").eq(ctx.balance, toSend);
        Test.expect("selfbalance be equal to value sent").eq(ctx.selfBalance, toSend);
        Test.expect("our balance should decrease by sent amount").eq(address(this).balance, prevBalance - toSend);
    }

    // Test properties of selfdestruct in constructor
    function test__Create_Selfdestruct() external payable {
        uint balancePre = address(this).balance;
        uint toSend = msg.value / 2;
        // Sanity check and make sure we got some Fil
        Test.expect("we should have received some Fil to play with").neq(toSend, 0);

        // Deploy 2 contracts, each of which SELFDESTRUCTS during constructor
        // Send half CALLVALUE to each
        address lb1 = address(new LifecycleBlowup{ value: toSend }(true));
        address lb2 = address(new LifecycleBlowup{ value: toSend }(true));

        // Should result in 2 distinct EthAddresses
        Test.expect("lb1 should be nonzero").neq(lb1, address(0));
        Test.expect("lb2 should be nonzero").neq(lb2, address(0));
        Test.expect("lb1 and lb2 should be distinct addresses").neq(lb1, lb2);

        // Neither should have extcodesize, and hash should be equal to the empty codehash
        Test.expect("lb1 should have empty code").iszero(lb1.extCodesize());
        Test.expect("lb1 hash should be empty hash").eq(lb1.extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);
        Test.expect("lb2 should have empty code").iszero(lb2.extCodesize());
        Test.expect("lb2 hash should be empty hash").eq(lb2.extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);

        // Actor ID should increment correctly
        (bool success, uint64 id) = lb1.getActorID();
        uint firstId = id;
        Test.expect("lb1 actor id should resolved successfully").success(success);
        Test.expect("lb1 actor id should be valid").gte(id, 100);
        (success, id) = lb2.getActorID();
        Test.expect("lb2 actor id should resolved successfully").success(success);
        Test.expect("lb2 actor id should be 1 greater than lb1").eq(id, firstId+1);

        // Balance should update correctly
        // This also tests a notable deviation from EVM behavior:
        // LifecycleBlowup has selfdestructed with itself as beneficiary.
        // In the EVM, it shouldn't have a balance. In FEVM, we expect it does.
        uint expectedBalance = balancePre - (2*toSend);
        Test.expect("our balance should have decreased by toSend * 2").eq(address(this).balance, expectedBalance);
        Test.expect("lb1 should have a balance of toSend").eq(lb1.balance, toSend);
        Test.expect("lb2 should have a balance of toSend").eq(lb2.balance, toSend);
    }

    // Test basic properties of selfdestruct -
    function test__Selfdestruct_After_Create() external payable {
        uint toSend = msg.value;
        // Sanity check and make sure we got some Fil
        Test.expect("we should have received some Fil to play with").neq(toSend, 0);

        // Deploy a contract (does NOT selfdestruct in constructor)
        // Send value
        LifecycleBlowup lb = new LifecycleBlowup{ value: toSend }(false);
        // Sanity check that we now have a normal contract deployed:
        Test.expect("lb should be nonzero").neq(address(lb), address(0));
        // ... which has nonzero codesize and codehash
        Test.expect("lb should have code").neq(address(lb).extCodesize(), 0);
        Test.expect("lb codehash should NOT be empty hash").neq(address(lb).extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);
        // ... which has the Fil we sent on construction
        Test.expect("lb should have a balance of toSend").eq(address(lb).balance, toSend);

        // And now the fun!
        // Test retrieving a value after selfdestruct is run in a sub-call:
        uint value = lb.blowUpAndReturn();
        Test.expect("should have returned the correct value").eq(value, 42);
        Test.expect("lb should still have code").neq(address(lb).extCodesize(), 0);
        Test.expect("lb codehash should still not empty").neq(address(lb).extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);
        Test.expect("lb should still have toSend balance").eq(address(lb).balance, toSend);
        // Check if we can still call functions on it from here:
        value = lb.incrementValue();
        Test.expect("should have incremented the value").eq(value, 43);
        value = lb.getValue();
        Test.expect("should reflect the last state change").eq(value, 43);
        value = lb.blowUpAndReturn();
        Test.expect("should still have toSend balance").eq(address(lb).balance, toSend);
        Test.expect("should still return 43").eq(value, 43);
    }

    // Test basic properties of selfdestruct -
    function test__Selfdestruct_Multi() external payable {
        uint balancePre = address(this).balance;
        uint toSend = msg.value;
        // Sanity check and make sure we got some Fil
        Test.expect("we should have received some Fil to play with").neq(toSend, 0);

        // Deploy a contract (does NOT selfdestruct in constructor)
        // Send value
        LifecycleBlowup lb = new LifecycleBlowup{ value: toSend }(false);
        // Sanity check that we now have a normal contract deployed:
        Test.expect("lb should be nonzero").neq(address(lb), address(0));
        // ... which has nonzero codesize and codehash
        Test.expect("lb should have code").neq(address(lb).extCodesize(), 0);
        Test.expect("lb codehash should NOT be empty hash").neq(address(lb).extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);
        // ... which has the Fil we sent on construction
        Test.expect("lb should have a balance of toSend").eq(address(lb).balance, toSend);

        // And now the fun!
        // Test selfdestruct called multiple times
        // The last selfdestruct sends value back to this contract
        uint value = lb.blowUpMultiAndReturn();
        Test.expect("should have returned the correct value").eq(value, 42);
        Test.expect("lb should still have code").neq(address(lb).extCodesize(), 0);
        Test.expect("lb codehash should still not empty").neq(address(lb).extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);
        Test.expect("lb should not have balance").iszero(address(lb).balance);
        Test.expect("our balance should be unchanged").eq(address(this).balance, balancePre);
        // Check if we can still call functions on it from here:
        value = lb.incrementValue();
        Test.expect("should have incremented the value").eq(value, 43);
        value = lb.getValue();
        Test.expect("should reflect the last state change").eq(value, 43);
        value = lb.blowUpAndReturn();
        Test.expect("should still have empty balance").iszero(address(lb).balance);
        Test.expect("should still return 43").eq(value, 43);
    }

    // Use selfdestruct to send funds to a contract before it is created
    function test__Selfdestruct_Prefund_Child() external payable {
        uint toSend = msg.value;
        // Sanity check and make sure we got some Fil
        Test.expect("we should have received some Fil to play with").neq(toSend, 0);

        // Deploy a contract (does NOT selfdestruct in constructor)
        // Send value
        LifecycleBlowup lb = new LifecycleBlowup{ value: toSend }(false);
        // Sanity check that we now have a normal contract deployed:
        Test.expect("lb should be nonzero").neq(address(lb), address(0));
        // ... which has nonzero codesize and codehash
        Test.expect("lb should have code").neq(address(lb).extCodesize(), 0);
        Test.expect("lb codehash should NOT be empty hash").neq(address(lb).extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);
        // ... which has the Fil we sent on construction
        Test.expect("lb should have a balance of toSend").eq(address(lb).balance, toSend);

        // This method uses CREATE to create a child
        // ... then calculates what child it would create with CREATE2+salt
        // ... then uses SELFDESTRUCT to send funds to that child before it is created
        // ... then uses CREATE2 to create the child
        // ... and finally increments a stored value and returns 3 addresses:
        (uint value, address firstChild, address secondChild, address calculatedChild)
            = lb.createPrefundAndBlowUp();

        // Get returned ID
        (bool success, uint64 lbID) = address(lb).getActorID();
        Test.expect("ID should have resolved successfully").success(success);
        Test.expect("returned ID should be valid").gte(lbID, 100);

        Test.expect("value should have incremented").eq(value, 43);
        // Check expected values for first child:
        uint64 childID;
        (success, childID) = firstChild.getActorID();
        Test.expect("ID should have resolved successfully").success(success);
        Test.expect("childID should be parent + 1").eq(childID, lbID+1);
        Test.expect("firstChild should have a nonzero address").neq(firstChild, address(0));
        Test.expect("firstChild should have nonzero codesize").neq(firstChild.extCodesize(), 0);
        Test.expect("firstChild should have nonzero codehash").neq(firstChild.extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);
        Test.expect("firstChild should have empty balance").iszero(firstChild.balance);

        // Check expected values for second child:
        (success, childID) = secondChild.getActorID();
        Test.expect("ID should have resolved successfully").success(success);
        Test.expect("childID should be parent + 2").eq(childID, lbID+2);
        Test.expect("secondChild should be equal to calculated address").eq(secondChild, calculatedChild);
        Test.expect("secondChild should have a nonzero address").neq(secondChild, address(0));
        Test.expect("secondChild should have nonzero codesize").neq(secondChild.extCodesize(), 0);
        Test.expect("secondChild should have nonzero codehash").neq(secondChild.extCodehash(), FilUtils.EVM_EMPTY_CODEHASH);
        Test.expect("secondChild should have been prefunded").eq(secondChild.balance, toSend);
    }

    // Tests what happens when a precompile receives funds via SELFDESTRUCT
    function test__Send_Precompile_Value() external payable {
        uint toSend = msg.value / 2;
        // Sanity check and make sure we got some Fil
        Test.expect("we should have received some Fil to play with").neq(toSend, 0);

        // Deploy a contract that we'll trigger to blow up and send
        // Fil to a precompile
        LifecycleBlowup lb = new LifecycleBlowup{ value: toSend }(false);
        // Check the actor ID and balance of the contract we just deployed
        (bool success, uint64 lbID) = address(lb).getActorID();
        Test.expect("actor id should resolve successfully").success(success);
        Test.expect("actor id should be valid").gte(lbID, 100);
        Test.expect("should have balance of toSend").eq(address(lb).balance, toSend);

        // Check the actor ID and balance of the precompile
        address beneficiary = FilUtils.CALL_ACTOR_BY_ID;
        uint64 pcID;
        (success, pcID) = beneficiary.getActorID();
        Test.expect("actor id should NOT resolve successfully").fail(success);
        Test.expect("actor id should be zero (did not resolve)").iszero(pcID);
        Test.expect("should have zero balance").iszero(beneficiary.balance);

        // Now, selfdestruct some funds to the precompile:
        lb.blowUp(beneficiary);
        // Sanity check that lb no longer has funds
        Test.expect("lb should not have funds").iszero(address(lb).balance);
        Test.expect("should not have funds, even if we ask about its ID addr").iszero(lbID.toIDAddress().balance);

        // Can we get an actor ID now?
        (success, pcID) = beneficiary.getActorID();
        Test.expect("actor id should resolve successfully").success(success);
        Test.expect("actor id should be lbID + 1").eq(pcID, lbID+1);

        // Does the precompile have a balance now? What if we use its ID address?
        Test.expect("should have balance of toSend").eq(beneficiary.balance, toSend);
        address bID = pcID.toIDAddress();
        Test.expect("should have balance of toSend when querying using ID address").eq(bID.balance, toSend);
    }
}