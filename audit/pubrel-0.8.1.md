---
package: pubrel
version: 0.8.1
standard: 45390b67d5e4cbf27491f4de7ea4a2ca013482e4ebc474978fd5a1d49c65e163
blocking: 0
date: 2026-11-19
auditor: pubrules audit prepare (pubrules 0.45.1), judged rules reviewed by Claude Code following rust-crate-quality
commit: 559d21f91b395fd923ae8e4fd9e20fd60936a0c1
last-release: v0.8.0
---

# Quality audit: pubrel 0.8.1

Audited against the Rust crate standard named above, at `559d21f` (#112's
merge on main). 0.8.1 is the version `pubrel next --package pubrel` gives
for its unreleased rows: a `fixed` row (#112: the claim commit `prepare`
pushes to `origin` names a random token, its pid and time, and no longer
the hostname; the worktree's links have doc comments; the README's
`prepare` row matches `--help`) and an `internal` row (#111: the suite's
scenarios on pubtest's steps, which merged after 0.8.0's release branch was
cut). pubrel is a binary-only crate, so section 9 applies. Since `v0.8.0`
only `src/main.rs` (the claim's text), `src/unfinished.rs` (`token`, doc
comments), the README, one scenario and its steps changed.

## Blocks a release

None.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads** (§4.1;
   `fuzz/fuzz_targets/`). Carried from 0.8.0's finding 1:
   `published::read_object` has no target, and SECURITY.md states the gap.
   #112 reads no new outside input: `unfinished::token` hashes the process
   id with std's per-process random keys.
2. **45 private items have no doc comment** (§9.2), counted on a fresh
   build (`cargo clippy -p pubrel -- -W clippy::missing_docs_in_private_items`
   after touching `src/main.rs`). Down from 0.8.0's 47: #112 documents
   `unfinished::link` and `unfinished::link_dir` (0.8.0's finding 2), and
   its new `unfinished::token` is documented. The 45 predate `v0.6.2`.

## Worth considering

3. **On Windows the worktree's links need symlink rights** (§9.4;
   `src/unfinished.rs`). Carried from 0.8.0's finding 4: `symlink_file`
   and `symlink_dir` fail without Developer Mode or the privilege, so
   `prepare` would fail there and undo itself; the doc comments now say
   so. CI runs the unit tests on Windows, not the functional suite.
4. **No property tests** (§3.2; `Cargo.toml`). Carried from 0.8.0's
   finding 5.
5. **Related tools: newer releases exist** (§9.8; `README.md:430-432`).
   Carried from 0.8.0's finding 6, checked with `cargo search` on
   2026-11-19: release-plz 0.4.3, knope 0.24.0 and cargo-release 1.1.7,
   against 0.3.170, 0.23.0 and 1.1.6 in the rows.
6. **Semicolons joining two ideas** (§10.1). Carried from 0.8.0's finding 7;
   #112's new text adds none that join two ideas.

## Sections met

- **§9, metadata:** `documentation` points at the README.
- **§5, metadata:** description, licence, repository, documentation,
  keywords, categories, `include` and `rust-version` are set.
- **§5, include:** every pattern is anchored with `/`.
- **§5, publish-check:** pubrules.mk's `publish-check` builds in
  `target/publish-check`.
- **The standard** is pubrules 0.45.1's (`45390b6`): the same 14 rules need
  review as under 0.8.0's, each judged here.
- **§9.1, §9.5:** 0.8.0's finding 3 is fixed. The README's `prepare` row now
  says what `--help` says: a worktree under `.git/pubrel/` from
  `origin/main`, the claim on `origin`, and `--no-pr` claiming nothing.
  `--help` exits 0 and prints to stdout, unchanged since 0.8.0.
- **§9.3 versioned interface:** unchanged. The claim commit's message is
  not output meant for scripts; `withdraw --unfinished` reads its "Being
  prepared by" line, and the README names that line's new shape ("pubrel
  X, token T, pid N, at T"). A claim written by 0.8.0 still parses: the
  listing reads the line whole, and `is_claim` reads the subject.
- **§9.4:** `tests/features/unfinished.feature` gains a scenario that reads
  the claim `origin` received, through a `post-receive` hook, and checks it
  names a token, pid and time and not the machine's hostname; 116
  scenarios in all.
- **§7.1 SECURITY.md:** unchanged since 0.8.0. The machine's finding at
  `SECURITY.md:75` misreads "fuzzing of part of what `check` reads from a
  pull request", as 0.7.0's report explains: that line is under the merge
  queue and pushes to main, where fuzzing runs.
- **§3.3, §3.4, §9.7, §5, §5.5, §9.2 module docs, §9.8 crates pubrel builds
  on:** as 0.8.0 found, with no change to them since.
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
