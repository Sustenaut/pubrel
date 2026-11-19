---
package: pubrel
version: 0.8.2
standard: 45390b67d5e4cbf27491f4de7ea4a2ca013482e4ebc474978fd5a1d49c65e163
blocking: 0
date: 2026-11-19
auditor: pubrules audit prepare (pubrules 0.45.1), judged rules reviewed by Claude Code following rust-crate-quality
commit: d73f5f49a3a162132e1537f48fff248916ce245f
last-release: v0.8.1
---

# Quality audit: pubrel 0.8.2

Audited against the Rust crate standard named above, at `d73f5f4` (#115's
merge on main). 0.8.2 is the version `pubrel next --package pubrel` gives
for its one unreleased row, a `fixed` row from #115: prepare reaches a
parent `corpus.json` names by a relative path from its worktree, through a
link in the prepare's own folder; `withdraw --unfinished` touches only
prepares' folders and the links they recorded, and clears a dead local
prepare's claim by its recorded token; the worktree's links are excluded by
exact path and never committed. pubrel is a binary-only crate, so section 9
applies. Since `v0.8.1`, `src/unfinished.rs`, `src/main.rs` and the README
changed.

## Blocks a release

None.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads, nor for the
   JSON prepare reads** (§4.1; `fuzz/fuzz_targets/`). Carried from 0.8.1's
   finding 1: `published::read_object` has no target, and SECURITY.md
   states the gap. #115 adds two readers of JSON with no target of their
   own: `unfinished::relative_parents` (`src/unfinished.rs:278`), the
   `parents` of a `corpus.json` at `origin/main`, and `Record::from_json`
   (`src/unfinished.rs:200`), a `prepare.json` under `.git/pubrel/`. Both
   parse with `serde_json` and read fields by name, returning nothing on a
   shape they do not expect, and the second reads a file pubrel itself
   wrote; neither is a parser of pubrel's own.
2. **50 private items have no doc comment** (§9.2), counted on a fresh
   build (`cargo clippy -p pubrel -- -W clippy::missing_docs_in_private_items`
   after touching `src/main.rs`). 45 are carried from 0.8.1's finding 2.
   The 5 new ones are in #115's `src/unfinished.rs`: `RECORD` (184),
   `Record::from_json` (200), `save` (227), `load` (234) and `remove_link`
   (520). The module docs say what each does as a whole, and `Record`, its
   fields and `to_json` are documented.

## Worth considering

3. **The relative parents' arithmetic is lexical** (§3.2;
   `src/unfinished.rs:242`, `:263`). `levels_above` and `lexical` take `..`
   by its text. The operating system resolves `..` after following links,
   so a parent path through a symlinked directory could land elsewhere
   than `pub` reaches from the checkout. Property tests over random relative paths (does
   `lexical(tree/corpus, p)` stay inside the prepare's folder whenever
   `levels_above` says it climbs no further) would pin this down. With
   0.8.1's finding 4: no property tests.
4. **On Windows the worktree's links need symlink rights** (§9.4). Carried
   from 0.8.1's finding 3, now also the relative parents' links.
5. **Related tools: newer releases exist** (§9.8; `README.md:430-432`).
   Carried from 0.8.1's finding 5.
6. **Semicolons joining two ideas** (§10.1). Carried from 0.8.1's finding 6;
   #115's README text adds a few ("…sits; `corpus.json` is never edited")
   in the same style.

## Sections met

- **§9, metadata:** `documentation` points at the README.
- **§5, metadata:** description, licence, repository, documentation,
  keywords, categories, `include` and `rust-version` are set.
- **§5, include:** every pattern is anchored with `/`.
- **§5, publish-check:** pubrules.mk's `publish-check` builds in
  `target/publish-check`.
- **The standard** asks the same 14 rules to be reviewed as 0.8.1's did,
  each judged here.
- **§9.3 versioned interface:** no command, flag or exit code changed. What
  `withdraw --unfinished` without a branch clears is narrower (a hand-made
  entry under `.git/pubrel/` is kept) and its claim clearing wider (a dead
  local prepare's claim, matched by token), both as the README describes,
  so a `fixed` row: a patch. The new `prepare.json` is pubrel's own record,
  not a file users write.
- **§9.1, §9.5:** `--help` is unchanged and still names every command and
  flag; the README's section "Where a prepare works, and one that does not
  finish" describes the per-prepare folder, the record, the parents' links
  and what `withdraw --unfinished` touches.
- **§9.4:** `tests/features/unfinished.feature` gains four scenarios that
  run the built binary, each failing on 0.8.1: a relative parent outside
  the repository (pub 0.4.0; `corpus.json` unchanged on the release branch),
  a hand-made link under `.git/pubrel/` that survives, a dead local
  prepare's claim cleared while another clone's is kept, and `.venv/` in
  `.gitignore` with no `.venv` or link in the release commit; 120 scenarios
  in all.
- **§7.1 SECURITY.md:** unchanged since 0.8.0. The machine's finding at
  `SECURITY.md:75` misreads "fuzzing of part of what `check` reads from a
  pull request", as 0.7.0's report explains: that line is under the merge
  queue and pushes to main, where fuzzing runs.
- **§3.3, §3.4, §9.7, §5.5, §9.2 module docs, §9.8 crates pubrel builds
  on:** as 0.8.1 found; `unfinished.rs`'s module docs describe the new
  layout.
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
