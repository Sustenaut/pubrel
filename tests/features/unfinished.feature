Feature: A prepare claims its release, works in a worktree of its own, and leaves nothing behind
  `prepare` first claims its release branch on origin, which fails if it
  is there, so a second prepare from any clone stops. It then cuts the
  release in a worktree under `.git/pubrel/`, from origin/main, sharing
  this checkout's pub store and key by links, never copies, so this
  checkout is never edited. A prepare that fails removes its worktree,
  its branch and its claim; `pubrel withdraw --unfinished` clears one
  that was killed. Records of an unfinished attempt at the same commit,
  which pub's store keeps, are listed in `.draft-discards`, so the
  release carries only its own.

  Background:
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "fixed" pushed to main

  Scenario: A prepare whose commit fails undoes itself, and the next prepare cuts the release
    Given git refuses commits
    When I run pubrel prepare
    Then it fails
    And stderr mentions "undid the unfinished prepare"
    And this checkout is clean, on main, with no release branch for 0.1.1
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
    And stderr mentions "with their signatures, of an unfinished attempt at this release, from this commit"
    And .draft-discards lists the unfinished attempt's records and a signature of one
    And stderr mentions "prepare again"
    And this checkout is clean, on main, with no release branch for 0.1.1
    When the release is cut again
    Then CHANGELOG.md has one section for "0.1.1"
    And CHANGELOG.md mentions "2026-01-02" and not "2026-01-01"

  Scenario: A prepare leaves this checkout, its signing key and its store as they were, whether it succeeds or fails
    Given this checkout, its signing key and its store are hashed
    And git refuses commits
    When pubrel prepare runs, and nothing here is checked out after it
    Then it fails
    And this checkout, its signing key and its store are as they were hashed
    And no worktree of a prepare is left
    When git takes commits again
    And pubrel prepare runs, and nothing here is checked out after it
    Then it succeeds
    And this checkout, its signing key and its store are as they were hashed
    And no worktree of a prepare is left

  Scenario: A second prepare, from another clone, stops while the first's claim is on origin
    Given another clone has claimed the release on origin
    When I run pubrel prepare
    Then it fails
    And stderr mentions "release/v0.1.1 is being prepared, or was cut"
    And stderr mentions "withdraw it first"

  Scenario: withdraw --unfinished clears a prepare that was killed: its worktree, its branch and its claim
    Given a prepare was killed after claiming the release and making its worktree
    When I run pubrel withdraw "--unfinished"
    Then it succeeds
    And it prints "undid the unfinished prepare"
    And no worktree of a prepare is left
    And origin has no branch "release/v0.1.1"
    When I run pubrel prepare
    Then it succeeds

  Scenario: withdraw --unfinished leaves a branch that holds a release commit
    Given a release is cut and pushed as "release/v0.1.1" without its pull request
    When I run pubrel withdraw "--unfinished"
    Then it fails
    And stderr mentions "nothing to undo"
    When I run pubrel withdraw "--unfinished release/v0.1.1"
    Then it fails
    And stderr mentions "holds a release commit"
    And origin has the branch "release/v0.1.1"

  Scenario: The release is planned from main, not from this checkout
    Given main has a change this checkout has not pulled
    When the release is cut again
    Then CHANGELOG.md mentions "a fix this checkout never saw" and not "2026-01-01"

  Scenario: Two --no-pr prepares in a row keep the first's release branch
    When I run pubrel prepare --no-pr twice, back on main between them
    Then it fails
    And stderr mentions "release/v0.1.1 exists here, holding a release commit: push it or delete it"
    And the first prepare's release branch is kept, with its release commit

  Scenario: withdraw --unfinished takes the branch to clear
    Given a prepare was killed after claiming the release and making its worktree
    When I run pubrel withdraw "--unfinished release/v0.1.1"
    Then it succeeds
    And origin has no branch "release/v0.1.1"
    And no worktree of a prepare is left

  Scenario: A claim that cannot be pushed says git's own reason
    Given origin cannot be reached
    When I run pubrel prepare
    Then it fails
    And stderr does not mention "is being prepared"

  Scenario: A claim another clone holds survives withdraw --unfinished with no branch
    Given another clone's prepare holds the claim on "release/v0.1.1"
    When I run pubrel withdraw "--unfinished"
    Then it fails
    And stderr mentions "nothing to undo"
    And stderr mentions "release/v0.1.1: Being prepared by pubrel 9.9.9 on elsewhere, pid 4242"
    And origin has the branch "release/v0.1.1"
