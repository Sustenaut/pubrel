Feature: Each change is a file of its own
  `pubrel add` writes each change to its own file in the package's
  `unreleased/` directory, named by when it was recorded and what it says,
  so pull requests that each record a change never touch the same file and
  never conflict. `prepare` moves the files' changes, in name order, into
  the release, and deletes them: a release pull request only deletes
  them, so it conflicts with none either. The single `unreleased.json`
  pubrel 0.5 kept is still read, and the next release removes it.

  Background:
    Given a cargo package released at version "0.1.0"

  Scenario: Two branches that each record a change merge without conflict
    Given a branch "one" that records "fixed" "an overflow"
    And a branch "two" that records "added" "a flag"
    When "one" and "two" are merged into main
    Then the unreleased directory holds 2 change files
    And the unreleased list holds "fixed" "an overflow"
    And the unreleased list holds "added" "a flag"

  Scenario: The same change recorded twice, even in one second, is two files
    When I run pubrel add "fixed" "a typo"
    And I run pubrel add "fixed" "a typo"
    Then it succeeds
    And the unreleased directory holds 2 change files

  Scenario: prepare releases the changes in name order and deletes their files
    Given the change file "20260102T000000Z-later.json" records "fixed" "the later fix"
    And the change file "20260101T000000Z-earlier.json" records "added" "the earlier feature"
    And the summary "Two changes."
    And they are pushed to main
    When I run pubrel prepare
    Then it succeeds
    And the published release record lists "the earlier feature" before "the later fix"
    And CHANGELOG.md has "the later fix" under "### Fixed"
    And the release deletes every unreleased file and adds none
    And the unreleased list is empty
    And the unreleased list has no summary

  Scenario: A malformed change file is refused, by name
    Given the change file "20260101T000000Z-bad.json" records "improved" "something"
    When I run pubrel next
    Then it fails
    And stderr mentions "20260101T000000Z-bad.json"

  Scenario: A branch that only rewords a recorded change records nothing new
    Given the change file "20260101T000000Z-a-fix.json" records "fixed" "a fix"
    And they are pushed to main
    When a branch changes "src/lib.rs" and rewords the change file "20260101T000000Z-a-fix.json"
    And I run pubrel check against main
    Then it fails
    And stderr mentions "records no change in corpus/publets/pkg.demo/unreleased/"

  Scenario: A release missing a change main gained fails check
    When a branch records "fixed" "an overflow" and is released
    And the release branch also records "fixed" "a late fix"
    And I run pubrel check against main
    Then it fails
    And stderr mentions "prepare again from the current main"

  Scenario: The pubrel 0.5 unreleased list is still read, and the release removes it
    Given a pubrel 0.5 unreleased list holding "fixed" "an old fix" and the summary "From before."
    And the change file "20260101T000000Z-new.json" records "added" "a new feature"
    And they are pushed to main
    When I run pubrel prepare
    Then it succeeds
    And the published release record lists "an old fix" before "a new feature"
    And the published summary is "demo 0.1.1 in brief: From before."
    And the pubrel 0.5 unreleased list is gone
    And the release deletes every unreleased file and adds none

  Scenario: A branch that appends to the pubrel 0.5 unreleased list records a change
    When a branch appends "fixed" "an old-style record" to the pubrel 0.5 unreleased list
    And I run pubrel check against main
    Then it succeeds
    And it prints "1 change(s) recorded"
