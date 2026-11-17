---
package: pubrel
version: 0.7.0
standard: bc3c5b6eeba4757b5d96bc00be2d0a1d3221bef89072b06a271c2b28e2205e34
blocking: 0
date: 2026-11-13
auditor: pubkit audit prepare (pubkit 0.37.0), judged rules reviewed by Claude Code following rust-crate-quality
commit: ddc6f2cce0e787fb993c242a4950ab63e48a634d
last-release: v0.6.2
---

# Quality audit: pubrel 0.7.0

Audited against the Rust crate standard named above, on main at `ddc6f2c`.
0.7.0 is the version `pubrel next --package pubrel` gives for the three
unreleased rows: a `changed` row (`prepare` drives `pub` 0.4 as well as 0.1
to 0.3), a `fixed` row (#99: `release.json`'s `tag` must have a Section 9.1
tag's shape) and an `added` row (#100: `release.yml` takes `runs-on`). Below
1.0 a `changed` row is a minor bump. pubrel is a binary-only crate, so
section 9 applies.

`pubkit audit prepare` wrote the header, ran the checks, decided the rules a
machine can decide, and listed the evidence for the 14 rules that need
judgement. Each was judged by reading the code, docs and workflows. The Rust
changes since `v0.6.2` are all in `src/record.rs`: the accepted range of
`pub` (now `>=0.1.0, <0.5.0`), with its documentation, refusal message and
unit test, and #99's `section_tag`, which `Config::parse_all` calls on
`release.json`'s `tag`, with its own unit test. The rest is
`.github/workflows/release.yml` (#100), the README and generated pubkit files
from the 0.34.0 to 0.37.0 bumps. Every other judged rule about the code reads
as it did for 0.6.2.

The auditor wrote #100 and the 0.34.0 to 0.37.0 bumps, so its review of those
changes is not independent.

## Blocks a release

None.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads** (§4.1;
   `fuzz/fuzz_targets/`). Unchanged since 0.5.4, carried from 0.6.2's
   finding 1: `published::read_object` has no target. SECURITY.md states the
   gap (`SECURITY.md:79-81`). The other entry points the machine listed are
   reached by an existing target: `Config::read_all` and `Config::parse_all`
   (and through them #99's `section_tag`) by `release_json`,
   and `manifest::read`, `Kind::parse` and `version::parse` by `manifest`.
2. **45 private items have no doc comment** (§9.2). `cargo clippy -p pubrel --
   -W clippy::missing_docs_in_private_items` reports 45, across `main.rs`,
   `jsdelivr.rs`, `manifest.rs`, `published.rs`, `record.rs`, `security.rs`
   and `withdraw.rs`, such as `record::Config::publet_rel` (`src/record.rs:275`).
   Each item predates `v0.6.2`, and neither change in this release adds one:
   `section_tag` is documented. 0.6.2's report counted 0 with an older
   toolchain, so the count rather than the code moved. Documenting them is
   work for its own pull request.

## Worth considering

3. **#99 refuses a `release.json` that 0.6.2 accepted** (§9.3). A `tag`
   without a Section 9.1 shape now fails every command, so the configuration
   pubrel reads got stricter. It is recorded as `fixed`, since `pub` already
   refused such a tag when `prepare` published the package publet, so no
   release could have come of one. The `changed` row for `pub` 0.4 makes this
   a minor release anyway.
4. **No property tests** (§3.2; `Cargo.toml`). Carried from 0.6.2's finding 3.
   `section_tag` would suit one, with tags built from random parts, but its
   unit test covers each boundary (month 00 and 13, two- and seven-letter
   domains, five- and seven-letter names, a two-digit year, a fifth part).
5. **Related tools: newer releases exist** (§9.8; `README.md:379-381`).
   `cargo search` today gives release-plz 0.4.3 (the row names 0.3.170), knope
   0.23.2 (row 0.23.0) and cargo-release 1.1.7 (row 1.1.6). Each row names the
   version checked, so none is wrong. cargo-semver-checks is 0.51.0, as its
   row says. The crates pubrel builds on match Cargo.lock: publet-core 0.1.4,
   semver 1.0.28, toml_edit 0.22.27 and serde_json 1.0.154.
6. **Semicolons joining two ideas** (§10.1). The machine lists 30, in the
   README and module docs. Carried from 0.6.2's finding 5. The `require_pub`
   doc comment #99's range change rewrote still ends one list with "none of
   which pubrel relies on". #99's own `section_tag` doc and the README
   paragraph each keep to one idea per sentence.

## Sections met

- **§7.1 SECURITY.md:** the machine's finding at `SECURITY.md:75` misreads it.
  That line is under "In the merge queue and on every push to `main`", and
  "fuzzing of part of what `check` reads from a pull request" describes what
  is fuzzed (input that arrives in a pull request), not when. The workflows
  run fuzzing in `queue.yml`, `main.yml` and `nightly.yml`, as it says. The
  other checks it lists (cargo deny, cargo vet, OSV-Scanner, dependency
  review, the 32-bit and big-endian tests) run where it says they do.
  Private reporting is described.
- **§9.1 README and `--help`:** no command or flag changed. The README now
  defines the `tag` shape #99 checks, the `pub` range (0.1 to 0.4) and the
  release workflow's `runs-on`. `--help` names every command, as before.
- **§9.3 versioned interface:** the README names the commands, flags,
  `release.json` and the release workflow's inputs. `runs-on` is a new input
  with the old behaviour as its default, recorded as `added`. See finding 3
  for #99.
- **§9.4 end to end:** 14 feature files. `make check` passes locally against
  the released `pub` v0.3.0: 44 unit tests and 104 scenarios. The `pub` 0.4
  path rests on its unit test and the pull request that admitted it. No
  local run drove `pub` 0.4.
- **§9.5 usage text:** `pubrel --help` exits 0 and prints 27 lines to stdout,
  nothing to stderr.
- **§9.2 module docs:** `src/record.rs`'s module docs give the new `pub`
  range. `section_tag`'s doc says what it checks and why. See finding 2 for
  the private items.
- **§3.3 oracles:** versions are parsed by `semver` itself. The tag shape has
  no established implementation to compare against.
- **§3.4 and §9.7 `// covers:`:** 41 comments, one added by #99
  (`record::section_tag`, whose test calls `section_tag` directly). Spot
  checks of `api::allowed`, `api::recorded`, `jsdelivr::urls` and
  `manifest::write, manifest::read` find each test exercising what it
  claims. `require_pub`'s test still checks only the range it applies.
- **§5 packaging and §5.5 README:** as 0.6.2 found. The README is short,
  links its documentation, and has Usage, one example, the minimum Rust
  version and Security.
- **RS-TEST-01:** under a declared exception until 2027-01-12.

## Checks run

| Check | Result |
|---|---|
| `cargo doc -p pubrel --no-deps` | pass |
| `cargo clippy -p pubrel --all-targets` | pass |
| `cargo clippy -p pubrel -- -W clippy::missing_docs_in_private_items` | 45 undocumented private items; see finding 2 |
| `pubrel --help` | exit 0 |
| `cargo publish --dry-run --locked -p pubrel` | pass |
| `make check` (fmt, lint, tests, docs, functional, publish-check, supply chain, vet, deny) against the released `pub` v0.3.0 | pass: 44 unit tests, 104 scenarios |
| `cargo search` for the related tools | ran; see finding 5 |

## Checks not run

- **The MSRV build (`cargo +1.88 check`):** 1.88 is not installed here. CI
  builds it in the merge queue and on main.
- **`make fuzz-smoke`:** CI runs it in the merge queue and on main.
- **docs.rs build and `cargo test --doc`:** not applicable to a binary-only
  crate (§9).
- **`pubkit audit draft`:** not used. The judged rules were reviewed by
  Claude Code.
