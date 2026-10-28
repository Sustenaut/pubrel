---
package: pubrel
version: 0.5.5
standard: bc3c5b6eeba4757b5d96bc00be2d0a1d3221bef89072b06a271c2b28e2205e34
blocking: 0
date: 2026-10-28
auditor: pubkit audit prepare (pubkit 0.14.6 with schryer/pubkit#90), judged rules reviewed by Claude Code following rust-crate-quality
commit: 62d72b82ee75108fd9d5ac778ddc9df49a18dc65
last-release: v0.5.4
---

# Quality audit: pubrel 0.5.5

Audited at `62d72b8` (origin/main, with schryer/pubrel#59 merged) against
the Rust crate standard named above. 0.5.5 is the version `pubrel next`
gives for the unreleased rows: one `security`, two `added` and one `fixed`,
which below 1.0 make a patch. pubrel is a binary-only crate, so section 9
applies, and the library-only rules (sections 1 and 2, §3.1 under its
declared exception, the docs.rs build) do not.

`pubkit audit prepare` wrote the header, ran the checks, decided the rules
a machine can decide, and listed the evidence for the 14 rules that need
judgement. Each of those was then judged by reading the code, docs and
workflows, and its finding moved below. Changes since `v0.5.4`: `prepare
--all` (`src/order.rs`, `src/main.rs`), the missing-audit hint
(`src/security.rs`), `tag` trying every package, publet-core 0.1.4, and the
CI generated from pubkit.json's `ci` block. This file should be committed
with the release's changes.

## Blocks a release

None.

## Falls short of the standard

1. **No fuzz target for the published objects `check` reads** (§4, judged;
   `fuzz/fuzz_targets/`). This is unchanged since 0.5.4. The targets are
   `corpus_lock`, `manifest`, `release_json` and `unreleased_json`.
   `published::claim_content` (src/published.rs:58),
   `published::kind_and_purpose` (src/published.rs:73),
   `published::read_object` (src/published.rs:127) and the signature
   checks behind `published::require_signed` (src/published.rs:273) parse
   bytes a pull request can change. SECURITY.md states the gap truthfully
   (it lists these objects as "not fuzzed"). The new `order::members` parses
   the JSON `cargo metadata` prints, on the releasing machine, from main,
   so it reads no untrusted input. Adding a target is a code change, for a
   person to decide.

## Worth considering

2. **"On every pull request ... CI runs" includes jobs a pull request may
   skip** (§7, §10 "name things exactly"; SECURITY.md:46, 58-63). The
   cross-target tests and fuzzing pass at once, with a notice, on a pull
   request that touches no Rust code, Cargo files, `fuzz/` or workflow
   files (RS-FUZZ-02, RS-PLAT-01). "On every pull request that changes code"
   would be exact for those two items.

3. **SECURITY.md omits two checks CI runs** (§7.1; SECURITY.md:46-75).
   OSV-Scanner runs in pr.yml, main.yml and nightly.yml (`job-osv`). The
   dependency review runs on pull requests (pr.yml, `job-review`).

4. **No property tests** (§3.2; Cargo.toml). proptest is not a
   dev-dependency. The version arithmetic is fuzzed through the
   `manifest` target and unit-tested in src/version.rs. A round-trip
   property, such as reading back a manifest that `manifest::write` set,
   would cover the editing at random inputs.

5. **Related tools: newer releases exist** (§9.8; README.md:319-321).
   `cargo search` today gives release-plz 0.4.2 (the row checked 0.3.170,
   and below 1.0 a minor bump may change its approach), knope 0.23.2 (row
   0.23.0) and cargo-release 1.1.7 (row 1.1.6). Each row names the version
   checked, so none is wrong. The `.changeset/` directory name for knope is
   still unverified, as in the 0.5.4 audit. cargo-semver-checks 0.51.0 and
   publet-core 0.1.4 are current, and the crates pubrel builds on match
   Cargo.lock (semver 1.0.28, toml_edit 0.22.27, serde_json 1).

6. **Wording of the new text** (§10).
   - README.md:180-182: "The branch is `release/` and the tags joined by
     `+` (...), and the pull request's notes hold each package's under its
     name" carries two ideas. Split it after the example.
   - src/main.rs:560: the error "prepare --all releases every package with
     changes; it takes no --package" joins two ideas with a semicolon. It
     is user-facing text (§9.5).

   `pubkit audit prepare` lists 27 more semicolons in module docs, such as
   README.md:195, src/main.rs:7 and src/record.rs:44-119. They predate this
   release, the 0.5.4 audit did not report them, and none is new text.

## Sections met

- **§9.1 README and `--help`:** the README has what the tool is for, its
  commands, why pubrel, related tools, installation, MSRV and versioning.
  `--help` and the README's Commands table agree, `prepare --all` included
  (README.md:44 and the usage text).
- **§5.5 README:** it has a Documentation line, Using it, an example, MSRV
  and Security.
- **§9.2 module docs:** `cargo clippy -- -W
  clippy::missing_docs_in_private_items` reports none. That was 43 at
  0.5.4. Every module has `//!` docs, the new `order` among them.
- **§9.3 versioned interface:** README.md:354-375 names the commands and
  flags, exit codes, script output and files. The new flag is recorded as
  `added`, and `tag`'s change as `fixed`: it now tries every package
  before failing, still exiting 1 on any failure.
- **§9.4 end to end:** 13 feature files. multi.feature runs `prepare --all`
  through order, the lockfile, check, merge and tag, with one package, with
  none, with `--package` refused, and with a failing check.
- **§9.5 usage text:** `--help` exits 0 and prints 26 lines to stdout,
  nothing to stderr. It is accurate for every command.
- **§3.3 oracles:** no established crate implements Cargo's bump rules as
  pubrel applies them to recorded categories. Versions are parsed by
  `semver` itself, which is used rather than reimplemented.
- **§3.4 and §9.7 `// covers:`:** spot-checked. `order::by_dependency`
  (src/order.rs:198), `order::members` (src/order.rs:241) and
  `security::next_step` (src/security.rs:310) are exercised by the tests
  under them. The comments use module paths, as §9 allows.
- **§7.1 SECURITY.md:** it says how to report privately, what is checked
  and when, and what fuzzing found. Each tool it names is run by a
  workflow on the triggers it gives, apart from finding 2. The imported
  audit sets match `supply-chain/config.toml`. "In every release" names the
  four checks release.json declares.
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
| `cargo publish --dry-run --locked -p pubrel` (with --allow-dirty: the only uncommitted file was this report) | pass |
| `cargo +1.88 check --locked -p pubrel` (MSRV) | pass |
| `make supply-chain-check` (whole repository, after `make venv`) | pass |
| `make deny` (whole repository) | pass |
| `make vet` (whole repository) | pass |
| `cargo search` for the related tools | ran; see finding 5 |

## Checks not run

- **`make fuzz-smoke` and `make functional`:** CI runs them on the release pull request.
- **docs.rs build and `cargo test --doc`:** not applicable to a binary-only crate (§9).
- **`pubkit audit draft`:** not used; the judged rules were reviewed by hand.
