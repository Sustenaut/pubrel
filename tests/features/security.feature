Feature: A release carries a signed record of its security checks
  release.json may declare security checks -- commands such as `cargo deny
  check` -- and facts to keep beside them. `prepare` runs them before it
  releases anything and records what ran as a `security` claim in the
  package publet, signed with the release. A failing check stops the
  release; check and tag refuse a release whose record is missing or
  unsigned.

  Background:
    Given a cargo package released at version "0.1.0"

  Scenario: The checks run, and the release records them
    Given release.json declares the security check "advisories" that prints "advisories ok" with tool "cargo-deny 9.9.9"
    And release.json declares the security fact "advisory database" that prints "abc1234"
    And the unreleased changes "fixed" pushed to main
    When I run pubrel prepare
    Then it succeeds
    And the published security record lists the check "advisories" with "advisories ok" from "cargo-deny 9.9.9"
    And the published security record lists the fact "advisory database" as "abc1234"
    And CHANGELOG.md has "advisories: `printf 'checking\nadvisories ok\n'` (cargo-deny 9.9.9) -- advisories ok" under "### Security checks"

  Scenario: A warning on stderr does not stand in for the check's summary
    Given release.json declares the security check "advisories" that prints "advisories ok" and warns "duplicate entries for crate syn"
    And the unreleased changes "fixed" pushed to main
    When I run pubrel prepare
    Then it succeeds
    And the published security record lists the check "advisories" with "advisories ok" from "cargo-deny 9.9.9"

  Scenario: A failing check stops the release before anything changes
    Given release.json declares the security check "advisories" that fails
    And the unreleased changes "fixed" pushed to main
    When I run pubrel prepare
    Then it fails
    And stderr mentions "security check `advisories` failed, so nothing is released"
    And stderr mentions "RUSTSEC-0000-0000"
    And the manifest states "0.1.0"
    And the unreleased list holds "fixed" "a fixed change"

  Scenario: A check does not run a stale tool from .venv
    Given release.json declares the security check "quality audit current" that runs ".venv/bin/pubkit audit --check"
    And tests/requirements.lock pins pubkit "0.15.2" and .venv has pubkit "0.9.1"
    And the unreleased changes "fixed" pushed to main
    When I run pubrel prepare
    Then it fails
    And stderr mentions ".venv has pubkit 0.9.1, but tests/requirements.lock pins 0.15.2: run `make venv`"
    And the manifest states "0.1.0"

  Scenario: A release whose security record lost its signature fails check
    Given release.json declares the security check "advisories" that prints "advisories ok" with tool "cargo-deny 9.9.9"
    When a branch records "fixed" "an overflow" and is released
    And the security record's signatures are removed
    And I run pubrel check against main
    Then it fails
    And stderr mentions "has no valid signature by"

  Scenario: A release cut without its declared checks fails check
    When a branch records "fixed" "an overflow" and is released
    And release.json now declares the security check "advisories" that prints "advisories ok" with tool "cargo-deny 9.9.9"
    And I run pubrel check against main
    Then it fails
    And stderr mentions "declares security checks, but the release of demo records none"
