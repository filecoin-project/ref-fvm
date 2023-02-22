// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

// No frills dummy used to test things on fresh-deployed contracts
contract Dummy {

    uint value = 100;

    event Value(uint indexed v);

    function receiveValue() public payable returns (uint) {
        return msg.value;
    }

    function logValue() public returns (uint) {
        emit Value(value);
        return value;
    }

    function incrementValue() public returns (uint) {
        return ++value;
    }

    function getValueView() public view returns (uint) {
        return value;
    }

    function getValuePure() public pure returns (uint) {
        return 42;
    }
}

library DummyLib {

    // Convenience method that deploys Dummy and returns it
    // as an address    
    function newDummy() internal returns (address) {
        Dummy d = new Dummy();
        return address(d);
    }
}