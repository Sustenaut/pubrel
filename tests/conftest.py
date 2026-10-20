"""Fixtures for pubrel's functional suite.

Every scenario drives the built `pubrel` as a user would, against a
throwaway git repository with a bare "remote", a real corpus and key made
by a real released `pub`, and a fake `gh` that records what it was asked to
do -- so nothing here touches GitHub, and nothing imports the Rust.

Binaries are resolved by pubkit's `binary` fixture, from PUBREL_BIN_DIR
(pubrel) and PUB_BIN_DIR (pub), so another implementation runs this suite
by setting one variable. The steps that judge a result (`it succeeds`, `it
prints "..."`) are pubkit's too.
"""

from __future__ import annotations

import json
import os
import re
import stat
import subprocess
from pathlib import Path

import pytest

GH = """#!/bin/sh
printf '%s\\037' "$@" >> "$GH_LOG"
printf '\\n' >> "$GH_LOG"
if [ "$1 $2" = "pr view" ]; then cat "$GH_PR_VIEW"; exit; fi
echo "https://example.org/fake/1"
"""

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


class Package:
    """A package repository under test."""

    def __init__(self, tmp: Path, kind: str, version: str, pubrel: Path, pub_dir: Path):
        self.tmp = tmp
        self.kind = kind
        self.remote = tmp / "remote.git"
        self.root = tmp / "work"
        self.fake = tmp / "fakebin"
        self.gh_log = tmp / "gh.log"
        self.pubrel = pubrel
        self.env = dict(
            os.environ,
            PATH=f"{self.fake}{os.pathsep}{pub_dir}{os.pathsep}{os.environ['PATH']}",
            GH_LOG=str(self.gh_log),
            GH_PR_VIEW=str(tmp / "gh-pr-view.json"),
            GIT_AUTHOR_NAME="Test", GIT_AUTHOR_EMAIL="test@example.org",
            GIT_COMMITTER_NAME="Test", GIT_COMMITTER_EMAIL="test@example.org",
        )
        self.env.pop("PUB", None)
        self.fake.mkdir()
        gh = self.fake / "gh"
        gh.write_text(GH)
        gh.chmod(gh.stat().st_mode | stat.S_IEXEC)

        manifest_kind, manifest, template, code, source = MANIFESTS[kind]
        self.manifest = manifest
        self.source = source
        self.root.mkdir()
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
        """Make `pub` unrunnable: a `pub` first on the PATH that fails, and
        $PUB naming nothing."""
        fake = self.fake / "pub"
        fake.write_text("#!/bin/sh\necho 'pub is not available here' >&2\nexit 127\n")
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        self.env["PUB"] = str(self.tmp / "no-such-pub")

    def objects(self) -> Path:
        return self.root / "corpus" / "objects"

    def locked(self, slug: str) -> str:
        lock = json.loads((self.root / "corpus" / "corpus.lock").read_text())
        return next(r["cid"] for r in lock["publets"] if r["slug"] == slug)

    def object_file(self, cid: str) -> Path:
        return self.objects() / f"{cid.replace(':', '_')}.cbor"

    # --- running things ---------------------------------------------------

    def run(self, *argv: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
        return subprocess.run(list(argv), cwd=cwd or self.root, env=self.env,
                              capture_output=True, text=True, check=False)

    def ok(self, *argv: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
        proc = self.run(*argv, cwd=cwd)
        assert proc.returncode == 0, f"{argv}: {proc.stdout}{proc.stderr}"
        return proc

    def git(self, *args: str) -> str:
        return self.ok("git", *args).stdout.strip()

    def pub(self, *args: str, cwd: Path | None = None):
        return self.ok("pub", *args, cwd=cwd)

    def pubrel_run(self, *args: str):
        return self.run(str(self.pubrel), *args)

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
        self.ok(str(self.pubrel), "prepare", "--no-pr", *named)
        self.last_branch = self.git("rev-parse", "--abbrev-ref", "HEAD")
        return self.last_branch

    def release(self, package: str | None = None):
        """Cut a release and merge it, as merging its PR would."""
        branch = self.cut(package)
        self.git("checkout", "-q", "main")
        self.git("merge", "-q", "--ff-only", branch)
        self.git("push", "-q", "origin", "main")

    # --- reading back -----------------------------------------------------

    def unreleased(self, package: str = "pkg.demo") -> list[dict]:
        path = self.root / "corpus" / "publets" / package / "unreleased.json"
        return json.loads(path.read_text())["changes"] if path.exists() else []

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
        if not self.gh_log.exists():
            return []
        return [line.split("\x1f")[:-1] for line in self.gh_log.read_text().splitlines()]


@pytest.fixture
def make_package(tmp_path: Path, binary):
    def make(kind: str, version: str) -> Package:
        return Package(tmp_path, kind, version, binary("pubrel"), binary("pub").parent)
    return make
