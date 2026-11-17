---
package: pubrel
version: 0.5.4
standard: bc3c5b6eeba4757b5d96bc00be2d0a1d3221bef89072b06a271c2b28e2205e34
blocking: 0
date: 2026-10-20
auditor: rust-crate-quality
---

# Quality audit: pubrel 0.5.4

Audited at `470953e` (origin/main) plus uncommitted edits to README.md
and SECURITY.md, against the Rust crate standard named above. This is a
re-audit. The first audit found two false claims, and these edits remove
both. pubrel is a binary-only crate, so section 9 applies: its
documentation is its README and its `--help`. The library-only rules
(RS-DOC-01..04, RS-TEST-02/03, docs.rs build, doctests) do not apply.
0.5.4 is the version `pubrel next` gives for the unreleased rows: two
`added` and two `internal`, which below 1.0 make a patch.

## Blocks a release

None.

The first audit's two blockers are fixed:

- README.md:389-393 and SECURITY.md:60-65 now say that only part of what
  `check` reads is fuzzed. They name the package publet's objects as not
  fuzzed. That matches fuzz/fuzz_targets/ and `check_release`
  (src/main.rs:440-480).
- README.md:381 now says the Gherkin suite covers every command. Every
  command is run by tests/features: `init` and `--version` in cli.feature,
  and the rest in their own features.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads** (§4,
   judged; fuzz/fuzz_targets/). The docs now state the gap truthfully.
   The rule still asks for a target. `published::claim_content`,
   `published::kind_and_purpose` and `published::read_object` parse bytes
   a pull request can change, as do the signature and rotation-chain
   checks behind `published::require_signed` (src/published.rs:52-80,
   115-151, 166-300). So does `withdraw::commit_named`.
   fuzz/src/lib.rs already compiles `published.rs`, but no target calls
   it. Adding one is a code change, for a person to decide.

2. **"covered by the Gherkin suite" is broader than the suite**
   (§10 "name things exactly"; README.md:393, SECURITY.md:63-65). The
   suite exercises loading objects, identifier mismatch, missing
   signatures and a key that is not a key (tests/features/verify.feature).
   No feature exercises following a key rotation. That path is covered by
   unit tests in src/published.rs
   (`a_release_signed_by_a_rotated_from_key_verifies`,
   `a_key_outside_the_chain_is_refused`). The sentence should say
   "covered by the Gherkin suite and unit tests", or name what each
   covers.

## Worth considering

3. **MSRV range claim unverified** (§1.8 as applied by §9, §10;
   README.md:358-359). The claim "every release from 1.88 to 1.99 was
   checked when it was set" is unverified. This audit confirmed only 1.88
   (`make msrv` passes) and 1.98.1 (the pinned toolchain), and the
   shallow clone has no history of the check. The rest of the section
   holds:
   - `let` chains are stable in 1.88;
   - the `msrv` CI job exists;
   - pubkit's rust-ci.yml v0.11.1 runs latest stable and beta on a
     schedule;
   - the stated policy is a minor bump.

