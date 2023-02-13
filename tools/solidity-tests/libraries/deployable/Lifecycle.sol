// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "./Dummy.sol";
import "../FilUtils.sol";
import "../EVMUtils.sol";

contract Lifecycle {

    // Record of codesize and codehash values
    uint selfCodesize;
    uint extCodesize;
    bytes32 selfCodehash;
    bytes32 extCodehash;

    // Record of call context
    struct Ctx {
        address self;
        address origin;
        address sender;
        uint callValue;
        uint balance;
        uint selfBalance;
    }

    Ctx ctx;    

    constructor() payable {
        updateCodeVals();
        updateCallCtxVals();
    }

    function getRecordedCodeVals() external view returns (uint, uint, bytes32, bytes32) {
        return (selfCodesize, extCodesize, selfCodehash, extCodehash);
    }

    function getRecordedCallCtxVals() external view returns (Ctx memory) {
        return ctx;
    }

    function updateCodeVals() public returns (uint, uint, bytes32, bytes32) {
        selfCodesize = EVMUtils.selfCodesize();
        extCodesize = EVMUtils.extCodesize(address(this));
        selfCodehash = EVMUtils.selfCodehash();
        extCodehash = EVMUtils.extCodehash(address(this));
        return (selfCodesize, extCodesize, selfCodehash, extCodehash);
    }

    function updateCallCtxVals() public payable returns (Ctx memory) {
        ctx.self = address(this);
        ctx.origin = tx.origin;
        ctx.sender = msg.sender;
        ctx.callValue = msg.value;
        uint bal;
        uint selfBal;
        assembly {
            bal := balance(address())
            selfBal := selfbalance()
        }
        ctx.balance = bal;
        ctx.selfBalance = selfBal;
        return ctx;
    }
}

contract LifecycleBlowup {

    uint value = 42;

    constructor(bool blowup) payable {
        if (blowup) {
            selfdestruct(payable(address(this)));
        }
    }

    function blowUp(address beneficiary) public {
        selfdestruct(payable(beneficiary));
    }

    function blowUpAndReturn() public returns (uint) {
        this.blowUp(address(this));
        return value;
    }

    function blowUpMultiAndReturn() public returns (uint) {
        this.blowUp(address(this));
        this.blowUp(msg.sender);
        return this.getValue();
    }

    bytes32 constant SALT = bytes32("saltysaltysalt");

    function createPrefundAndBlowUp() public returns (uint, address, address, address) {
        Dummy d1 = new Dummy();

        // Calculate address we'd deploy to if we CREATE2 a dummy
        bytes32 initHash = keccak256(type(Dummy).creationCode);
        address child = calculateChildCreate2(SALT, initHash, address(this));

        // selfdestruct, prefunding dummy with funds
        this.blowUp(child);

        Dummy d2 = new Dummy{ salt: SALT }();
        return (++value, address(d1), address(d2), child);
    }

    function getValue() public view returns (uint) {
        return value;
    }

    function incrementValue() public returns (uint newValue) {
        return ++value;
    }

    // Calculate the address for the child n should deploy
    function calculateChildCreate2(bytes32 salt, bytes32 bytecodeHash, address deployer) internal pure returns (address addr) {
        return address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xFF), deployer, salt, bytecodeHash)))));
    }
}