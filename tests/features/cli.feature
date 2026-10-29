Feature: Starting out, and asking pubrel about itself
  `pubrel init` writes a repository's release.json, and says where
  `pubrel add` records changes. `--version` prints the line the release workflow matches, and
  `--help` prints the usage. An unknown command fails, with the usage on
  stderr.

  Scenario: init writes release.json and names the unreleased directory
    Given a fresh cargo repository named "demo"
    When I run pubrel "init"
    Then it succeeds
    And it prints "release.json written; `pubrel add` records each change in corpus/publets/pkg.demo/unreleased/."
    And release.json names the package "demo" as "pkg.demo" with a "cargo-package" manifest at "Cargo.toml"
    And the unreleased list is empty

  Scenario: init leaves an existing release.json alone
    Given a fresh cargo repository named "demo"
    And pubrel "init" has run
    When I run pubrel "init"
    Then it fails
    And stderr mentions "release.json already exists"

  Scenario: --version prints the line the release workflow matches
    Given a fresh cargo repository named "demo"
    When I run pubrel "--version"
    Then it succeeds
    And it prints this pubrel's version as "pubrel X.Y.Z"

  Scenario Outline: Help is printed on request, to stdout
    Given a fresh cargo repository named "demo"
    When I run pubrel "<flag>"
    Then it succeeds
    And it prints "usage: pubrel <command> [--package NAME]"
    And it prints "summary [TEXT...]"
    And stderr is empty

    Examples:
      | flag   |
      | --help |
      | -h     |
      | help   |

  Scenario: An unknown command fails, with the usage on stderr
    Given a fresh cargo repository named "demo"
    When I run pubrel "frobnicate"
    Then it fails
    And stderr mentions "usage: pubrel <command> [--package NAME]"
