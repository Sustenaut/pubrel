Feature: A release is cut from main as a pull request
  `pubrel prepare` moves the unreleased changes into the package publet,
  publishes it, bumps the manifest, writes the changelog, and opens a
  release PR. Nothing reaches main except through that PR.

  Scenario Outline: Each manifest kind is bumped
    Given a <kind> package released at version "0.1.0"
    And the unreleased changes "added" pushed to main
    When I run pubrel prepare
    Then it succeeds
    And the manifest states "0.1.1"
    And the published release record states "0.1.1" with "added"
    And the unreleased list is empty
    And CHANGELOG.md has a section "## 0.1.1"
    And a pull request "Release v0.1.1" was requested from "release/v0.1.1"

    Examples:
      | kind            |
      | cargo           |
      | cargo-workspace |
      | pyproject       |
      | npm             |

  Scenario: A workspace's internal dependencies move with it
    Given a cargo-workspace package released at version "0.1.0"
    And the unreleased changes "fixed" pushed to main
    When I run pubrel prepare
    Then the manifest pins its internal dependency at "0.1.1"

  Scenario: An npm package's lockfile moves with it
    Given a npm package released at version "0.1.0"
    And a package-lock.json at version "0.1.0"
    And the unreleased changes "fixed" pushed to main
    When I run pubrel prepare
    Then it succeeds
    And package-lock.json states "0.1.1" for itself and its root package
    And package-lock.json keeps its dependencies' versions

  Scenario: A dirty tree is refused
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "added" pushed to main
    And an uncommitted edit
    When I run pubrel prepare
    Then it fails
    And stderr mentions "not clean"
