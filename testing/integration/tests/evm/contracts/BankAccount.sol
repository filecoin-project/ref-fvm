// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Account {
    // Address of the user the account belongs to.
    address public owner;
    // Address of the bank contract where the account was opened.
    address public bank;

    constructor(address _owner) payable {
        bank = msg.sender;
        owner = _owner;
    }
}

contract Bank {
    // Address of the user who created the bank.
    address public owner;
    // Accounts opened at the bank.
    Account[] public accounts;

    constructor() payable {
        owner = msg.sender;
    }

    // Open a new account and return its address.
    function openAccount() external payable returns (address) {
        Account account = new Account(msg.sender);
        accounts.push(account);
        return address(account);
    }
}
