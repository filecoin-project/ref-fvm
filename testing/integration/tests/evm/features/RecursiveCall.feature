Feature: RecursiveCall

  @wip
  Scenario: Single DELEGATECALL modifies delegator state
    Given 1 random account
    When account 1 creates a RecursiveCallInner contract
    And account 1 creates a RecursiveCallOuter contract
    And account 1 calls recurse on contract 2 with max depth 1 and contracts
      | contracts  |
      | contract 1 |
    Then the depths and senders of the contracts are
      | contracts  | depths | senders   |
      | contract 2 | 1      | account 1 |
      | contract 1 | 0      |           |
