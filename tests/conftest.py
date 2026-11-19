"""Fixtures for pubrel's functional suite, on pubtest's steps.

Every scenario drives the built `pubrel` as a user would, through pubtest's
`I run`, in pubtest's scratch directory: a throwaway git repository with a
bare "remote", a real corpus and key made by a real released `pub`, and
pubtest's stand-in `gh`, which records what it was asked to do -- so
nothing here touches GitHub, and nothing imports the Rust.

pubrel is pubtest's program under test, from PUBREL_BIN_DIR, and `pub` is
found on PUB_BIN_DIR, so another implementation runs this suite by setting
one variable. What judges a result is pubtest's steps, and the domain
Thens in steps/test_pubrel.py where pubtest has none.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path

import pytest
from pubtest.steps.standins import standins

# What `gh` answers when no scenario says otherwise: a pull request's URL.
GH_ANSWER = "https://example.org/fake/1\n"

MANIFESTS = {
    "cargo": ("cargo-package", "Cargo.toml",
              '[package]\nname = "demo"\nversion = "{v}"\nedition = "2024"\n',
              ["src/", "Cargo.toml"], "src/lib.rs"),
    "cargo-workspace": ("cargo-workspace", "Cargo.toml",
                        '[workspace]\nmembers = ["crates/*"]\n\n[workspace.package]\n'
                        'version = "{v}"\nedition = "2024"\n\n[workspace.dependencies]\n'
                        'core = {{ path = "crates/core", version = "{v}" }}\n',
                        ["crates/", "Cargo.toml"], "crates/core/src/lib.rs"),
    # A workspace and a crate in it released with a version of its own:
    # both start at the same version, so a workspace bump that moved the
    # crate's pin along with its own would go unnoticed otherwise.
    "multi": ("cargo-workspace", "Cargo.toml",
              '[workspace]\nmembers = ["crates/*"]\n\n[workspace.package]\n'
              'version = "{v}"\nedition = "2024"\n\n[workspace.dependencies]\n'
              'core = {{ path = "crates/core", version = "{v}" }}\n'
              'algo = {{ path = "crates/algo", version = "{v}" }}\n',
              ["crates/", "Cargo.toml"], "crates/core/src/lib.rs"),
    "pyproject": ("pyproject", "pyproject.toml",
                  '[project]\nname = "demo"\nversion = "{v}"\n',
                  ["src/", "pyproject.toml"], "src/demo/__init__.py"),
    "npm": ("npm", "package.json",
            '{{\n  "name": "demo",\n  "version": "{v}",\n  "type": "module"\n}}\n',
            ["src/", "package.json"], "src/index.js"),
}


def unreleased(root: Path, package: str = "pkg.demo") -> list[dict]:
    """The changes a package has not released, as pubrel reads them: a
    pubrel 0.5 unreleased.json's rows, then each file of the unreleased
    directory in name order."""
    publet = root / "corpus" / "publets" / package
    legacy = publet / "unreleased.json"
    rows = json.loads(legacy.read_text())["changes"] if legacy.exists() else []
    return rows + [json.loads(p.read_text()) for p in sorted((publet / "unreleased").glob("*.json"))]


class Package:
    """A package repository under test."""

    def __init__(self, root: Path, tmp: Path, kind: str, version: str, pubrel: Path, pub_dir: Path,
                 pubtest_ctx, monkeypatch):
        self.tmp = tmp
        self.kind = kind
        self.remote = tmp / "remote.git"
        self.root = root
        self.pubrel = pubrel
        self.pubtest_ctx = pubtest_ctx
        self.monkeypatch = monkeypatch
        # pubtest's stand-ins first on PATH, then pub; pubtest puts PATH back
        # when the scenario ends.
        self.standins = standins(pubtest_ctx, tmp)
        self.standins.add("gh", out=GH_ANSWER)
        os.environ["PATH"] = f"{self.standins.bin}{os.pathsep}{pub_dir}{os.pathsep}{os.environ['PATH']}"
        for name, val in (("GIT_AUTHOR_NAME", "Test"), ("GIT_AUTHOR_EMAIL", "test@example.org"),
                          ("GIT_COMMITTER_NAME", "Test"), ("GIT_COMMITTER_EMAIL", "test@example.org")):
            monkeypatch.setenv(name, val)
        monkeypatch.delenv("PUB", raising=False)

        manifest_kind, manifest, template, code, source = MANIFESTS[kind]
        self.manifest = manifest
        self.source = source
        self.git("init", "-q", "-b", "main")
        (self.root / manifest).write_text(template.format(v=version))
        (self.root / source).parent.mkdir(parents=True, exist_ok=True)
        (self.root / source).write_text("// demo\n")
        demo = {
            "name": "demo", "package": "pkg.demo", "tag": "PKG-DEMOPK-10-2026",
            "manifest": {"kind": manifest_kind, "path": manifest}, "code": code,
        }
        if kind == "multi":
            algo = self.root / "crates" / "algo"
            (algo / "src").mkdir(parents=True)
            (algo / "src" / "lib.rs").write_text("// algo\n")
            (algo / "Cargo.toml").write_text(
                f'[package]\nname = "demo-algo"\nversion = "{version}"\nedition = "2024"\n')
            config = {"corpus": "corpus", "packages": [demo, {
                "name": "demo-algo", "package": "pkg.demo-algo", "tag": "PKG-DEMOAL-10-2026",
                "manifest": {"kind": "cargo-package", "path": "crates/algo/Cargo.toml"},
                "code": ["crates/algo/"],
            }]}
        else:
            config = {**demo, "corpus": "corpus"}
        (self.root / "release.json").write_text(json.dumps(config, indent=2))
        (self.root / ".gitignore").write_text("/corpus/.publet/\n")
        corpus = self.root / "corpus"
        corpus.mkdir()
        self.pub("corpus", "init", "--name=demo", cwd=corpus)
        key = self.pub("sign", "--generate-key", "--principal=human", "--label=Test",
                       cwd=corpus).stdout.split()[1]
        self.pub("policy", f"--root={key}", cwd=corpus)
        # Releases must be signed by the corpus's author, as `pubrel init`
        # records it.
        config = json.loads((self.root / "release.json").read_text())
        config["key"] = self.author()
        (self.root / "release.json").write_text(json.dumps(config, indent=2))
        subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(self.remote)], check=True)
        self.git("remote", "add", "origin", str(self.remote))
        self.commit("initial")
        self.git("push", "-q", "-u", "origin", "main")

    def author(self) -> str:
        config = (self.root / "corpus" / ".publet" / "config").read_text()
        return next(line.split("=", 1)[1] for line in config.splitlines()
                    if line.startswith("author="))

    def without_pub(self):
        """Make `pub` unrunnable: a stand-in `pub` first on the PATH that
        fails, and $PUB naming nothing."""
        self.standins.add("pub", err="pub is not available here\n", code=127)
        self.monkeypatch.setenv("PUB", str(self.tmp / "no-such-pub"))

    def objects(self) -> Path:
        return self.root / "corpus" / "objects"

    def locked(self, slug: str) -> str:
        lock = json.loads((self.root / "corpus" / "corpus.lock").read_text())
        return next(r["cid"] for r in lock["publets"] if r["slug"] == slug)

    def object_file(self, cid: str) -> Path:
        return self.objects() / f"{cid.replace(':', '_')}.cbor"

    # --- running things ---------------------------------------------------

    def run(self, *argv: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
        return subprocess.run(list(argv), cwd=cwd or self.root, env=dict(os.environ),
                              capture_output=True, text=True, check=False)

    def ok(self, *argv: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
        proc = self.run(*argv, cwd=cwd)
        assert proc.returncode == 0, f"{argv}: {proc.stdout}{proc.stderr}"
        return proc

    def git(self, *args: str) -> str:
        return self.ok("git", *args).stdout.strip()

    def pub(self, *args: str, cwd: Path | None = None):
        return self.ok("pub", *args, cwd=cwd)

    def commit(self, message: str):
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)

    # --- shaping the history ---------------------------------------------

    def add(self, category: str, change: str, package: str | None = None):
        named = ["--package", package] if package else []
        self.ok(str(self.pubrel), "add", category, change, *named)

    def cut(self, package: str | None = None) -> str:
        """Cut a release on its branch, as `prepare` does; return the branch."""
        named = ["--package", package] if package else []
        done = self.ok(str(self.pubrel), "prepare", "--no-pr", *named)
        # prepare cuts it in a worktree of its own and leaves this checkout
        # where it was: the branch it committed on, checked out here.
        self.last_branch = re.search(r"committed on (\S+);", done.stdout).group(1)
        self.git("checkout", "-q", self.last_branch)
        return self.last_branch

    def release(self, package: str | None = None):
        """Cut a release and merge it, as merging its PR would."""
        branch = self.cut(package)
        self.git("checkout", "-q", "main")
        self.git("merge", "-q", "--ff-only", branch)
        self.git("push", "-q", "origin", "main")

    # --- reading back -----------------------------------------------------

    def unreleased(self, package: str = "pkg.demo") -> list[dict]:
        return unreleased(self.root, package)

    def manifest_version(self, manifest: str | None = None) -> str:
        text = (self.root / (manifest or self.manifest)).read_text()
        return re.search(r'^\s*"?version"?\s*[=:]\s*"([^"]+)"', text, re.M).group(1)

    def pinned(self, path: str) -> str:
        text = (self.root / "Cargo.toml").read_text()
        return re.search(rf'path = "{re.escape(path)}", version = "([^"]+)"', text).group(1)

    def published(self, slug: str) -> dict:
        lock = json.loads((self.root / "corpus" / "corpus.lock").read_text())
        cid = next(r["cid"] for r in lock["publets"] if r["slug"] == slug)
        path = self.root / "corpus" / "objects" / (cid.replace(":", "_") + ".cbor")
        return json.loads(self.pub("read", "--json", str(path)).stdout)["object"]

    def gh_calls(self) -> list[list[str]]:
        return self.standins.runs("gh")


def checkout_cut(pubtest_ctx, code: int, stdout: str) -> None:
    """prepare cuts the release in a worktree and leaves this checkout where
    it was: the branch it cut, checked out here, for the steps that read it,
    unless the scenario reads the checkout as prepare left it."""
    cut = re.search(r"^on (\S+), in a worktree", stdout, re.M)
    if code == 0 and cut and not getattr(pubtest_ctx, "keep_checkout", False):
        subprocess.run(["git", "checkout", "-q", cut.group(1)], cwd=pubtest_ctx.directory, check=True)


def pytest_bdd_after_step(request, feature, scenario, step, step_func, step_func_args):
    """After pubtest's `I run "pubrel ..."`, the cut branch as above."""
    if step.type == "when" and re.match(r'I run "pubrel\s', step.name):
        ctx = request.getfixturevalue("pubtest_ctx")
        checkout_cut(ctx, ctx.outcome.code, ctx.outcome.stdout)


@pytest.fixture
def make_package(pubtest_ctx, tmp_path: Path, binary, monkeypatch):
    """A package repository in the scenario's scratch directory."""
    def make(kind: str, version: str) -> Package:
        assert pubtest_ctx.directory is not None, "a package needs a scratch directory first"
        return Package(pubtest_ctx.directory, tmp_path, kind, version, binary("pubrel"), binary("pub").parent,
                       pubtest_ctx, monkeypatch)
    return make
