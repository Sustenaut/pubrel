# pubrel

Version a package by what changed, and record each release as a version of
a publet.

A release of a package is a version of its **package publet**: `pkg.<name>`
in the package's own corpus. Its `identity` claim states the version, the
`vX.Y.Z` tag, and the commit. Its `release` claim lists what changed. The
publet's lineage is the release history, and `CHANGELOG.md` is generated
from it. The same tool and workflow release a Rust crate, a Cargo
workspace, or a Python package.

**Documentation:** this README and `pubrel --help` document the tool: its
commands, its files, and the interface it keeps stable.
[`CHANGELOG.md`](CHANGELOG.md) lists every release.
[`SECURITY.md`](SECURITY.md) says how to report a vulnerability and what is
checked.

## Using it

```sh
pubrel init                                  # release.json and an empty unreleased list
pubrel add added "a new subcommand"          # every PR that changes code records a row
pubrel summary "Faster checks, and ..."      # optional: a paragraph opening the release
pubrel next                                  # the version the rows imply
pubrel prepare                               # on a clean main: cut a release PR
```

`pubrel prepare` moves the unreleased rows into the package publet, sets the
manifest version, publishes the package publet with the corpus's key,
regenerates `CHANGELOG.md`, and opens a `release/vX.Y.Z` pull request.
Merging it is the release: CI tags it.

### Commands

| Command | What it does |
|---|---|
| `init` | Writes `release.json` and an empty `unreleased.json`. |
| `add CATEGORY CHANGE...` | Records a change under one of `changed`, `removed`, `added`, `fixed`, `security`, `internal`. |
| `summary [TEXT...]` | Sets the next release's summary. With no text, prints it. |
| `next` | Prints the version the unreleased changes imply. |
| `check BASE` | For CI on a pull request: a change to a `code` path must record a row. A release must be exactly the bump its changes require, published, and signed by `key` when `release.json` names one. |
| `prepare [--no-pr]` | Cuts a release: publishes the package publet, commits on a `release/` branch, and opens a pull request. `--no-pr` stops after the commit. |
| `withdraw PR\|BRANCH` | After a release's pull request is closed unmerged: keeps `pub` from publishing its objects again. See [Withdrawing a release](#withdrawing-a-release). |
| `changelog` | Regenerates each package's changelog from its package publet. |
| `tag` | For CI on main: for each package whose manifest version is published and not yet tagged, creates the tag and the GitHub release, then refreshes jsDelivr for a package that lists `jsdelivr` files. |
| `--version` (or `version`) | Prints `pubrel X.Y.Z`. |
| `--help` | Prints the usage. |

When `release.json` lists several packages, `--package NAME` (or
`--package=NAME`) names the one that `add`, `summary` and `prepare` act on.
`next` without it reports every package. `pubrel --help` (or `-h`, or
`help`) prints the usage to stdout. An unknown command prints it to
stderr, and fails.

## The rules

The version follows mechanically from the categories of what changed since
the last release, under Cargo's flavour of semantic versioning, the one
`cargo` resolves `^` requirements by:

| Category | Meaning | From 1.0.0 | Below 1.0.0 |
|---|---|---|---|
| `changed`, `removed` | an already-published interface changed: commands, flags, output, exit codes, file formats, object shapes | major | minor |
| `added` | a new feature | minor | patch |
| `fixed`, `security` | the same functionality | patch | patch |
| `internal` | code changed, nothing users run did: a refactor, a crate moved | none | none |

Below 1.0.0, as Cargo reads `0.y.z`, `y` is the breaking part: `0.3.1` to
`0.4.0` breaks, and `0.3.1` to `0.3.2` does not, whatever it adds. So a
dependent's `^0.3` requirement takes every compatible release and no
breaking one.

A package never released before is first released at its manifest version,
raised to at least 0.1.0. `internal` changes never make a release on their
own. They wait for the next release something else makes, and are listed
in it.

### Withdrawing a release

A release cut by `prepare` but never merged must be withdrawn before the
next one is cut. `prepare` publishes into `pub`'s local store
(`corpus/.publet/`). Closing the release's pull request removes its objects
from git, but not from the store, and `pub` exports what it holds. So the
next release would carry the abandoned one's records too, permanently.

```sh
gh pr close 42                  # the release pull request, unmerged
pubrel withdraw 42              # or the release branch: pubrel withdraw release/v1.2.3
pubrel prepare                  # from main, as usual
```

