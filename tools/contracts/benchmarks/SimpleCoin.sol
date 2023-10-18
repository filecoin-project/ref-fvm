// SPDX-License-Identifier: MIT
pragma solidity >=0.4.2;

contract SimpleCoin {
    mapping(address => uint256) balances;

    event Transfer(address indexed _from, address indexed _to, uint256 _value);

    error lessThanFive(string err, uint256 code);

    constructor() {
        balances[tx.origin] = 10000;
    }

    function sendCoin(
        address receiver,
        uint256 amount
    ) public returns (bool sufficient) {
        if (balances[msg.sender] < amount) return false;
        balances[msg.sender] -= amount;
        balances[receiver] += amount;
        emit Transfer(msg.sender, receiver, amount);
        return true;
    }

    function greaterThanTen(uint256 num) public pure returns (bool) {
        assert(num > 1);
        if (num < 5) revert lessThanFive("Less than five", 2);
        require(num > 10, "Less Than ten");
        return true;
    }

    function getBalanceInEth(address addr) public view returns (uint256) {
        return getBalance(addr) * 2;
    }

    function getBalance(address addr) public view returns (uint256) {
        return balances[addr];
    }
}
