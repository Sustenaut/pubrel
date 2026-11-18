Feature: A prepare that does not finish leaves nothing behind, and one prepare runs at a time
  `prepare` changes the tree before it commits: the package publet, the
  version, the lock, the unreleased changes it folds in, and what pub
  exports. A step that fails before the commit undoes all of it, and
  `pubrel withdraw --unfinished` undoes a prepare that was killed. Records
  of an unfinished attempt at the same commit, which pub's store keeps,
  are listed in `.draft-discards` so the release carries only its own. A
  prepare holds a lock in the clone's git directory while it runs.

  Background:
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "fixed" pushed to main

  Scenario: A prepare whose commit fails undoes itself, and the next prepare cuts the release
    Given git refuses commits
    When I run pubrel prepare
    Then it fails
    And stderr mentions "undid the unfinished prepare"
    And the tree is clean, on main, with no release branch for 0.1.1
    When git takes commits again
    And the release is cut again
    Then CHANGELOG.md has one section for "0.1.1"

  Scenario: Records an unfinished attempt left from the same commit are discarded, and the release carries only its own
    Given the date is "2026-01-01"
    And git refuses commits
    And a prepare has failed and undone itself
    And the date is "2026-01-02"
    And git takes commits again
    When I run pubrel prepare
    Then it fails
    And stderr mentions "record(s) of an unfinished attempt at this release, from this commit"
    And stderr mentions "prepare again"
    And the tree is clean, on main, with no release branch for 0.1.1
    When the release is cut again
    Then CHANGELOG.md has one section for "0.1.1"
    And CHANGELOG.md mentions "2026-01-02" and not "2026-01-01"

  Scenario: withdraw --unfinished undoes a prepare that was killed on its release branch
    Given a prepare was killed on its release branch after changing the version and exporting an object
    When I run pubrel withdraw "--unfinished"
    Then it succeeds
    And it prints "undid the unfinished prepare"
    And the tree is clean, on main, with no release branch for 0.1.1
    When the release is cut again
    Then CHANGELOG.md has one section for "0.1.1"

  Scenario: A second prepare, or a withdraw of an unfinished one, waits for the prepare that runs
    Given a prepare is running here
    When I run pubrel prepare
    Then it fails
    And stderr mentions "a prepare is running here (pid"
    When I run pubrel withdraw "--unfinished"
    Then it fails
    And stderr mentions "a prepare is running here (pid"

  Scenario: The lock of a prepare that ended does not stop the next
    Given a prepare that ended left its lock
    When the release is cut again
    Then CHANGELOG.md has one section for "0.1.1"
