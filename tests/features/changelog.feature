Feature: The changelog is a view of the package publet
  CHANGELOG.md is generated from the published release records, newest
  first, and never edited by hand.

  Background:
    Given a scratch directory
    And the program under test is "pubrel"

  Scenario: Releases appear newest first
    Given a cargo package released at version "0.1.0"
    And it is released again with "added" "a later feature"
    When I run "pubrel changelog"
    Then it succeeds
    And CHANGELOG.md lists "0.1.1" before "0.1.0"
    And CHANGELOG.md has "a later feature" under "### Added"
