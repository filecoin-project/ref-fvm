Feature: SimpleCoin

  Rule: Owner has initial balance

    Scenario: When we deploy the contract, the owner gets 10000 coins
      Given 1 random account
      When account 1 creates a SimpleCoin contract
      Then the balance of account 1 is 10000 coins

    @two_deploys
    Scenario: Two accounts deploy the same contract
      Given 2 random accounts
      When account 1 creates a SimpleCoin contract
      And account 2 creates a SimpleCoin contract
      # the suite queries the last deployed account
      Then the balance of account 1 is 0 coins
      And the balance of account 2 is 10000 coins
