Feature: A crate's public API changes no more than its recorded changes say
  A cargo package whose release.json entry sets "semver-checks" has its
  public API compared with its last release's tag by cargo-semver-checks.
  The recorded changes set how far it may move: `changed` or `removed`
  allow a break, `added` allows the API to grow, and anything else allows
  no change. The level is what the change is, not the version it makes:
  below 1.0 a break bumps the minor version, and is still a break.

  The cargo-semver-checks here is a stand-in that reports a break or an
  addition when told to, judged by the release type it is asked about, as
  the real tool judges it.

  Background:
    Given a scratch directory
    And the program under test is "pubrel"

  Scenario: A fix that leaves the API alone passes
    Given a cargo crate released and tagged at "0.1.0", whose API is checked
    When a branch changes "src/lib.rs" and records "fixed" "an overflow"
    And I run "pubrel check origin/main"
    Then it succeeds
    And it prints "API: changes no more than a patch release allows, against v0.1.0"
    And "cargo-semver-checks" was run with "semver-checks check-release --package demo --baseline-rev v0.1.0 --release-type patch"

  Scenario: An unrecorded break fails, saying how to record it
    Given a cargo crate released and tagged at "0.1.0", whose API is checked
    And the API breaks
    When a branch changes "src/lib.rs" and records "added" "a new function"
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "demo's public API changed more than its recorded changes allow (recorded: added, so a minor release) since v0.1.0"
    And stderr mentions "function_missing"
    And stderr mentions "pubrel add removed"

  Scenario: A recorded break passes, and below 1.0 bumps the minor version
    Given a cargo crate released and tagged at "0.1.0", whose API is checked
    And the API breaks
    When a branch changes "src/lib.rs" and records "removed" "the old function"
    And I run "pubrel check origin/main"
    Then it succeeds
    And "cargo-semver-checks" was run with "semver-checks check-release --package demo --baseline-rev v0.1.0 --release-type major"
    When I run "pubrel next"
    Then it succeeds
    And it prints:
      """
      0.2.0
      """

  Scenario: An unrecorded addition fails a patch
    Given a cargo crate released and tagged at "0.1.0", whose API is checked
    And the API grows
    When a branch changes "src/lib.rs" and records "fixed" "an overflow"
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "(recorded: fixed, so a patch release)"
    And stderr mentions "pubrel add added"

  Scenario: prepare refuses an unrecorded break before anything changes
    Given a cargo crate released and tagged at "0.1.0", whose API is checked
    And the unreleased changes "fixed" pushed to main
    And the API breaks
    When I run "pubrel prepare"
    Then it fails
    And stderr mentions "changed more than its recorded changes allow"
    And the manifest states "0.1.0"
    And the unreleased list holds "fixed" "a fixed change"

  Scenario: A release pull request is checked against the release before it
    Given a cargo crate released and tagged at "0.1.0", whose API is checked
    When a branch records "added" "a new function" and is released
    And I run "pubrel check origin/main"
    Then it succeeds
    And it prints "release 0.1.1"
    And the API of "demo" was last checked against "v0.1.0" as a "minor" release

  Scenario: Without the last release's tag, the check says how to get it
    Given a cargo crate released and tagged at "0.1.0", whose API is checked
    When the tag "v0.1.0" is deleted
    And a branch changes "src/lib.rs" and records "fixed" "an overflow"
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "the tag v0.1.0 does not exist here; fetch tags"

  Scenario: A package never released has nothing to check against
    Given a released-never cargo package at version "0.1.0"
    And release.json checks the crate's API
    When a branch changes "src/lib.rs" and records "added" "the first release"
    And I run "pubrel check origin/main"
    Then it succeeds
    And it prints "API: not checked, as there is no earlier release to check it against"

  Scenario: Only a crate has an API to check
    Given a released-never pyproject package at version "0.1.0"
    And release.json checks the crate's API
    When I run "pubrel next"
    Then it fails
    And stderr mentions "`semver-checks` checks a crate's public API, so it needs a cargo-package manifest"
