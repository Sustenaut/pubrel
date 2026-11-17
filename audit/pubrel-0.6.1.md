---
package: pubrel
version: 0.6.1
standard: bc3c5b6eeba4757b5d96bc00be2d0a1d3221bef89072b06a271c2b28e2205e34
blocking: 0
date: 2026-10-29
auditor: pubkit audit prepare (pubkit 0.17.1), judged rules reviewed by Claude Code following rust-crate-quality
commit: 5714cb1d1d1a0577c02b5aa594a18a93d9a93d9c
last-release: v0.6.0
---

# Quality audit: pubrel 0.6.1

Audited at `5714cb1` (origin/main, with Sustenaut/pubrel#72 merged)
against the Rust crate standard named above. 0.6.1 is the version `pubrel
next` gives for the one unreleased change, an `added` row for
release.yml's optional release-app secrets. Below 1.0 an addition is a
patch bump. pubrel is a binary-only crate, so section 9 applies. The
library-only rules do not: sections 1 and 2, §3.1 under its declared
exception, and the docs.rs build.

`pubkit audit prepare` wrote the header, ran the checks, decided the rules
a machine can decide, and listed the evidence for the 14 rules that need
judgement. Each of those was judged by reading the code, docs and
workflows, and its finding moved below. Changes since `v0.6.0`: no Rust
code. They are release.yml's optional `RELEASE_APP_ID` and
`RELEASE_APP_KEY` secrets and the README text describing them (#72), and
the move to pubkit 0.17.1 in the generated workflows, `pubkit`,
`pubkit.mk`, AGENTS.md and the pubkit lock. So every judged rule about the
crate's code reads as it did for 0.6.0. This file should be committed with
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
   as "not fuzzed". The other entry points `prepare` lists are covered:
   `manifest::Kind::parse` and `manifest::read` by `manifest`,
   `Config::read_all` and `Config::parse_all` by `release_json`, and
   `version::parse` by the version arithmetic in each target. Adding a
   target for the objects is a code change, for a person to decide.

## Worth considering

2. **SECURITY.md omits two checks CI runs** (§7.1; SECURITY.md:46-75).
   This is carried from 0.5.5. OSV-Scanner runs in pr.yml, main.yml and
   nightly.yml (`job-osv`). The dependency review runs on pull requests
   (pr.yml, `job-review`).

3. **"On every pull request ... CI runs" includes jobs a pull request may
   skip** (§7, §10 "name things exactly"; SECURITY.md:46, 58-63). This is
   carried from 0.5.5. With pubkit 0.17.1, `job-rust-cross.yml` and
   `job-rust-fuzz.yml` still pass at once, with a notice, on a pull
   request that touches no Rust code, Cargo files, `fuzz/` or workflow
   files.

4. **No property tests** (§3.2; Cargo.toml). proptest is not a
   dev-dependency. `record::fragment_name` remains the clearest candidate:
   for any change text, the name is a single path component, ends in
   `.json`, and sorts after an earlier stamp. The fuzz target already
   asserts the first two.

5. **Related tools: newer releases exist** (§9.8; README.md:369-371).
   `cargo search` today gives release-plz 0.4.2 (the row checked 0.3.170),
   knope 0.23.2 (row 0.23.0) and cargo-release 1.1.7 (row 1.1.6). Each row
   names the version checked, so none is wrong. knope's row says "Its
   changeset files are the nearest thing to pubrel's rows". Since 0.6.0,
   pubrel's changes are also one file each, so that comparison understates
   the likeness. This is carried from 0.6.0. cargo-semver-checks 0.51.0
   and publet-core 0.1.4 are current. The crates pubrel builds on match
   Cargo.lock: semver 1.0.28, toml_edit 0.22.27 and serde_json 1.0.154.

6. **Semicolons joining two ideas** (§10, §9.5). `pubkit audit prepare`
   lists 30, such as README.md:53, README.md:224, src/main.rs:7 and
   src/record.rs:44-119. All predate this release: 0.6.0's finding 6 named
   the user-facing ones (src/main.rs:281, src/security.rs:194). The new
   README text for the release app has none.

## Sections met

- **§9.1 README and `--help`:** the README has what the tool is for, its
  commands, why pubrel, related tools, installation, MSRV and versioning.
  No command or flag changed since 0.6.0, so `--help` and the Commands
  table still agree. The CI section now shows how to pass the release
  app's two secrets, and what the workflow does with them and without
  them.
- **§5.5 README:** it has a Documentation line, Using it, an example, MSRV
  and Security.
- **§9.2 module docs:** `cargo clippy -- -W
  clippy::missing_docs_in_private_items` reports none. No module changed.
- **§9.3 versioned interface:** README.md:404-432 names the commands and
  flags, exit codes, script output, files, and the reusable workflow. The
  workflow's new secrets are optional, so no caller breaks, and the change
  is recorded as `added`, which gives 0.6.1. The interface list now names
  the workflow's secrets beside its inputs (README.md:431-432), a
  documentation change made with this report.
- **§9.4 end to end:** 14 feature files, unchanged since 0.6.0. The
  release workflow's app path runs only on a push to main with the secrets
  set, so no suite exercises it before the first release that uses it.
  Its fallback is the path every pull request and push has run.
- **§9.5 usage text:** `--help` exits 0 and prints 27 lines to stdout,
  nothing to stderr, and is unchanged.
- **§3.3 oracles:** no established crate implements Cargo's bump rules as
  pubrel applies them to recorded categories. Versions are parsed by
  `semver` itself, which is used rather than reimplemented.
- **§3.4 and §9.7 `// covers:`:** 39 comments, none added since 0.6.0.
  Spot-checked `api::allowed` (src/api.rs:142), `jsdelivr::urls`
  (src/jsdelivr.rs:134) and `manifest::write, manifest::read`
  (src/manifest.rs:380): each test calls what it names. The comments use
  module paths, as §9 allows.
- **§7.1 SECURITY.md:** it says how to report privately, what is checked
  and when, and what fuzzing found. Each tool it names is run by a
  workflow on the triggers it gives, apart from findings 2 and 3.
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

- **`make fuzz-smoke` and `make functional`:** CI runs them on the release pull request.
- **docs.rs build and `cargo test --doc`:** not applicable to a binary-only crate (§9).
- **`pubkit audit draft`:** not used; the judged rules were reviewed by Claude Code.
