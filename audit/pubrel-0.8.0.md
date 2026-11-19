---
package: pubrel
version: 0.8.0
standard: 4db4feebe0e2037f0b045bf2675ab88f010eec4141bc9f4c731d8eb35c8eec58
blocking: 0
date: 2026-11-19
auditor: pubrules audit prepare (pubrules 0.43.0), judged rules reviewed by Claude Code following rust-crate-quality
commit: 7df2937bb104c2f54398c70332c8a744268e8b06
last-release: v0.7.1
---

# Quality audit: pubrel 0.8.0

Audited against the Rust crate standard named above, at `7df2937` (#108's
merge on main). 0.8.0 is the version `pubrel next --package pubrel` gives
for its unreleased rows: a `changed` row (prepare cuts the release from
`origin/main` in a worktree of its own, so the checkout is never edited),
an `added` row (prepare claims its release branch on `origin` first;
`withdraw --unfinished [BRANCH]` clears a killed prepare), a `fixed` row
(records an unfinished attempt left at the same commit, with their
signatures, go to `.draft-discards`), and #107's move to pubrules 0.43.0.
pubrel is a binary-only crate, so section 9 applies.

## Blocks a release

None.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads** (§4.1;
   `fuzz/fuzz_targets/`). Carried from 0.7.1's finding 1:
   `published::read_object` has no target, and SECURITY.md states the gap.
   Every other entry point the machine lists is reached by the
   `release_json` or `manifest` target. #108 adds no parser of outside
   bytes: `published::signature_target` reads a field of an object through
   the same `load` as `read_object`, and the claim commit's subject is
   matched as a prefix of text pubrel itself wrote.
2. **47 private items have no doc comment** (§9.2), counted on a fresh
   build (`cargo clippy -p pubrel -- -W clippy::missing_docs_in_private_items`
   after touching `src/main.rs`; `audit prepare` reports 0 from clippy's
   cache). 45 are carried from 0.7.1's finding 2. The 2 new ones are
   `unfinished::link` and `unfinished::link_dir` (`src/unfinished.rs:175`,
   `:185`), each one line per platform; every other item #108 adds is
   documented, and `unfinished.rs` has module docs that say what it does
   and why.

## Worth considering

3. **The README's `prepare` row lags `--help`** (§9.1; `README.md:71`).
   `--help` says prepare cuts the release "in a worktree from origin/main:
   claim its branch"; the README's commands table still says only that it
   "commits on a `release/` branch". The section "Where a prepare works, and
   one that does not finish" says it all, and the `withdraw --unfinished`
   row links there; the `prepare` row could link there too.
4. **On Windows the worktree's links need symlink rights** (§9.4;
   `src/unfinished.rs:179`). `symlink_file` and `symlink_dir` fail without
   Developer Mode or the privilege, so `prepare` would fail there, and undo
   itself. CI runs the unit tests on Windows, not the functional suite, so
   no scenario covers it. pubrel's releases are cut on Linux.
5. **No property tests** (§3.2; `Cargo.toml`). Carried from 0.7.1's
   finding 4.
6. **Related tools: newer releases exist** (§9.8; `README.md:430-432`).
   Checked with `cargo search` on 2026-11-19: release-plz 0.4.3, knope
   0.24.0 and cargo-release 1.1.7, against 0.3.170, 0.23.0 and 1.1.6 in the
   rows. cargo-semver-checks is still 0.51.0, and the four crates pubrel
   builds on match `Cargo.lock`.
7. **Semicolons joining two ideas** (§10.1). Carried from 0.7.1's finding 6,
   35 places as the machine lists them; #108's new text in the README and
   `unfinished.rs` adds none that join two ideas.

## Sections met

- **§9.3 versioned interface:** the changes to what users depend on are
  recorded. `prepare` no longer refuses a dirty checkout or a HEAD other
  than `origin/main`, no longer leaves the checkout on the release branch,
  and pushes a claim to `release/vX` on `origin` before it cuts (the
  `changed` and `added` rows); `withdraw --unfinished [BRANCH]` is new (the
  `added` row). For a 0.x release, `changed` bumps the minor version: 0.8.0.
  `prepare --no-pr` still prints "committed on BRANCH; push it and open a
  PR to release", so a script that reads that line still works.
- **§9.1, §9.5:** `--help` exits 0, prints to stdout, and names every
  command and flag, `withdraw --unfinished [BRANCH]` included; the README's
  commands table names the same, apart from finding 3.
- **§9.4:** `tests/features/unfinished.feature` runs the built binary
  through a failed and a successful prepare (checkout, signing key and
  store hashed before and after), a second clone's claim, a killed
  prepare, two `--no-pr` prepares in a row, a plan from main this checkout
  has not pulled, and a claim another clone holds; 115 scenarios in all.
- **§3.4, §9.7:** the new tests are functional scenarios; the `// covers:`
  comments spot-checked (`api`, `jsdelivr`, `manifest`, `withdraw`) each
  exercise the items they name.
- **§7.1 SECURITY.md:** its only change is `pubkit` renamed `pubrules`
  (#107). The machine's finding at `SECURITY.md:75` misreads "fuzzing of
  part of what `check` reads from a pull request", as 0.7.0's report
  explains: that line is under the merge queue and pushes to main, where
  fuzzing runs.
- **§3.3, §5, §5.5, §9.2 module docs:** as 0.7.1 found; `unfinished.rs`
  has module docs.
- **Not applicable to a command-line tool (binary only):** §1.1, §1.2,
  §1.3, §1.4, §1.5, §1.6, §1.7, §1.8, §1.9, §2.1, §2.2, §2.3, §2.4, §2.5,
  §2.6, §2.7, §3.1, §5.4.
- **RS-TEST-01:** under a declared exception (no test vectors yet: the
  version arithmetic and the files pubrel reads need cases from an
  independent source, such as Cargo's documented SemVer rules, generated
  outside pubrel), so not reported as a gap.

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

## Checks not run

- **`make fuzz-smoke` and `make functional`:** CI runs them on the release pull request.
