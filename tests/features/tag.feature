Feature: CI tags what was published, and only that
  On main, a manifest version the package publet records as released gets
  a tag naming the package publet, and a GitHub release.

  Background:
    Given a scratch directory
    And the program under test is "pubrel"

  Scenario: A published release is tagged and released
    Given a cargo package released at version "0.1.0"
    When I run "pubrel tag"
    Then it succeeds
    And the tag "v0.1.0" exists and names the package publet
    And "gh" was run with arguments matching "release create v0.1.0 *"

  Scenario: An existing tag is left alone
    Given a cargo package released at version "0.1.0"
    And the tag "v0.1.0" already exists
    When I run "pubrel tag"
    Then it succeeds
    And it prints "already exists"

  Scenario: An unpublished version is not tagged
    Given a released-never cargo package at version "0.1.0"
    When I run "pubrel tag"
    Then it succeeds
    And it prints "not a published release"
    And no tag exists

  Scenario: Release notes say how to install that release
    Given a cargo package released at version "0.1.0"
    And release.json says a release installs with "cargo install --locked demo --version {version} # from {tag}"
    And the unreleased changes "fixed" pushed to main
    When I run "pubrel prepare"
    Then it succeeds
    And "gh" was run with arguments matching "pr create * --body *cargo install --locked demo --version 0.1.1 # from v0.1.1*"

  Scenario: A release served through jsDelivr is warmed and its ranges purged
    Given a cargo package released at version "0.1.0"
    And release.json serves "app/app.js" through jsDelivr
    And the environment sets "GITHUB_REPOSITORY" to "owner/demo"
    And a stand-in "curl"
    When I run "pubrel tag"
    Then it succeeds
    And it prints "jsDelivr: warmed 1 and purged 3 URLs for v0.1.0"
    And "curl" was run with arguments matching "* https://cdn.jsdelivr.net/gh/owner/demo@v0.1.0/app/app.js"
    And "curl" was run with arguments matching "* https://purge.jsdelivr.net/gh/owner/demo@0/app/app.js"
    And "curl" was run with arguments matching "* https://purge.jsdelivr.net/gh/owner/demo@0.1/app/app.js"

  Scenario: A jsDelivr failure is a warning, after the release is made
    Given a cargo package released at version "0.1.0"
    And release.json serves "app/app.js" through jsDelivr
    And the environment sets "GITHUB_REPOSITORY" to "owner/demo"
    And a stand-in "curl" that fails, saying "curl: (22) failed"
    When I run "pubrel tag"
    Then it succeeds
    And the tag "v0.1.0" exists and names the package publet
    And stderr mentions "warning: jsDelivr"

  Scenario: A jsDelivr file outside the repository is refused
    Given a cargo package released at version "0.1.0"
    And release.json serves "../secrets" through jsDelivr
    When I run "pubrel tag"
    Then it fails
    And stderr mentions "`jsdelivr` needs `files`"
