Feature: RecursiveCall

  Scenario: Single DELEGATECALL modifies caller state
    Given 1 random account
    When account 1 creates 2 RecursiveCall contracts
    And account 1 calls recurse on contract 1 with max depth 1 and contract addresses:
      | action       | address    |
      | DELEGATECALL | contract 2 |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender                                     |
      | contract 1 | 1     | account 1                                  |
      | contract 2 | 0     | 0x0000000000000000000000000000000000000000 |


  Scenario: Single CALL modifies callee state
    Given 1 random account
    When account 1 creates 2 RecursiveCall contracts
    And account 1 calls recurse on contract 1 with max depth 1 and contract addresses:
      | action | address    |
      | CALL   | contract 2 |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender     |
      | contract 1 | 0     | account 1  |
      | contract 2 | 1     | contract 1 |


  Scenario: Multiple DELEGATECALL modifies caller state
    Given 1 random account
    When account 1 creates 4 RecursiveCall contracts
    And account 1 calls recurse on contract 4 with max depth 5 and contract addresses:
      | action       | address    |
      | DELEGATECALL | contract 3 |
      | DELEGATECALL | contract 2 |
      | DELEGATECALL | contract 1 |
      | DELEGATECALL | contract 2 |
      | DELEGATECALL | contract 3 |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender    |
      | contract 1 | 0     |           |
      | contract 2 | 0     |           |
      | contract 3 | 0     |           |
      | contract 4 | 5     | account 1 |


  Scenario: Multiple CALL modifies callee state
    Given 1 random account
    When account 1 creates 4 RecursiveCall contracts
    And account 1 calls recurse on contract 4 with max depth 5 and contract addresses:
      | action | address    |
      | CALL   | contract 3 |
      | CALL   | contract 2 |
      | CALL   | contract 1 |
      | CALL   | contract 2 |
      | CALL   | contract 3 |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender     |
      | contract 1 | 3     | contract 2 |
      | contract 2 | 4     | contract 1 |
      | contract 3 | 5     | contract 2 |
      | contract 4 | 0     | account 1  |


  Scenario: Mixed CALL/DELEGATECALL
    Given 1 random account
    When account 1 creates 5 RecursiveCall contracts
    And account 1 calls recurse on contract 1 with max depth 4 and contract addresses:
      | action       | address    |
      | DELEGATECALL | contract 2 |
      | CALL         | contract 3 |
      | DELEGATECALL | contract 4 |
      | CALL         | contract 5 |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender     |
      | contract 1 | 1     | account 1  |
      | contract 2 | 0     |            |
      | contract 3 | 3     | contract 1 |
      | contract 4 | 0     |            |
      | contract 5 | 4     | contract 3 |


  Scenario: Deep self-recursion with DELEGATECALL
    Given 1 random account
    When account 1 creates 2 RecursiveCall contracts
    And account 1 calls recurse on contract 1 with max depth 100 and contract addresses:
      | action       | address    |
      | DELEGATECALL | contract 2 |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender    |
      | contract 1 | 100   | account 1 |
      | contract 2 | 0     |           |


  Scenario: Deep self-recursion with CALL
    Given 1 random account
    When account 1 creates 1 RecursiveCall contracts
    And account 1 calls recurse on contract 1 with max depth 100 and contract addresses:
      | action | address    |
      | CALL   | contract 1 |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender     |
      | contract 1 | 100   | contract 1 |


  Scenario: REVERT only reverts the last step of the recursion.
    Given 1 random account
    When account 1 creates 3 RecursiveCall contracts
    And account 1 calls recurse on contract 1 with max depth 3 and contract addresses:
      | action       | address    |
      | DELEGATECALL | contract 2 |
      | CALL         | contract 3 |
      | CALL         | contract 1 |
      | REVERT       |            |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender     |
      | contract 1 | 1     | account 1  |
      | contract 2 | 0     |            |
      | contract 3 | 2     | contract 1 |
