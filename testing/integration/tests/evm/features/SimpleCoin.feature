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
      And account 2 tries to create a SimpleCoin contract
      Then the execution fails with message 'Actor sequence invalid: 0 != 1'

  Rule: Nonce increases

    Scenario: Sending multiple messages to views, the tester and the state stay in sync
      Given 1 random account
      When account 1 creates a SimpleCoin contract
      Then the balance of account 1 is 10000 coins
      And the balance of account 1 is 10000 coins
      And the balance of account 1 is 10000 coins
      And the seqno of account 1 is 4

    Scenario: Deploying with the wrong nonce fails
      Given 1 random account
      When the seqno of account 1 is set to 2
      And account 1 tries to create a SimpleCoin contract
      Then the execution fails with message 'Actor sequence invalid: 2 != 0'

  Rule: Sending coins

    Scenario: When the sender has sufficient balance, the receiver is credited and the sender is credited
      Given 3 random accounts
      When account 1 creates a SimpleCoin contract
      And account 1 sends account 2 4000 coins
      And account 1 sends account 3 1000 coins
      And account 2 sends account 3 2000 coins
      Then the balance of account 1 is 5000 coins
      Then the balance of account 2 is 2000 coins
      Then the balance of account 3 is 3000 coins

    Scenario: Doesn't have enough to send
      Given 2 random accounts
      When account 1 creates a SimpleCoin contract
      And account 1 sends account 2 11000 coins
      Then the balance of account 1 is 10000 coins
      And the balance of account 2 is 0 coins

    Scenario: When coins are sent, an event is emitted
      Given 2 random accounts
      When account 1 creates a SimpleCoin contract
      And account 1 sends account 2 4000 coins
      Then a Transfer event of 4000 coins from account 1 to account 2 is emitted
