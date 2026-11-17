---
package: pubrel
version: 0.7.1
standard: bc3c5b6eeba4757b5d96bc00be2d0a1d3221bef89072b06a271c2b28e2205e34
blocking: 0
date: 2026-11-14
auditor: pubkit audit prepare (pubkit 0.37.1), judged rules reviewed by Claude Code following rust-crate-quality
commit: b92370ffe11a81a854e7fc1dce6ad1d66288f627
last-release: v0.7.0
---

# Quality audit: pubrel 0.7.1

Audited against the Rust crate standard named above, at `b92370f` (#105's
branch on main). 0.7.1 is the version `pubrel next --package pubrel` gives
for its one unreleased row, a `fixed` row: `release.yml` pins
`actions/cache` 6.1.0 and `actions/checkout` 7.0.1 by commit, both on Node
24 (#104, recorded as `fixed` by #105). pubrel is a binary-only crate, so
section 9 applies.

`pubkit audit prepare` wrote the header, ran the checks, decided the rules a
machine can decide, and listed the evidence for the 14 rules that need
judgement. The only change since `v0.7.0` is three `uses:` lines in
`.github/workflows/release.yml` and the changelog row. No Rust source, no
README or SECURITY.md, and no `Cargo.toml` or `Cargo.lock` changed (`git diff
v0.7.0 -- src Cargo.toml Cargo.lock README.md SECURITY.md` is empty). So every
judged rule reads as it did in 0.7.0's audit (`audit/pubrel-0.7.0.md`), and
this report carries its verdicts.

The auditor wrote #104 and #105, so its review of them is not independent.

## Blocks a release

None.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads** (§4.1;
   `fuzz/fuzz_targets/`). Carried from 0.7.0's finding 1:
   `published::read_object` has no target, and SECURITY.md states the gap.
   Every other entry point the machine lists is reached by the
   `release_json` or `manifest` target.
2. **45 private items have no doc comment** (§9.2). Carried from 0.7.0's
   finding 2, recounted on a fresh build at this commit. `pubkit audit
   prepare` reports 0 because clippy prints no warnings for crates it has
   already checked and cached. Each item predates `v0.6.2`.

## Worth considering

3. **#99 refuses a `release.json` that 0.6.2 accepted** (§9.3). Carried from
   0.7.0's finding 3.
4. **No property tests** (§3.2; `Cargo.toml`). Carried from 0.7.0's finding 4.
5. **Related tools: newer releases exist** (§9.8; `README.md:379-381`).
   Carried from 0.7.0's finding 5 (release-plz 0.4.3, knope 0.23.2,
   cargo-release 1.1.7 against the versions the rows name).
6. **Semicolons joining two ideas** (§10.1). Carried from 0.7.0's finding 6.

## Sections met

- **The release workflow:** the three new pins are each action's tag commit
  (`actions/cache` v6.1.0 `55cc834`, `actions/checkout` v7.0.1 `3d3c42e`),
  and the inputs used (`path`, `key`, `fetch-depth`, `token`) exist in both.
  Pinning by commit rather than by a movable tag is what SonarQube Cloud's
  S7637 asks.
- **§7.1 SECURITY.md:** unchanged. The machine's finding at `SECURITY.md:75`
  misreads "fuzzing of part of what `check` reads from a pull request", as
  0.7.0's report explains. That line is under the merge queue and pushes to
  main, where fuzzing runs.
- **§9.1, §9.3, §9.4, §9.5, §9.2 module docs, §3.3, §3.4 and §9.7, §5 and
  §5.5:** as 0.7.0 found, with no code, README or interface change since.
- **RS-TEST-01:** under a declared exception until 2027-01-12.

## Checks run

| Check | Result |
|---|---|
| `cargo doc -p pubrel --no-deps` | pass |
| `cargo clippy -p pubrel --all-targets` | pass |
| `cargo clippy -p pubrel -- -W clippy::missing_docs_in_private_items`, after `touch src/main.rs` | 45 undocumented private items; see finding 2 |
| `pubrel --help` | exit 0 |
| `cargo publish --dry-run --locked -p pubrel` | pass |
| `cargo +1.88 check --locked -p pubrel` (MSRV) | pass |
| `make supply-chain-check`, `make deny`, `make vet` | pass |

## Checks not run

- **`make fuzz-smoke` and `make functional`:** CI runs them on the release
  pull request. 0.7.0's audit ran `make check` (44 unit tests, 104
  scenarios), and no code changed since.
- **docs.rs build and `cargo test --doc`:** not applicable to a binary-only
  crate (§9).
- **`pubkit audit draft`:** not used. The judged rules were reviewed by
  Claude Code.
