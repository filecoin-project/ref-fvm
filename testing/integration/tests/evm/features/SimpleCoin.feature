Feature: SimpleCoin

  Rule: Owner has initial balance

    Scenario: When we deploy the contract, the owner gets 10000 coins
      Given 1 random account
      When account 1 creates a SimpleCoin contract
      Then the balance of account 1 is 10000 coins

    Scenario: Two accounts deploy the same contract
      Given 2 random accounts
      When account 1 creates a SimpleCoin contract
      And account 2 creates a SimpleCoin contract
      # the suite queries the last deployed account
      Then the balance of account 1 is 0 coins
      And the balance of account 2 is 10000 coins

  Rule: Multiple deployments

    Scenario: An account can deploy the same contract twice
      Given 1 random account
      When account 1 creates a SimpleCoin contract
      And account 1 creates a SimpleCoin contract

    Scenario: Different accounts with the same public key
      Given accounts with private keys
        | private keys                                                     |
        | 5e969c4ac2f287128d6fd71e7d111dbd19a5b2bea59da5d5d908044a514f5f8e |
        | 5e969c4ac2f287128d6fd71e7d111dbd19a5b2bea59da5d5d908044a514f5f8e |
      When account 1 creates a SimpleCoin contract
      Then account 2 fails to create a SimpleCoin contract with 'Actor sequence invalid: 0 != 1'
