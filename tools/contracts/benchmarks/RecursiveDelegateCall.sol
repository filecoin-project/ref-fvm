// SPDX-License-Identifier: MIT
pragma solidity >=0.8.17;

contract RecursiveDelegatecall {
    event RecursiveCallEvent(uint256 count, address self);
    // event RecursiveCallReturnEvent(bool success, bytes returnedData);
    uint public totalCalls;

    function recursiveCall(uint256 count) public returns (uint256) {
        emit RecursiveCallEvent(count, address(this));
	totalCalls += 1;
        if (count > 1) {
           count -= 1;
           (bool success, bytes memory returnedData) = address(this).delegatecall(
               abi.encodeWithSignature("recursiveCall(uint256)", count)
           );
        //    emit RecursiveCallReturnEvent(success, returnedData);
	   return count;
        }
	return count;
    }
}