`pubrel withdraw` lists every object that release added in
`corpus/.draft-discards`, which `pub` never exports, except key objects and
the signatures that rotate a key: those are the corpus's, not the
release's, and the first release is the one that exports its key. It keeps that file out
of git through the clone's own exclude file. It refuses a release that is
already merged, and a branch that adds no release record. Given a pull
request number, it asks `gh` for its state: an open pull request must be
closed first. It then deletes the release branch from `origin` and from
this clone, unless it is a fork's, so the next release can use the name. As a backstop,
`prepare` and `check` refuse a release that carries records naming any
other commit, and `prepare` says so when a release cut again unchanged
reproduces a withdrawn object.

A release may open with a summary, set by `pubrel summary`: a paragraph,
in prose, of what the release is about. It is published as a `summary`
claim, signed with the release, and the changelog and GitHub release show
it above the list of changes. Recording a change keeps it, and `prepare`
moves it into the release with the rows. A release without a summary is not held
to an earlier release's: `check` and `tag` require the summary signed only
when it names the version being released.

## release.json

`release.json` at the repository root says where everything is:

```json
{ "name": "publet-cli", "package": "pkg.publet-cli", "tag": "PKG-PUBCLI-10-2026",
  "corpus": "corpus",
  "manifest": { "kind": "cargo-workspace", "path": "Cargo.toml" },
  "code": ["crates/", "bin/", "porcelain/", "Cargo.toml", "Cargo.lock"],
  "command": { "name": "pub", "check": "pub --version" },
  "install": "cargo install --locked --git https://github.com/schryer/publet --tag {tag} publet-cli",
  "key": "pub:sha2-256:m2smuezcqpjeyy6iofa2pjl47lgbwu254hohndyhs2do34fcrcma" }
```

`key` is the key a release must be signed by: the corpus's author, which
`pubrel init` records. `check` and `tag` verify that the package publet's
`identity` and `release` claims carry a valid signature by it, and that every
object they read hashes to its identifier. Without `key` they warn and do
not check signatures.

A key is replaced by rotating it (`pub sign --rotate`, publet spec Section
10.5): a new key object names the old one in `rotates`, and both keys sign
it for the purpose `rotation`. `release.json` then names the new key, and
`check` and `tag` also accept a signature by any key it rotates from, through
a chain of such rotations, so earlier releases still verify. A rotation that
only one key signed is not followed.

A repository that releases more than one package lists them under
`packages`. `corpus` and `key` at the top are shared:

```json
{ "corpus": "corpus", "key": "pub:sha2-256:…",
  "packages": [
    { "name": "publet-cli", "package": "pkg.publet-cli", "tag": "PKG-PUBCLI-10-2026",
      "manifest": { "kind": "cargo-workspace", "path": "Cargo.toml" },
      "code": ["crates/", "bin/", "porcelain/", "Cargo.toml", "Cargo.lock"] },
    { "name": "publet-core", "package": "pkg.publet-core", "tag": "PKG-PUBCOR-10-2026",
      "manifest": { "kind": "cargo-package", "path": "crates/publet-core/Cargo.toml" },
      "code": ["crates/publet-core/"] } ] }
```

Each package has its own unreleased list, package publet, changelog (beside
its manifest: `crates/publet-core/CHANGELOG.md`) and tag prefix
(`publet-core-v0.1.1`). The root package keeps `v`, and `tag-prefix` and
`changelog` override both. `check`, `changelog` and `tag` act on every
package. A changed path belongs to every package whose `code` covers it, so
a change to a crate the workspace also ships is recorded in both lists. A
workspace bump never moves the pin of a crate released on its own, even
when their versions coincide. That crate's own release pins its new version
in the workspace.

`install`, optional, is how a release is installed: release notes show it,
with `{tag}` replaced by the release's git tag and `{version}` by its
version.

Manifest kinds: `cargo-workspace` (the workspace version and every internal
path dependency pinned at it), `cargo-package`, `pyproject`, and `npm` (the
top-level `version` of a `package.json`, and the lockfile's own two copies
of it when a `package-lock.json` sits beside it; only those bytes change,
and npm is not run). `command` is optional, and a library has none.

### Served through jsDelivr

