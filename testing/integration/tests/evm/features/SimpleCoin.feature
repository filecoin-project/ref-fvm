Feature: SimpleCoin

  Rule: Owner has initial balance

    Scenario: When we deploy the contract, the owner gets 10000 coins
      Given 1 random account
      When account 1 creates a SimpleCoin contract
      Then the balance of account 1 is 10000 coins
