Feature: The version follows from what changed
  Versions follow Cargo's convention. From 1.0.0: changed or removed bumps
  major, added bumps minor, fixed or security bumps patch. Below 1.0.0
  every part shifts one place right: changed or removed bumps minor, and
  anything compatible bumps patch. A package never released is first
  released at its manifest version, raised to at least 0.1.0.

  Background:
    Given a scratch directory
    And the program under test is "pubrel"

  Scenario Outline: A first release is the manifest version, at least 0.1.0
    Given a released-never cargo package at version "<manifest>"
    And the unreleased changes "fixed"
    When I run "pubrel next"
    Then it prints "<first>"

    Examples:
      | manifest | first |
      | 0.0.1    | 0.1.0 |
      | 0.1.0    | 0.1.0 |
      | 0.2.2    | 0.2.2 |

  Scenario Outline: Below 1.0, a break bumps minor and anything else patch
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "<categories>"
    When I run "pubrel next"
    Then it prints "<next>"

    Examples:
      | categories    | next  |
      | added         | 0.1.1 |
      | changed       | 0.2.0 |
      | fixed,removed | 0.2.0 |
      | fixed         | 0.1.1 |
      | security      | 0.1.1 |

  Scenario Outline: From 1.0, each category bumps its part
    Given a cargo package released at version "1.4.2"
    And the unreleased changes "<categories>"
    When I run "pubrel next"
    Then it prints "<next>"

    Examples:
      | categories    | next  |
      | added         | 1.5.0 |
      | changed       | 2.0.0 |
      | fixed,removed | 2.0.0 |
      | fixed         | 1.4.3 |
      | security      | 1.4.3 |

  Scenario: Nothing to release is refused
    Given a cargo package released at version "0.1.0"
    When I run "pubrel next"
    Then it fails
    And stderr mentions "nothing has changed"

  Scenario: Internal changes alone release nothing
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "internal"
    When I run "pubrel next"
    Then it fails
    And stderr mentions "only internal changes are recorded"

  Scenario: Internal changes ride along with a real one
    Given a cargo package released at version "0.1.0"
    And the unreleased changes "internal,fixed"
    When I run "pubrel next"
    Then it succeeds
    And it prints "0.1.1"
