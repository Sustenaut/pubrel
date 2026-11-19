Feature: A release is cut from main as a pull request
  `pubrel prepare` moves the unreleased changes into the package publet,
  publishes it, bumps the manifest, writes the changelog, and opens a
  release PR. Nothing reaches main except through that PR.

  Background:
    Given a scratch directory
    And the program under test is "pubrel"

  Scenario Outline: Each manifest kind is bumped
    Given a <kind> package released at version "0.1.0"
    And the unreleased changes "added" pushed to main
    When I run "pubrel prepare"
    Then it succeeds
    And the manifest states "0.1.1"
    And the published release record states "0.1.1" with "added"
    And the unreleased list is empty
    And the file "CHANGELOG.md" includes "## 0.1.1"
    And "gh" was run with arguments matching "pr create --base main --title Release v0.1.1 --body *"
    And the current branch is "release/v0.1.1"
    And the remote "origin" has the branch "release/v0.1.1"

    Examples:
      | kind            |
      | cargo           |
      | cargo-workspace |
      | pyproject       |
      | npm             |

  Scenario: A workspace's internal dependencies move with it
    Given a cargo-workspace package released at version "0.1.0"
    And the unreleased changes "fixed" pushed to main
    When I run "pubrel prepare"
    Then the file "Cargo.toml" includes "core = { path = "crates/core", version = "0.1.1" }"

  Scenario: An npm package's lockfile moves with it
    Given a npm package released at version "0.1.0"
    And a package-lock.json at version "0.1.0"
    And the unreleased changes "fixed" pushed to main
    When I run "pubrel prepare"
    Then it succeeds
    And the JSON in "package-lock.json" has "version" equal to "0.1.1"
    And the JSON in "package-lock.json" has "packages..version" equal to "0.1.1"
    And the JSON in "package-lock.json" has "packages.node_modules/dep.version" equal to "0.1.0"

  Scenario: An edit in the checkout is neither released nor touched
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "added" pushed to main
    And a file "src/lib.rs" holding "// edited"
    And nothing is checked out here after a prepare
    When I run "pubrel prepare --no-pr"
    Then it succeeds
    And the file "src/lib.rs" holds "// edited"
    And the current branch is "main"
    And the release branch it cut has "src/lib.rs" holding "// demo"
