@wip
Feature: SelfDestruct

  Scenario: SELFDESTRUCT on contract creation, sending funds to self => fails
    Given 1 random account
    When the beneficiary is self
    And account 1 tries to create a SelfDestructOnCreate contract
    Then the execution fails with message '???'


  Scenario: SELFDESTRUCT on contract creation, sending funds to an f410 EthAccount that doesn’t exist => succeeds
    Given 1 random account
    And a non-existing f410 account 0x76c499be8821b5b9860144d292fff728611bfd1a
    When the beneficiary is 0x76c499be8821b5b9860144d292fff728611bfd1a
    And account 1 creates a SelfDestructOnCreate contract
    Then f410 account 0x76c499be8821b5b9860144d292fff728611bfd1a exists

  Scenario: SELFDESTRUCT on contract creation, sending funds to an f410 EthAccount that doesn’t exist => funds received
    Given 1 random account
    When the beneficiary is 0x76c499be8821b5b9860144d292fff728611bfd1a
    And the value sent to the contract is 100 atto
    And account 1 creates a SelfDestructOnCreate contract
    Then the balance of f410 account 0x76c499be8821b5b9860144d292fff728611bfd1a is 100 atto


  Scenario: SELFDESTRUCTS + CREATE2. If possible, test this scenario: https://0age.medium.com/the-promise-and-the-peril-of-metamorphic-contracts-9eb8b8413c5e

  Scenario: Chain of SELFDESTRUCT on unwind, sending funds to caller
