---
package: pubrel
version: 0.6.2
standard: bc3c5b6eeba4757b5d96bc00be2d0a1d3221bef89072b06a271c2b28e2205e34
blocking: 0
date: 2026-11-03
auditor: pubkit audit prepare (pubkit 0.24.0), judged rules reviewed by Claude Code following rust-crate-quality
commit: 532cd2e10b2f6075e3989b303f84e86c044f48f0
last-release: v0.6.1
---

# Quality audit: pubrel 0.6.2

Audited against the Rust crate standard named above, on main with
Sustenaut/pubrel#86 merged. 0.6.2 is the version `pubrel next` gives for the
one unreleased change, an `added` row: `prepare` drives `pub` from 0.1 up to
0.3, where it stopped below 0.3. Below 1.0 an addition is a patch bump. pubrel
is a binary-only crate, so section 9 applies.

`pubkit audit prepare` wrote the header, ran the checks, decided the rules a
machine can decide, and listed the evidence for the 14 rules that need
judgement. Each was judged by reading the code, docs and workflows. The only
Rust change since `v0.6.1` is #86, in `src/record.rs`: the accepted range of
`pub`, its documentation and refusal message, and a unit test. The rest is
generated pubkit files, CI workflows, `integrations.json`, and the move of
`pub` in CI to `v0.3.0`. So every judged rule about the code reads as it did
for 0.6.1.

The auditor wrote #86, so its review of that change is not independent.

## Blocks a release

None.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads** (§4;
   `fuzz/fuzz_targets/`). Unchanged since 0.5.4, carried from 0.6.1's
   finding 1. SECURITY.md states the gap.

## Resolved on this branch

- **R1. Was false: SECURITY.md said fuzzing, the cross-target tests and the
  minimum Rust version build run on every pull request** (§7.1, the machine's
  finding at `SECURITY.md:60`). Since the move to pubkit's merge queue they run
  in `queue.yml`, `main.yml` and `nightly.yml`, and `pr.yml` runs neither
  `job-rust-fuzz` nor `job-rust-cross` nor `job-rust-msrv`. SECURITY.md now
  lists them under the merge queue and pushes to main. This also resolves
  0.6.1's finding 3.
- **R2. Fell short: SECURITY.md omitted OSV-Scanner and the dependency review**
  (§7.1, carried from 0.6.1's finding 2). Both are now listed, as the
  workflows run them: `job-osv` in all four, `job-review` in `pr.yml`.

## Worth considering

2. **pubrel reads objects with publet-core 0.1.4** (`Cargo.toml:23`). Its
   decoder refuses objects over 64 KiB, where publet-core 0.2.0, released on
   2026-11-02, accepts up to 1 MiB. pubrel reads the package publet's own
   objects, which are small, so nothing it reads today is affected. Moving to
   0.2.0 is a dependency change for its own pull request.
3. **No property tests** (§3.2; `Cargo.toml`). Carried from 0.6.1's finding 4.
4. **Related tools: newer releases exist** (§9.8; README.md:369-371).
   `cargo search` today gives release-plz 0.4.3 (row 0.3.170), knope 0.23.2
   (row 0.23.0) and cargo-release 1.1.7 (row 1.1.6). Each row names the
   version checked, so none is wrong. The crates pubrel builds on match
   Cargo.lock: semver 1.0.28, toml_edit 0.22.27, serde_json 1.0.154. The
   publet-core row names 0.1.4, which is what pubrel uses.
5. **Semicolons joining two ideas** (§10). Carried from 0.6.1's finding 6. #86
   adds none.

## Sections met

- **§9.1 README and `--help`:** no command or flag changed. The README's one
  change says which `pub` releases `prepare` drives (0.1 to 0.3).
- **§9.3 versioned interface:** `prepare` now accepts more releases of `pub`,
  and refuses none it accepted before, so no caller breaks. Recorded as
  `added`.
- **§9.4 end to end:** 14 feature files. CI now runs them against `pub`
  v0.3.0, and they pass locally against the released v0.3.0 (104 scenarios).
  The 0.2 path is no longer run by the suite.
- **§9.5 usage text:** `--help` exits 0.
- **§9.2 module docs:** `src/record.rs`'s module docs give the new range.
- **§3.3 oracles:** versions are parsed by `semver` itself.
- **§3.4 and §9.7 `// covers:`:** 40 comments, one added by #86
  (`require_pub`). Its test checks the range `require_pub` applies, not the
  running of `pub`, so the claim is narrower than the name suggests.
- **§5 packaging:** as 0.6.1 found.
- **RS-TEST-01:** under a declared exception until 2027-01-12.

## Checks run

| Check | Result |
|---|---|
| `cargo doc -p pubrel --no-deps` | pass |
| `cargo clippy -p pubrel --all-targets` | pass |
| `cargo clippy -p pubrel -- -W clippy::missing_docs_in_private_items` | 0 undocumented private item(s) |
| `pubrel --help` | exit 0 |
| `cargo publish --dry-run --locked -p pubrel` | pass |
| `cargo +1.88 check --locked -p pubrel` (MSRV) | pass |
| `make supply-chain-check`, `make deny`, `make vet` | pass |
| `make check` against the released `pub` v0.3.0, on #86 | pass: 43 unit tests, 104 scenarios |
| `cargo search` for the related tools | ran; see finding 4 |

## Checks not run

- **`make fuzz-smoke`:** CI runs it in the merge queue and on main.
- **docs.rs build and `cargo test --doc`:** not applicable to a binary-only crate (§9).
- **`pubkit audit draft`:** not used; the judged rules were reviewed by Claude Code.
