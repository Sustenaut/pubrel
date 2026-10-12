# Security

## Reporting a vulnerability

Report it privately through GitHub:
**[report a vulnerability](https://github.com/schryer/pubrel/security/advisories/new)**
(the repository's Security tab, "Report a vulnerability"). Please don't open
a public issue for it.

Say which version and command are affected, how to reproduce it, and what
an attacker could do. You'll get a reply in the advisory.

## Supported versions

Releases are cut only from `main`: `pubrel prepare` refuses to cut one
from anywhere else. So a fix is released as the next version of pubrel.
Earlier versions get no separate fixes.

## What pubrel checks for you

`pubrel check` and `pubrel tag` read the package publet and verify it with
`publet-core`, without `pub`:

- every object they read must hash to the identifier that names it;
- a release's `identity` and `release` claims must carry a valid Ed25519
  signature by the key `release.json` names, or by a key it rotates from.
  So must its `security` claim when `release.json` declares security
  checks, and its `summary` claim when it has one.

If `release.json` names no `key`, they print a warning and do not check
signatures. The key is trusted because the repository names it, so a change
to `key` is a change to `release.json`, reviewed like any other.

A rotation carries that trust back to an earlier key only when the newer
key's object names it in `rotates` and both keys signed that object for the
purpose `rotation` (publet spec Section 10.5). A key that merely claims to
rotate from the release key, signed by itself alone, gains nothing. At most
64 rotations are followed.

`pubrel prepare` runs the commands that `release.json`'s `security` section
declares, with `sh -c`, on the machine of whoever cuts the release. Treat
`release.json` as code. `check` and `tag`, which run in CI, do not run those
commands.

## What is checked in pubrel itself

**On every pull request and push to `main`, CI runs:**

- `cargo fmt --check`;
- `cargo clippy` with the `all` and `pedantic` groups, `unwrap_used`,
  `expect_used` and `panic`, and warnings as errors;
- `cargo doc`, with warnings as errors;
- the unit tests, on Linux, macOS and Windows;
- the Gherkin suite, which runs the built binary against throwaway
  repositories;
- on a pull request, `pubrel check`, through pubrel's own release
  workflow built from the checkout.
- the tests on a 32-bit (i686) and a big-endian (s390x) target, under
  emulation, and a build on the minimum Rust version;
- fuzzing of what `check` reads from a pull request: `release.json`,
  `unreleased.json`, the corpus lock and the manifests, with the version
  arithmetic (`fuzz/`). Every input that once found a bug is replayed
  first (`fuzz/regressions/`).

- `cargo deny check`: fails on any crate in the dependency tree with a
  RustSec advisory (vulnerable, unmaintained, unsound, or yanked), on a
  licence outside the permissive list, and on any source but crates.io
  (`deny.toml`).
- `cargo vet`: every dependency is covered by an audit imported from
  Mozilla, Google, the Bytecode Alliance, Zcash, ISRG or Embark Studios, by
  a publisher those organisations trust, or by an exemption that records
  the evidence that does exist (`supply-chain/`). publet-core is trusted as
  this project's own crate.

**In the package:** [`SUPPLY-CHAIN.md`](SUPPLY-CHAIN.md) lists every crate
pubrel builds, for any target, whether it runs code at build time, and how
it is vouched for. It is generated, and CI fails if it is stale.

**In every release:** `pubrel prepare` runs `cargo deny`, `cargo vet` and
the report's check before cutting a release, and records them as a signed
`security` claim in pubrel's package publet: each command, its tool and
version, and its summary, beside the RustSec database revision and the
audit sets imported. The changelog and the GitHub release list it.

**Fuzzing found** one bug before pubrel was published: bumping a version
whose part to bump was already `u64::MAX` panicked, where it should
refuse. Overflow is checked in pubrel's release builds, so a pull request
could make `check` crash. It is fixed, with a regression test.

**In the code:** `unsafe` is forbidden (`unsafe_code = "forbid"` in
`Cargo.toml`). Release builds keep integer overflow checks
(`overflow-checks = true`).
