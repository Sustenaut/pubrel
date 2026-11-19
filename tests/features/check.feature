Feature: CI holds every change to the rules
  A pull request that changes code records what it changes; one that
  releases publishes exactly the version its changes require.

  Background:
    Given a scratch directory
    And the program under test is "pubrel"
    And a cargo package released at version "0.1.0"

  Scenario: A code change without a recorded change fails
    When a branch changes "src/lib.rs"
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "records no change"

  Scenario: A code change with a recorded change passes
    When a branch changes "src/lib.rs" and records "fixed" "an off-by-one"
    And I run "pubrel check origin/main"
    Then it succeeds
    And it prints "1 change(s) recorded"

  Scenario: A change outside the code paths needs no record
    When a branch changes "README.md"
    And I run "pubrel check origin/main"
    Then it succeeds
    And it prints "no code changed"

  Scenario: A release with the bump its changes require passes
    When a branch records "added" "a feature" and is released
    And I run "pubrel check origin/main"
    Then it succeeds
    And it prints "release 0.1.1"

  Scenario: A release whose manifest disagrees fails
    When a branch records "added" "a feature" and is released
    And the branch's manifest is set to "0.3.0"
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "states 0.3.0"

  Scenario: An internal change satisfies check
    When a branch changes "src/lib.rs" and records "internal" "move a module"
    And I run "pubrel check origin/main"
    Then it succeeds
    And it prints "1 change(s) recorded"
