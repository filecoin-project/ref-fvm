Feature: RecursiveCall

  @wip
  Scenario: Single DELEGATECALL modifies delegator state
    Given 1 random account
    When account 1 creates a RecursiveCallInner contract
    And account 1 creates a RecursiveCallOuter contract
    # XXX: This fails, I don't know why, when the delegate call is made in Solidity.
    And account 1 calls recurse on contract 2 with max depth 1 and contracts
      | contracts  |
      | contract 1 |
