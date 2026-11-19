Feature: A release may open with a summary
  `pubrel summary` gives the next release a paragraph that says what it
  is about, in prose, above its list of changes. `prepare` publishes it
  as a `summary` claim in the package publet, signed with the release,
  and the changelog and the GitHub release show it first. A release
  needs no summary.

  Background:
    Given a scratch directory
    And the program under test is "pubrel"
    And a cargo package released at version "0.1.0"

  Scenario: The summary opens the release's changelog section
    Given the summary "Checks run faster, and a crash on empty input is gone."
    And the unreleased changes "fixed" pushed to main
    When I run "pubrel prepare"
    Then it succeeds
    And CHANGELOG.md has "Checks run faster, and a crash on empty input is gone." before "### Fixed"
    And the pull request's notes begin the release with "Checks run faster, and a crash on empty input is gone."
    And the published summary is "demo 0.1.1 in brief: Checks run faster, and a crash on empty input is gone."
    And the file "corpus/publets/pkg.demo/unreleased/summary.md" does not exist

  Scenario: Recording a change keeps the summary
    Given the summary "A smaller release."
    When I run "pubrel add fixed 'an overflow'"
    And I run "pubrel summary"
    Then it succeeds
    And it prints "A smaller release."

  Scenario: With no summary set, pubrel summary says how to set one
    When I run "pubrel summary"
    Then it succeeds
    And it prints "no summary yet: pubrel summary TEXT..."

  Scenario: A release without a summary has none in its changelog
    Given the unreleased changes "fixed" pushed to main
    When I run "pubrel prepare"
    Then it succeeds
    And the file "corpus/corpus.lock" does not include "pkg.demo#summary"

  Scenario: A release whose summary lost its signature fails check
    Given the summary "A smaller release."
    When a branch records "fixed" "an overflow" and is released
    And the summary's signatures are removed
    And I run "pubrel check origin/main"
    Then it fails
    And stderr mentions "has no valid signature by"
