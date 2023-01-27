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


  Scenario: Chain of SELFDESTRUCT on unwind, sending funds to caller
    Given 1 random account
    When the value sent to the contract is 100 atto
    And account 1 creates 5 SelfDestructChain contracts
    # NOTE: The balances will check out because the `gas_fee_cap` on the `Message` is set to 0,
    # which results in no charge for gas to the account, everything goes into miner penalty.
    Then the balance of account 1 is 9500 atto
    And the balance of contract 1 is 100 atto
    When account 1 calls destroy on contract 1 with addresses:
      | contract   |
      | contract 2 |
      | contract 3 |
      | contract 4 |
      | contract 5 |
    Then the balance of account 1 is 10000 atto
    And the balance of contract 1 is 0 atto


  @wip
  Scenario: SELFDESTRUCT + CREATE2. If possible, test this scenario: https://0age.medium.com/the-promise-and-the-peril-of-metamorphic-contracts-9eb8b8413c5e
    Given 1 random account
    When the code of transient contract Metamorphic / TransientContract is loaded
    And account 1 creates a Metamorphic / MetamorphicContractFactory contract
    And the value sent to the contract is 100 atto
    And account 1 calls deployMetamorphicContractWithConstructor on contract 1 with the code of Cocoon
    Then contract 2 describes itself as "Cocoon"
# And the balance of contract 2 is 100 atto
# When contract 2 is told to self destruct
# And account 1 calls deployMetamorphicContractWithConstructor on contract 1 with the code of Butterfly
# And contract 2 describes itself as "Butterfly"
# # Cocoon is expected to have refunded to its own address, which carries over to the new incarnation.
# And the balance of contract 2 is 100 atto