4. **Wording of the new text** (§10).
   - README.md:393 uses a semicolon to join two ideas ("signatures; that
     code is covered...") and was not rewrapped.
   - SECURITY.md:66-74: the `cargo deny` and `cargo vet` bullets end in
     full stops inside a list whose other items end in semicolons. The
     `cargo vet` bullet carries a second sentence.
   - The "In every release" sentence (SECURITY.md:80-86) carries four
     clauses. Two sentences would read better.

5. **The `version` spelling is not exercised.** README.md:47 now lists
   `version` beside `--version`, and README.md:381 says the suite covers
   every command. The suite runs `--version` but never `version`. Both
   are handled by the same match arm (src/main.rs:61). A one-row
   Examples table in cli.feature would close this.

6. **Undocumented private items** (§9 "module docs still matter").
   `cargo clippy -- -W clippy::missing_docs_in_private_items` reports 43
   of them, and none are `pub`. Examples:
   - `main::USAGE` (src/main.rs:31);
   - the command functions in src/main.rs;
   - `published::to_json` (src/published.rs:95);
   - `security::shell` (src/security.rs:123).

   Every `pub` item another module relies on is documented, with
   `# Errors` where it returns a `Result`. Every module has `//!` docs.

7. **Related tools: one detail unverified.** These were checked, and
   each exists on crates.io and matches the README:
   - release-plz 0.3.170 depends on `git-cliff-core` (its Cargo.toml).
   - knope 0.23.0 uses conventional commits and changesets (its README
     and `src/step/create_change_file.rs`).
   - cargo-release 1.1.6 takes `LEVEL|VERSION`
     (`src/steps/version.rs:43`).
   - cargo-semver-checks 0.51.0.

   The `.changeset/` directory name for knope is unverified. Its constant
   lives in `knope-versioning`, which was not fetched.

## Resolved since the first audit

- `--help` and the README now agree: the Commands table lists `version`
  (README.md:47).
- SECURITY.md's "In every release" names the quality-audit check
  (SECURITY.md:80-83), matching release.json's four checks.
- SECURITY.md's CI list is one list.

## Sections met

- **§4 `unsafe`:** forbidden (`unsafe_code = "forbid"`). The one fuzzing
  bug has its regression input
  (`fuzz/regressions/manifest/version-at-u64-max`) and a named test
  (`version::tests::a_version_at_the_top_is_refused_not_overflowed`).
- **§5 packaging:** description, licence, repository, keywords,
  categories, `rust-version` and an anchored `include` are set.
  `cargo package --list` ships only the sources, README, CHANGELOG,
  LICENSE, SECURITY.md and SUPPLY-CHAIN.md. `documentation` points at
  the README, as §9 asks.
- **§6 versions:** 0.5.4 is the bump the recorded rows require.
- **§9 README:** it has all of these:
  - purpose and central use;
  - commands;
  - why pubrel;
  - related tools;
  - installation;
  - MSRV;
  - versioning, and the versioned interface (commands, exit codes,
    script output, files, the workflow).

  `--help`, `-h` and `help` exit 0 and print to stdout, with nothing on
  stderr.
- **`// covers:` spot-checks:** the tests for `version::next`,
  `version::parse` and `jsdelivr::urls` exercise what they claim.

## Checks run

| Check | Result |
|---|---|
| `make doc` | pass |
| `make lint` | pass |
| `make fmt-check` | pass |
| `make test` | pass, 31 tests |
| `make functional` | pass, 89 scenarios |
| `make msrv` (1.88) | pass |
| `make publish-check` | pass at clean HEAD. On the edited tree it stops because README.md and SECURITY.md are uncommitted. The same command with `--allow-dirty` passes, so it passes once the edits are committed. |
| `make supply-chain-check` | pass (re-run after the edits) |
| `make doc-coverage-check` | pass (not required for a binary-only crate) |
| `make sync-check` | pass (re-run after the edits). Conforms, 9 rules. Open exception RS-TEST-01 until 2027-01-12. |
| `make vectors-check` | fails: no `tools/vectors/generate.py` and no `testdata/`. RS-TEST-01 is under a declared exception, so this is not reported as a gap. |
| `cargo clippy -- -W clippy::missing_docs_in_private_items` | 43 warnings, none on a `pub` item |
| `cargo info` for the related crates | ran; see finding 7 |

The Rust checks ran before the documentation edits. The edits touch only
Markdown, which no Rust check reads in a binary-only crate.

## Checks not run

- **docs.rs build:** not applicable to a binary-only crate (§9). The
  clippy private-docs run replaces it.
- **`cargo test --doc`:** there is no library target.
- **`make fuzz-smoke`:** no nightly toolchain or cargo-fuzz installed.
- **`make deny`, `make vet`:** cargo-deny and cargo-vet are not
  installed.

This report should be committed with the release's changes, together with
the README.md and SECURITY.md edits it audited.
