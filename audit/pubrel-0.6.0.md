---
package: pubrel
version: 0.6.0
standard: bc3c5b6eeba4757b5d96bc00be2d0a1d3221bef89072b06a271c2b28e2205e34
blocking: 0
date: 2026-10-29
auditor: pubkit audit prepare (pubkit 0.17.0), judged rules reviewed by Claude Code following rust-crate-quality
commit: 00538b21f9d1c5a2a2f6ffeac26a61bae54d9219
last-release: v0.5.5
---

# Quality audit: pubrel 0.6.0

Audited at `00538b2` (origin/main, with Sustenaut/pubrel#67 merged)
against the Rust crate standard named above. 0.6.0 is the version `pubrel
next` gives for the unreleased changes: one `internal` and two `fixed` in
the pubrel 0.5 `unreleased.json`, and one `changed` in
`unreleased/`, which below 1.0 makes a minor bump. pubrel is a
binary-only crate, so section 9 applies. The library-only rules do not:
sections 1 and 2, §3.1 under its declared exception, and the docs.rs
build.

`pubkit audit prepare` wrote the header, ran the checks, decided the rules
a machine can decide, and listed the evidence for the 14 rules that need
judgement. Each of those was then judged by reading the code, docs and
workflows, and its finding moved below. Changes since `v0.5.5`: unreleased
changes as one file each (`src/main.rs`, `src/record.rs`, the
`unreleased_json` fuzz target, fragments.feature), the stale-`.venv`
refusal and the audit-pin hint (`src/security.rs`), and the move to
Sustenaut/pubrel with pubkit 0.17.0. This file should be committed with
the release's changes.

## Blocks a release

None.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads** (§4, judged;
   `fuzz/fuzz_targets/`). This is unchanged since 0.5.4. The targets are
   `corpus_lock`, `manifest`, `release_json` and `unreleased_json`.
   `published::read_object` (src/published.rs:127) and the signature
   checks behind `published::require_signed` parse bytes a pull request can
   change. SECURITY.md states the gap truthfully: it lists these objects
   as "not fuzzed". The new parsers are covered. `record::change`,
   `record::brief` and `record::fragment_name` are driven by
   `unreleased_json`, and the new `Config::summary_rel` and
   `Config::legacy_rel` by `release_json`. `security::venv_current`,
   `pinned` and `installed` read `tests/requirements.lock` and `.venv` on
   the releasing machine, from main, so they read no untrusted input.
   Adding a target for the objects is a code change, for a person to
   decide.

## Worth considering

2. **SECURITY.md omits two checks CI runs** (§7.1; SECURITY.md:46-75).
   This is carried from 0.5.5. OSV-Scanner runs in pr.yml, main.yml and
   nightly.yml (`job-osv`). The dependency review runs on pull requests
   (pr.yml, `job-review`).

3. **"On every pull request ... CI runs" includes jobs a pull request may
   skip** (§7, §10 "name things exactly"; SECURITY.md:46, 58-63). This is
   carried from 0.5.5. The cross-target tests and fuzzing pass at once,
   with a notice, on a pull request that touches no Rust code, Cargo
   files, `fuzz/` or workflow files.

4. **No property tests** (§3.2; Cargo.toml). proptest is not a
   dev-dependency. `record::fragment_name` is a new candidate: for any
   change text, the name is a single path component, ends in `.json`, and
   sorts after an earlier stamp. The fuzz target already asserts the
   first two.

5. **Related tools: newer releases exist, and one row reads older**
   (§9.8; README.md:348-350). `cargo search` today gives release-plz 0.4.2
   (the row checked 0.3.170), knope 0.23.2 (row 0.23.0) and cargo-release
   1.1.7 (row 1.1.6). Each row names the version checked, so none is
   wrong. knope's row says "Its changeset files are the nearest thing to
   pubrel's rows". Since 0.6.0, pubrel's changes are also one file each, so
   that comparison now understates the likeness. cargo-semver-checks
   0.51.0 and publet-core 0.1.4 are current. The crates pubrel builds on
   match Cargo.lock: semver 1.0.28, toml_edit 0.22.27 and serde_json
   1.0.154.

6. **Wording of the new text** (§10, §9.5). Each of these joins two ideas
   with a semicolon:
   - README.md:53: "A file can be written or edited by hand; `next`,
     `check` and `prepare` refuse one ...".
   - src/main.rs:281, user-facing: "release.json written; `pubrel add`
     records each change in ...".
   - src/security.rs:194, user-facing: "... what a machine can check;
     complete the rules it marks for review, then prepare again".
   - src/record.rs:338 and src/record.rs:398, doc comments.

   `pubkit audit prepare` lists about 25 more semicolons, such as
   README.md:224, src/main.rs:7 and src/record.rs:44-119. They predate
   0.5.5 and are not new text.

## Sections met

- **§9.1 README and `--help`:** the README has what the tool is for, its
  commands, why pubrel, related tools, installation, MSRV and versioning.
  `--help` and the README's Commands table agree on the changed `init`,
  `add` and `summary` (README.md:38-40 and the usage text). The new
  "Unreleased changes: one file each" section describes the layout, the
  name order, and the pubrel 0.5 `unreleased.json` still read.
- **§5.5 README:** it has a Documentation line, Using it, an example, MSRV
  and Security.
- **§9.2 module docs:** `cargo clippy -- -W
  clippy::missing_docs_in_private_items` reports none. `main.rs`'s `//!`
  docs describe the `unreleased/` directory, and each new function has a
  doc comment.
- **§9.3 versioned interface:** README.md:391-411 names the commands and
  flags, exit codes, script output and files, and now lists the
  `unreleased/` directory and `summary.md` as files pubrel reads and
  writes. The layout change is recorded as `changed`, which gives 0.6.0.
  `init` no longer writing `unreleased.json` is part of the same row.
- **§9.4 end to end:** 14 feature files. fragments.feature covers:
  - two branches each recording a change merging cleanly;
  - `prepare` merging in name order and deleting only;
  - a malformed file named in the error;
  - `check` refusing a branch that only rewords a change, and a release
    that misses a change main gained;
  - the pubrel 0.5 list read and migrated.
- **§9.5 usage text:** `--help` exits 0 and prints 27 lines to stdout,
  nothing to stderr. It is accurate for every command. Finding 6 notes
  two user-facing messages.
- **§3.3 oracles:** no established crate implements Cargo's bump rules as
  pubrel applies them to recorded categories. Versions are parsed by
  `semver` itself, which is used rather than reimplemented.
- **§3.4 and §9.7 `// covers:`:** spot-checked the new comments.
  `record::change` (src/record.rs:900), `record::fragment_name` (:914),
  `record::brief` (:934), `security::venv_current` and
  `security::next_step` are exercised by the tests under them. The
  comments use module paths, as §9 allows.
- **§7.1 SECURITY.md:** it says how to report privately, what is checked
  and when, and what fuzzing found. Each tool it names is run by a
  workflow on the triggers it gives, apart from findings 2 and 3. Its
  fuzzing line names "the unreleased changes" for the new layout.
- **§5 packaging:** metadata, an anchored `include` and pubkit.mk's own
  `publish-check` target directory. `documentation` points at the README,
  as §9 asks.
- **RS-TEST-01:** under a declared exception, until 2027-01-12 (no test
  vectors yet), so not reported as a gap.

## Checks run

| Check | Result |
|---|---|
| `cargo doc -p pubrel --no-deps` | pass |
| `cargo clippy -p pubrel --all-targets` | pass |
| `cargo clippy -p pubrel -- -W clippy::missing_docs_in_private_items` | 0 undocumented private item(s) |
| `pubrel --help` | exit 0 |
| `cargo publish --dry-run --locked -p pubrel` | pass |
| `cargo +1.88 check --locked -p pubrel` (MSRV) | pass |
| `make supply-chain-check` (whole repository) | pass |
| `make deny` (whole repository) | pass |
| `make vet` (whole repository) | pass |
| `cargo search` for the related tools | ran; see finding 5 |

## Checks not run

- **`make fuzz-smoke` and `make functional`:** CI runs them on the release pull request. Both passed on Sustenaut/pubrel#67, whose head is this commit's content.
- **docs.rs build and `cargo test --doc`:** not applicable to a binary-only crate (§9).
- **`pubkit audit draft`:** not used; the judged rules were reviewed by Claude Code.
