@wip
Feature: BankAccount

  Scenario: Open bank account
    Given 2 random accounts
    When account 1 creates a Bank contract
    And account 2 opens a bank account
    Then the owner of the bank is account 1
    And the owner of the bank account is account 2
    And the bank of the bank account is set