[jsDelivr](https://www.jsdelivr.com/) serves every git tag of a public
GitHub repository at `cdn.jsdelivr.net/gh/OWNER/REPO@TAG/PATH` as soon as
the tag exists, so a release needs nothing published there. A package
whose files are loaded that way may list them:

```json
"jsdelivr": { "files": ["app/app.js", "app/style.css"] }
```

After tagging, `pubrel tag` then asks jsDelivr for each file at the new
tag, so the first visitor does not wait while jsDelivr fetches it from
GitHub, and purges the URLs whose version range now resolves to the new
release (`@1`, `@1.2`, and no version). Ranges resolve only against `vX.Y.Z`
tags, so a package with another tag prefix is warmed and not purged. The
repository is `GITHUB_REPOSITORY` in Actions, or what `gh repo view` says.
Either step failing is a warning: the release is already made, and its
files are served all the same. Each path must be a plain path inside the
repository.

## Security checks

`release.json` may declare checks a release must pass, and facts to keep
beside them:

```json
"security": {
  "checks": [
    { "name": "advisories", "run": "cargo deny --locked check", "version": "cargo deny --version" },
    { "name": "supply chain", "run": "cargo vet --locked", "version": "cargo vet --version" }
  ],
  "facts": [
    { "name": "RustSec advisory database", "run": "git -C \"$(ls -d ~/.cargo/advisory-dbs/*/ | head -1)\" rev-parse HEAD" }
  ]
}
```

`pubrel prepare` runs every check before it changes anything, and cuts no
release if one fails. Each check and fact runs with the release it is
checking in its environment, `PUBREL_PACKAGE` (the package's name) and
`PUBREL_VERSION`, so a check can judge that release: `pubkit audit --check`
finds the quality audit report for it this way. It records what ran in a `security` claim in the
package publet: each check's command, its tool and version, and the last
line the tool printed, beside each fact's value. The claim is signed with
the release, and the changelog and the GitHub release list it. `check` and
`tag` refuse a release whose record is missing or unsigned. What was
checked for a version is as verifiable as the version itself.

## API checks

A crate released on its own may set `"semver-checks": true` in its
`release.json` entry. Its public API is then compared with the one at its
last release's tag by
[`cargo-semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks),
and may move only as far as the changes recorded since allow:

| recorded | the API may | level |
|---|---|---|
| `changed` or `removed` | break | major |
| `added` | grow, not break | minor |
| anything else | stay as it is | patch |

The level is what the change *is*, not the version it makes: below 1.0 a
break bumps the minor version, and is still a break. So a removed function
recorded as `fixed` fails `check` on its pull request, and again in
`prepare`, with the tool's findings and the `pubrel add` that would record
it. `check` runs it whenever the crate's code changes, and on a release
pull request. A crate never released has nothing to compare against, and
`check` says so.

The baseline is a git tag, so CI needs the tags (`fetch-depth: 0`, as the
shared workflow checks out) and the tool: pass `semver-checks-version`.

## CI

Every package calls the same workflow:

```yaml
release:
  uses: schryer/pubrel/.github/workflows/release.yml@vX.Y.Z
  with: { pubrel-version: vX.Y.Z }   # add semver-checks-version: 0.51.0 for API checks
  permissions: { contents: write }
```

On a pull request it runs `pubrel check`. On main it runs `pubrel tag`,
which tags a published version and creates the GitHub release. CI never
signs anything: a release is signed when its package publet is published,
by whoever cut it.

## Why pubrel

- **The version is decided when a change lands, not at release time.** The
  pull request that makes a change records its category with `pubrel add`.
  `pubrel check` fails a pull request that changes a `code` path without a
  row. It also fails a release pull request whose version is not exactly
  the bump its rows require. Nobody chooses a version, and no commit
  message is parsed for one.
- **The release record is signed and checked.** Each release is a version
  of the package publet, signed with the corpus's key by whoever cuts it.
  `check` and `tag` verify that signature, and the hash of every object
  they read, with `publet-core`. They do it in CI, where no signing key is
  present.
- **What was checked is part of the release.** Declared security checks
  run before a release is cut, and their record is signed with it.
- **One tool for Rust, Python and npm.** The same commands and workflow
  release a Cargo package, a Cargo workspace with crates versioned on
  their own, a `pyproject.toml` package, and a `package.json` package.

## Related tools

Each was checked with `cargo info` at the version given. Its approach was
read from its own source or README.

| Tool | Version checked | How it picks a version | How pubrel differs |
|---|---|---|---|
| [release-plz](https://crates.io/crates/release-plz) | 0.3.170 | From conventional-commit messages (its crates.io description). It depends on `git-cliff-core`. | pubrel reads an explicit category row per change, not commit messages. |
| [knope](https://crates.io/crates/knope) | 0.23.0 | From conventional commits and changeset files in `.changeset/` (its README and source). | Its changeset files are the nearest thing to pubrel's rows. pubrel also signs the release record and checks the signature in CI. |
| [cargo-release](https://crates.io/crates/cargo-release) | 1.1.6 | The person releasing names it: `cargo release LEVEL\|VERSION`. | pubrel derives the version from recorded changes, and CI checks it. |

pubrel builds on these rather than reimplementing them:

| Crate | Version | What pubrel uses it for |
|---|---|---|
| [cargo-semver-checks](https://crates.io/crates/cargo-semver-checks) | 0.51.0 | Run as a tool, to compare a crate's API with its last release. |
| [`publet-core`](https://crates.io/crates/publet-core) | 0.1.4 | Canonical CBOR, content identifiers and Ed25519, to read the package publet back and verify its signatures. |
| [`semver`](https://crates.io/crates/semver) | 1 | Versions, and the range of `pub` releases it drives. |
| [`toml_edit`](https://crates.io/crates/toml_edit) | 0.22 | Setting a manifest's version, keeping its comments and layout. |
| [`serde_json`](https://crates.io/crates/serde_json) | 1 | `release.json`, `unreleased.json` and the corpus lock. |

## Installation

```sh
cargo install --locked pubrel
```

Each GitHub release also names its own version
(`cargo install --locked pubrel --version X.Y.Z`).

Cutting a release (`pubrel prepare`) drives a released `pub` (0.1 or 0.2) to
build and publish the package publet, which signs it. That happens on the
machine of whoever cuts it:

```sh
cargo install --locked --git https://github.com/schryer/publet --tag v0.2.0 publet-cli
```

Everything else (`next`, `check`, `tag`, `changelog`) reads the package
publet and verifies its signatures itself, with `publet-core`. So CI needs
only `pubrel`, `git`, and `gh` for `tag`.

## Versioning

pubrel's own releases follow Cargo's convention, through pubrel itself.
From 1.0, a breaking change bumps the major version, an addition the minor
version, and a fix the patch version. Below 1.0 each shifts one place: at
`0.y.z`, a break bumps `y` and anything compatible bumps `z`.

pubrel has no library API, so cargo-semver-checks does not apply to it.
Its versioned interface is what users and scripts depend on. A change to
any of the following is recorded as `changed` or `removed`:

- **Commands and flags:** those in the table under [Commands](#commands).
- **Exit codes:** 0 on success, and 1 on any failure, including an unknown
  command. Error messages go to stderr.
- **Output meant for scripts:**
  - `pubrel --version` prints `pubrel X.Y.Z`. The release workflow matches
    that line.
  - `pubrel next` prints the version alone. When `release.json` lists
    several packages and no `--package` is given, it prints one line per
    package: `NAME VERSION`, or `NAME: ` followed by why there is nothing
    to release.

  All other output is for people.
- **Files it reads and writes:** `release.json`, each package's
  `unreleased.json`, the claims it publishes in the package publet (which
  later versions read back), and the generated `CHANGELOG.md`.
- **The reusable workflow** `.github/workflows/release.yml` and its
  inputs.

## Minimum Rust version

pubrel builds on Rust 1.88 (`rust-version` in `Cargo.toml`), the first
release where `let` chains are stable. CI builds it there, and every
release from 1.88 to 1.99 was checked when it was set. The repository
develops on the Rust that `rust-toolchain.toml` pins, and a scheduled run
tests the latest stable and beta. The minimum may rise in a release that
bumps the minor version, never in a patch release.

## Security

Report a vulnerability privately, through the repository's Security tab
("Report a vulnerability"). [`SECURITY.md`](SECURITY.md) says how, and
describes what is checked and what is not.
[`SUPPLY-CHAIN.md`](SUPPLY-CHAIN.md) lists every crate pubrel builds, and
how each is vouched for.

## Developing

```sh
make venv          # hash-locked Python environment for the functional suite
make check         # fmt, clippy, unit tests, docs, and the Gherkin suite
```

Testing is split as in publet. Rust unit tests cover invariants with no
user-visible surface, such as the bump arithmetic and manifest editing.
A Gherkin suite (`tests/features`) covers the behaviour of every command.
It drives the built binary against throwaway
repositories with a real `pub` and a fake `gh`. Binaries
are resolved from `PUBREL_BIN_DIR` and `PUB_BIN_DIR`. The shared testing
setup is [pubkit](https://github.com/schryer/pubkit)'s: the plugin those
fixtures and the common steps come from, `pubkit.mk`, the hash-locked pins,
and the CI workflow. `make sync-check` fails if the managed copies drift.

Part of what `pubrel check` reads from a pull request is fuzzed (`fuzz/`):
`release.json`, `unreleased.json`, the corpus lock, and the manifests with
the version arithmetic. On a release pull request, `check` also reads the
package publet's objects from `corpus/objects/` and verifies their
signatures. That code is covered by the Gherkin suite and unit tests, and
is not fuzzed. `make fuzz-smoke` replays every input that once
found a bug (`fuzz/regressions/`), then fuzzes each target for a minute.
It needs nightly Rust and cargo-fuzz.
pubrel's own CI calls its own release workflow from the checkout, so it is
held to the rules it provides.

## Licence

Apache-2.0: see [`LICENSE`](LICENSE).
