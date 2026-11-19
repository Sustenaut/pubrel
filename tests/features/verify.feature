Feature: A release is checked against the key the repository names
  `release.json` names the key a release must be signed by. check and tag
  read the package publet and verify its signatures themselves, so CI
  needs no `pub`; only cutting a release, which signs, does.

  Background:
    Given a scratch directory
    And the program under test is "pubrel"
    And a cargo package released at version "0.1.0"

  Scenario: A signed release passes check without pub
    When a branch records "added" "a feature" and is released
    And pub is not available
    And I run "pubrel check origin/main"
    Then it succeeds
    And it prints "release 0.1.1"

  Scenario: A signed release is tagged without pub
    When pub is not available
    And I run "pubrel tag"
    Then it succeeds
    And the tag "v0.1.0" exists and names the package publet

  Scenario: A release with no signature by the named key fails check
    When a branch records "added" "a feature" and is released
    And the release's signatures are removed
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "has no valid signature by"

  Scenario: A release with no signature by the named key is not tagged
    When the release's signatures are removed
    And I run "pubrel tag"
    Then it fails
    And stderr mentions "has no valid signature by"
    And no tag exists

  Scenario: A key that is not a key fails
    When a branch records "added" "a feature" and is released
    And release.json names the identity claim as its key
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "is not an ed25519 key"

  Scenario: A release record altered after publishing fails
    When a branch records "added" "a feature" and is released
    And the published identity claim is altered
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "identifier mismatch"

  Scenario: Without a named key, the release passes with a warning
    When a branch records "added" "a feature" and is released
    And release.json names no key
    And I run "pubrel check origin/main"
    Then it succeeds
    And stderr mentions "names no `key`"
