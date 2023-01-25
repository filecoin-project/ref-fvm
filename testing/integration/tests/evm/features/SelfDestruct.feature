@wip
Feature: SelfDestruct

  # Some of these are impossible to test today because we don’t dispatch to smart contract logic on send.
  # https://github.com/filecoin-project/ref-fvm/issues/835
  # To simulate these, we will explicity call a method on the contract to trigger the call back to the self-destructed contract.

  Scenario: SELFDESTRUCT on contract creation, sending funds to self => fails
    Given 1 random account
    When the beneficiary is self
    And account 1 tries to create a SelfDestructOnCreate contract
    Then the execution fails with message 'Huh?'


  Scenario: SELFDESTRUCT on contract creation, sending funds to an f410 EthAccount that doesn’t exist => succeeds
    Given 1 random account
    And a non-existing f410 account 0x76c499be8821b5b9860144d292fff728611bfd1a
    When the beneficiary is 0x76c499be8821b5b9860144d292fff728611bfd1a
    And account 1 creates a SelfDestructOnCreate contract
    Then the f410 account 0x76c499be8821b5b9860144d292fff728611bfd1a exists


  @dispatch_on_send
  Scenario: SELFDESTRUCT on contract creation, sending funds to a smart contract that ends up returning the funds to the sender => in Eth those funds would vanish, in Filecoin they are preserved.

  @dispatch_on_send
  Scenario: Chain of SELFDESTRUCTs

  @dispatch_on_send
  Scenario: Reentrant chain of SELFDESTRUCTs. Beneficiary calls a method in selfdestructed and causes it to selfdestruct again, over and over again.

  @dispatch_on_send
  Scenario: SELFDESTRUCTS + CREATE. Beneficiary creates a new smart contract. Beneficiary ends up calling selfdestructed contract on a method that creates a new smart contract (yikes)

  @dispatch_on_send
  Scenario: SELFDESTRUCTS + CREATE2. If possible, test this scenario: https://0age.medium.com/the-promise-and-the-peril-of-metamorphic-contracts-9eb8b8413c5e

  @dispatch_on_send
  Scenario: SELFDESTRUCT + EXTCODE* methods
