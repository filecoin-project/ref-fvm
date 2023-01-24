@wip
Feature: RecursiveCall

  Scenario: Single DELEGATECALL modifies delegator state
    Given 1 random account
    When account 1 creates a RecursiveCallInner contract
    And account 1 creates a RecursiveCallOuter contract
    And account 1 calls recurse on contract 2 with max depth 1 and contract addresses:
      | addresses  |
      | contract 1 |
    Then the state of depth and sender of the contracts are
      | contract   | depth | sender    |
      | contract 1 | 0     |           |
      | contract 2 | 1     | account 1 |

  Scenario: Multiple DELEGATECALL
    Given 1 random account
    When account 1 creates a RecursiveCallInner contract
    And account 1 creates a RecursiveCallInner contract
    And account 1 creates a RecursiveCallInner contract
    And account 1 creates a RecursiveCallOuter contract
    And account 1 calls recurse on contract 4 with max depth 5 and contract addresses:
      | addresses  |
      | contract 3 |
      | contract 2 |
      | contract 1 |
      | contract 2 |
      | contract 3 |
    Then the state of depth and sender of the contracts are:
      | contract   | depth | sender    |
      | contract 1 | 0     |           |
      | contract 2 | 0     |           |
      | contract 3 | 0     |           |
      | contract 4 | 5     | account 1 |
