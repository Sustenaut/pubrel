"""Steps for every pubrel feature."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/add.feature")
scenarios("../features/next.feature")
scenarios("../features/check.feature")
scenarios("../features/prepare.feature")
scenarios("../features/tag.feature")
scenarios("../features/changelog.feature")
scenarios("../features/verify.feature")
scenarios("../features/multi.feature")
scenarios("../features/security.feature")
scenarios("../features/api.feature")
scenarios("../features/summary.feature")
scenarios("../features/cli.feature")
scenarios("../features/withdraw.feature")


# --- givens ------------------------------------------------------------------


@given(parsers.parse('a released-never {kind} package at version "{version}"'),
       target_fixture="pkg")
def never_released(make_package, kind: str, version: str):
    return make_package(kind, version)


@given(parsers.parse('a {kind} package released at version "{version}"'), target_fixture="pkg")
def released(make_package, kind: str, version: str):
    pkg = make_package(kind, version)
    pkg.add("added", "the first release")
    pkg.commit("record the first release's changes")
    pkg.git("push", "-q", "origin", "main")
    pkg.release()
    return pkg


@given(parsers.parse('the unreleased changes "{categories}"'))
def unreleased_changes(pkg, categories: str):
    for category in categories.split(","):
        pkg.add(category, f"a {category} change")


@given(parsers.parse('the unreleased changes "{categories}" pushed to main'))
def unreleased_pushed(pkg, categories: str):
    unreleased_changes(pkg, categories)
    pkg.commit("record changes")
    pkg.git("push", "-q", "origin", "main")


@given("an uncommitted edit")
def uncommitted(pkg):
    (pkg.root / pkg.source).write_text("// edited\n")


@given(parsers.parse('it is released again with "{category}" "{change}"'))
def released_again(pkg, category: str, change: str):
    pkg.add(category, change)
    pkg.commit("record a change")
    pkg.git("push", "-q", "origin", "main")
    pkg.release()


@given(parsers.parse('the tag "{tag}" already exists'))
def tag_exists(pkg, tag: str):
    pkg.git("tag", tag)


# --- whens -------------------------------------------------------------------


@when(parsers.parse('I run pubrel add "{category}" "{change}"'), target_fixture="result")
def run_add(pkg, category: str, change: str):
    return pkg.pubrel_run("add", category, change)


@when("I run pubrel next", target_fixture="result")
def run_next(pkg):
    return pkg.pubrel_run("next")


@when("I run pubrel prepare", target_fixture="result")
def run_prepare(pkg):
    return pkg.pubrel_run("prepare")


@when("I run pubrel tag", target_fixture="result")
def run_tag(pkg):
    return pkg.pubrel_run("tag")


@when("I run pubrel changelog", target_fixture="result")
def run_changelog(pkg):
    return pkg.pubrel_run("changelog")


@when(parsers.parse('a branch changes "{path}"'))
def branch_changes(pkg, path: str):
    pkg.git("checkout", "-q", "-b", "feature")
    target = pkg.root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("// changed\n")
    pkg.commit("a change")


@when(parsers.parse('a branch changes "{path}" and records "{category}" "{change}"'))
def branch_changes_and_records(pkg, path: str, category: str, change: str):
    branch_changes(pkg, path)
    pkg.add(category, change)
    pkg.commit("record it")


@when(parsers.parse('a branch records "{category}" "{change}" and is released'))
def branch_released(pkg, category: str, change: str):
    # As merged: the change and its record land on main; the release PR is
    # then cut from main and checked against it.
    pkg.add(category, change)
    pkg.commit("record a change")
    pkg.git("push", "-q", "origin", "main")
    pkg.ok(str(pkg.pubrel), "prepare", "--no-pr")


@when(parsers.parse('the branch\'s manifest is set to "{version}"'))
def set_manifest(pkg, version: str):
    path = pkg.root / pkg.manifest
    text = path.read_text().replace(f'version = "{pkg.manifest_version()}"',
                                    f'version = "{version}"', 1)
    path.write_text(text)
    pkg.commit("tamper with the version")


@when("pub is not available")
def no_pub(pkg):
    pkg.without_pub()


@when("the release's signatures are removed")
def unsign(pkg):
    """Delete every signature over the package publet's claims."""
    targets = {pkg.locked(f"pkg.demo#{part}") for part in ("identity", "release")}
    for path in pkg.objects().glob("*.cbor"):
        obj = json.loads(pkg.pub("read", "--json", str(path)).stdout)["object"]
        if obj["type"] == "sig" and obj["body"]["target"] in targets:
            path.unlink()
    pkg.commit("remove the release's signatures")


@when("release.json names the identity claim as its key")
def wrong_key(pkg):
    path = pkg.root / "release.json"
    config = json.loads(path.read_text())
    config["key"] = pkg.locked("pkg.demo#identity")
    path.write_text(json.dumps(config, indent=2))
    pkg.commit("name the wrong key")


@when("release.json names no key")
def no_key(pkg):
    path = pkg.root / "release.json"
    config = json.loads(path.read_text())
    del config["key"]
    path.write_text(json.dumps(config, indent=2))
    pkg.commit("name no key")


@when("the published identity claim is altered")
def alter(pkg):
    # Same length, so the bytes stay canonical CBOR and only the identifier
    # gives the alteration away.
    path = pkg.object_file(pkg.locked("pkg.demo#identity"))
    data = path.read_bytes()
    assert b"0.1.1" in data
    path.write_bytes(data.replace(b"0.1.1", b"0.1.2"))
    pkg.commit("alter the published identity")


@when("I run pubrel check against main", target_fixture="result")
def run_check(pkg):
    return pkg.pubrel_run("check", "origin/main")


# --- thens -------------------------------------------------------------------


@then(parsers.parse('the unreleased list holds "{category}" "{change}"'))
def holds(pkg, category: str, change: str):
    assert {"category": category, "change": change} in pkg.unreleased()


@then("the unreleased list is empty")
def empty(pkg):
    assert pkg.unreleased() == []


@then(parsers.parse('the manifest states "{version}"'))
def manifest_states(pkg, version: str):
    assert pkg.manifest_version() == version


@given(parsers.parse('a package-lock.json at version "{version}"'))
def package_lock(pkg, version: str):
    lock = {"name": "demo", "version": version, "lockfileVersion": 3, "requires": True,
            "packages": {"": {"name": "demo", "version": version},
                         "node_modules/dep": {"version": version}}}
    (pkg.root / "package-lock.json").write_text(json.dumps(lock, indent=2) + "\n")
    pkg.commit("add package-lock.json")
    pkg.git("push", "-q", "origin", "main")


@then(parsers.parse('package-lock.json states "{version}" for itself and its root package'))
def package_lock_states(pkg, version: str):
    lock = json.loads((pkg.root / "package-lock.json").read_text())
    assert lock["version"] == version, lock
    assert lock["packages"][""]["version"] == version, lock


@then("package-lock.json keeps its dependencies' versions")
def package_lock_keeps(pkg):
    lock = json.loads((pkg.root / "package-lock.json").read_text())
    assert lock["packages"]["node_modules/dep"]["version"] == "0.1.0", lock


@then(parsers.parse('the manifest pins its internal dependency at "{version}"'))
def internal_pin(pkg, version: str):
    text = (pkg.root / pkg.manifest).read_text()
    assert f'core = {{ path = "crates/core", version = "{version}" }}' in text, text


@then(parsers.parse('the published release record states "{version}" with "{category}"'))
def record_states(pkg, version: str, category: str):
    body = pkg.published("pkg.demo#release")["body"]
    assert f"demo {version} was released" in body["content"], body["content"]
    assert category in [row[0] for row in body["data"]["rows"]]


@then(parsers.parse('CHANGELOG.md has a section "{heading}"'))
def changelog_section(pkg, heading: str):
    assert heading in (pkg.root / "CHANGELOG.md").read_text()


@then(parsers.parse('a pull request "{title}" was requested from "{branch}"'))
def pr_requested(pkg, title: str, branch: str):
    calls = pkg.gh_calls()
    assert any(c[:2] == ["pr", "create"] and title in c for c in calls), calls
    assert pkg.git("rev-parse", "--abbrev-ref", "HEAD") == branch
    assert branch in pkg.git("ls-remote", "--heads", "origin")


@then(parsers.parse('the tag "{tag}" exists and names the package publet'))
def tag_names_package(pkg, tag: str):
    message = pkg.git("tag", "-l", "--format=%(contents)", tag)
    assert "package pkg.demo [PKG-DEMOPK-10-2026]" in message, message
    assert tag in pkg.git("ls-remote", "--tags", "origin")


@then(parsers.parse('a GitHub release "{tag}" was requested'))
def release_requested(pkg, tag: str):
    assert any(c[:3] == ["release", "create", tag] for c in pkg.gh_calls()), pkg.gh_calls()


@then("no tag exists")
def no_tag(pkg):
    assert pkg.git("tag", "-l") == ""


@then(parsers.parse('CHANGELOG.md lists "{newer}" before "{older}"'))
def newest_first(pkg, newer: str, older: str):
    text = (pkg.root / "CHANGELOG.md").read_text()
    assert text.index(f"## {newer}") < text.index(f"## {older}"), text


@then(parsers.parse('CHANGELOG.md has "{entry}" under "{heading}"'))
def under_heading(pkg, entry: str, heading: str):
    text = (pkg.root / "CHANGELOG.md").read_text()
    section = text.split(heading, 1)[1]
    assert f"- {entry}" in section.split("##", 1)[0], text


# --- several packages --------------------------------------------------------


@given(parsers.parse('a workspace whose "{a}" and "{b}" are released at version "{version}"'),
       target_fixture="pkg")
def multi_released(make_package, a: str, b: str, version: str):
    pkg = make_package("multi", version)
    for name in (a, b):
        pkg.add("added", f"the first release of {name}", package=name)
        pkg.commit(f"record {name}'s first release")
        pkg.git("push", "-q", "origin", "main")
        pkg.release(package=name)
    return pkg


@when(parsers.parse('"{name}" records "{category}" "{change}" and is released'))
def multi_release(pkg, name: str, category: str, change: str):
    pkg.add(category, change, package=name)
    pkg.commit(f"record a change to {name}")
    pkg.git("push", "-q", "origin", "main")
    pkg.release(package=name)


@when(parsers.parse('"{name}" records "{category}" "{change}" and its release is cut'))
def multi_cut(pkg, name: str, category: str, change: str):
    pkg.add(category, change, package=name)
    pkg.commit(f"record a change to {name}")
    pkg.git("push", "-q", "origin", "main")
    pkg.cut(package=name)


@when(parsers.parse('"{name}" records "{category}" "{change}" on main'))
def multi_record(pkg, name: str, category: str, change: str):
    pkg.add(category, change, package=name)
    pkg.commit(f"record a change to {name}")


@when(parsers.parse('a branch changes "{path}" and records "{category}" "{change}" for "{name}"'))
def multi_branch(pkg, path: str, category: str, change: str, name: str):
    branch_changes(pkg, path)
    pkg.add(category, change, package=name)
    pkg.commit("record it")


@when(parsers.parse('it also records "{category}" "{change}" for "{name}"'))
def multi_also(pkg, category: str, change: str, name: str):
    pkg.add(category, change, package=name)
    pkg.commit("record it too")


@when(parsers.parse('I run pubrel add "{category}" "{change}" without naming a package'),
      target_fixture="result")
def multi_add_unnamed(pkg, category: str, change: str):
    return pkg.pubrel_run("add", category, change)


@then(parsers.parse('the release branch is "{branch}"'))
def release_branch(pkg, branch: str):
    assert pkg.last_branch == branch, pkg.last_branch


@then(parsers.parse('"{manifest}" states "{version}"'))
def manifest_at(pkg, manifest: str, version: str):
    assert pkg.manifest_version(manifest) == version


@then(parsers.parse('the workspace states "{version}"'))
def workspace_states(pkg, version: str):
    assert pkg.manifest_version("Cargo.toml") == version


@then(parsers.parse('the workspace pins "{path}" at "{version}"'))
def workspace_pins(pkg, path: str, version: str):
    assert pkg.pinned(path) == version, (pkg.root / "Cargo.toml").read_text()


@then(parsers.parse('the tag "{tag}" exists'))
def tag_present(pkg, tag: str):
    assert pkg.git("tag", "-l", tag) == tag


@then(parsers.parse('"{path}" names the package publet "{slug}"'))
def changelog_names(pkg, path: str, slug: str):
    text = (pkg.root / path).read_text()
    assert f"package publet `{slug}`" in text, text
    assert "## 0.1.0" in text, text


# --- security checks ---------------------------------------------------------


def declare(pkg, kind: str, entry: dict, push: bool = True):
    path = pkg.root / "release.json"
    config = json.loads(path.read_text())
    config.setdefault("security", {}).setdefault(kind, []).append(entry)
    path.write_text(json.dumps(config, indent=2))
    pkg.commit(f"declare a security {kind[:-1]}")
    if push:
        pkg.git("push", "-q", "origin", "main")


@given(parsers.parse('release.json declares the security check "{name}" that prints "{line}" with tool "{tool}"'))
def check_prints(pkg, name: str, line: str, tool: str):
    declare(pkg, "checks", {"name": name, "run": f"printf 'checking\\n{line}\\n'",
                            "version": f"echo '{tool}'"})


@given(parsers.parse('release.json declares the security check "{name}" that prints "{line}" and warns "{warning}"'))
def check_prints_and_warns(pkg, name: str, line: str, warning: str):
    # As cargo deny does: the summary on stdout, then warnings on stderr.
    declare(pkg, "checks", {"name": name,
                            "run": f"printf '{line}\\n'; printf '{warning}\\n' >&2",
                            "version": "echo 'cargo-deny 9.9.9'"})


@given(parsers.parse('release.json declares the security check "{name}" that fails'))
def check_fails(pkg, name: str):
    declare(pkg, "checks", {"name": name,
                            "run": "echo 'error[vulnerability]: RUSTSEC-0000-0000' >&2; exit 1"})


@given(parsers.parse('release.json declares the security fact "{name}" that prints "{value}"'))
def fact_prints(pkg, name: str, value: str):
    declare(pkg, "facts", {"name": name, "run": f"echo '{value}'"})


@when(parsers.parse('release.json now declares the security check "{name}" that prints "{line}" with tool "{tool}"'))
def check_declared_late(pkg, name: str, line: str, tool: str):
    # On the release branch, after the release was cut without it.
    declare(pkg, "checks", {"name": name, "run": f"printf '{line}\\n'",
                            "version": f"echo '{tool}'"}, push=False)


@when("the security record's signatures are removed")
def unsign_security(pkg):
    target = pkg.locked("pkg.demo#security")
    for path in pkg.objects().glob("*.cbor"):
        obj = json.loads(pkg.pub("read", "--json", str(path)).stdout)["object"]
        if obj["type"] == "sig" and obj["body"]["target"] == target:
            path.unlink()
    pkg.commit("remove the security record's signatures")


def security_rows(pkg) -> list[list[str]]:
    return pkg.published("pkg.demo#security")["body"]["data"]["rows"]


@then(parsers.parse('the published security record lists the check "{name}" with "{result}" from "{tool}"'))
def records_check(pkg, name: str, result: str, tool: str):
    rows = security_rows(pkg)
    assert any(r[0] == "check" and r[1] == name and r[3] == tool and r[4] == result
               for r in rows), rows


@then(parsers.parse('the published security record lists the fact "{name}" as "{value}"'))
def records_fact(pkg, name: str, value: str):
    rows = security_rows(pkg)
    assert any(r[0] == "fact" and r[1] == name and r[4] == value for r in rows), rows


# --- API checks ----------------------------------------------------------------

# A stand-in for cargo-semver-checks: it logs its arguments and reports
# whatever $SEMVER_API says the API did, judged by the release type asked
# about -- a break passes only a major release, an addition a minor one or
# more -- as the real tool judges it.
SEMVER_CHECKS = """#!/bin/sh
printf '%s\\037' "$@" >> "$SEMVER_LOG"
printf '\\n' >> "$SEMVER_LOG"
level=""; prev=""
for a in "$@"; do [ "$prev" = "--release-type" ] && level="$a"; prev="$a"; done
api=$(cat "$SEMVER_API" 2>/dev/null)
if [ "$api" = breaks ] && [ "$level" != major ]; then
  echo "--- failure function_missing: pub fn removed or renamed ---"; exit 1
fi
if [ "$api" = grows ] && [ "$level" = patch ]; then
  echo "--- warning function_added: pub fn added in a patch release ---"; exit 1
fi
echo "     Summary no semver update required"
"""


def semver_log(pkg) -> list[list[str]]:
    log = pkg.tmp / "semver.log"
    if not log.exists():
        return []
    return [line.split("\x1f")[:-1] for line in log.read_text().splitlines()]


@given("release.json checks the crate's API")
def checks_api(pkg):
    tool = pkg.fake / "cargo-semver-checks"
    tool.write_text(SEMVER_CHECKS)
    tool.chmod(0o755)
    pkg.env["SEMVER_LOG"] = str(pkg.tmp / "semver.log")
    pkg.env["SEMVER_API"] = str(pkg.tmp / "semver.api")
    path = pkg.root / "release.json"
    config = json.loads(path.read_text())
    config["semver-checks"] = True
    path.write_text(json.dumps(config, indent=2))
    pkg.commit("check the API")
    pkg.git("push", "-q", "origin", "main")


@given(parsers.parse('a cargo crate released and tagged at "{version}", whose API is checked'),
       target_fixture="pkg")
def checked_crate(make_package, version: str):
    pkg = released(make_package, "cargo", version)
    checks_api(pkg)
    pkg.git("tag", f"v{version}")
    return pkg


@given("the API breaks")
def api_breaks(pkg):
    (pkg.tmp / "semver.api").write_text("breaks")


@given("the API grows")
def api_grows(pkg):
    (pkg.tmp / "semver.api").write_text("grows")


@when(parsers.parse('the tag "{tag}" is deleted'))
def tag_deleted(pkg, tag: str):
    pkg.git("tag", "-d", tag)


def checked(call: list[str], crate: str, tag: str, level: str) -> bool:
    expected = ["semver-checks", "check-release", "--package", crate,
                "--baseline-rev", tag, "--release-type", level]
    return call == expected


@then(parsers.parse('the API of "{crate}" was checked against "{tag}" as a "{level}" release'))
def api_checked(pkg, crate: str, tag: str, level: str):
    calls = semver_log(pkg)
    assert any(checked(c, crate, tag, level) for c in calls), calls


@then(parsers.parse('the API of "{crate}" was last checked against "{tag}" as a "{level}" release'))
def api_last_checked(pkg, crate: str, tag: str, level: str):
    calls = semver_log(pkg)
    assert calls and checked(calls[-1], crate, tag, level), calls


@then(parsers.parse('pubrel next prints "{version}"'))
def next_prints(pkg, version: str):
    proc = pkg.pubrel_run("next")
    assert proc.returncode == 0 and proc.stdout.strip() == version, proc.stdout + proc.stderr


# --- summaries -----------------------------------------------------------------


def unreleased_json(pkg) -> dict:
    path = pkg.root / "corpus" / "publets" / "pkg.demo" / "unreleased.json"
    return json.loads(path.read_text()) if path.exists() else {}


@given(parsers.parse('the summary "{text}"'))
def set_summary(pkg, text: str):
    pkg.ok(str(pkg.pubrel), "summary", text)
    pkg.commit("summarise the next release")
    pkg.git("push", "-q", "origin", "main")


@when("I run pubrel summary", target_fixture="result")
def run_summary(pkg):
    return pkg.pubrel_run("summary")


@when("the summary's signatures are removed")
def unsign_summary(pkg):
    target = pkg.locked("pkg.demo#summary")
    for path in pkg.objects().glob("*.cbor"):
        obj = json.loads(pkg.pub("read", "--json", str(path)).stdout)["object"]
        if obj["type"] == "sig" and obj["body"]["target"] == target:
            path.unlink()
    pkg.commit("remove the summary's signatures")


@then(parsers.parse('CHANGELOG.md has "{text}" before "{heading}"'))
def changelog_before(pkg, text: str, heading: str):
    log = (pkg.root / "CHANGELOG.md").read_text()
    newest = log.split("\n## ")[1]
    assert text in newest and newest.index(text) < newest.index(heading), newest


@then(parsers.parse('the pull request\'s notes begin the release with "{text}"'))
def notes_begin(pkg, text: str):
    # The notes span lines, so read the fake gh's log whole: the summary
    # follows the line naming the commit and release record.
    log = pkg.gh_log.read_text()
    assert "pr\x1fcreate" in log, log
    assert f"`.\n\n{text}\n\n### " in log, log


@then(parsers.parse('the published summary is "{content}"'))
def published_summary(pkg, content: str):
    assert pkg.published("pkg.demo#summary")["body"]["content"] == content


@then("the unreleased list has no summary")
def no_unreleased_summary(pkg):
    assert "summary" not in unreleased_json(pkg)


@then("no summary is published")
def no_summary(pkg):
    lock = json.loads((pkg.root / "corpus" / "corpus.lock").read_text())
    assert not any(r["slug"] == "pkg.demo#summary" for r in lock["publets"])


# --- starting out ----------------------------------------------------------------


class FreshRepo:
    """A git repository with only a Cargo.toml: what `pubrel init` starts from."""

    def __init__(self, root: Path, pubrel: Path):
        self.root = root
        self.pubrel = pubrel
        self.env = dict(os.environ)

    def pubrel_run(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run([str(self.pubrel), *args], cwd=self.root, env=self.env,
                              capture_output=True, text=True, check=False)

    def unreleased(self, package: str = "pkg.demo") -> list[dict]:
        path = self.root / "corpus" / "publets" / package / "unreleased.json"
        return json.loads(path.read_text())["changes"]


@given(parsers.parse('a fresh cargo repository named "{name}"'), target_fixture="pkg")
def fresh_repo(tmp_path, binary, name: str):
    root = tmp_path / name
    root.mkdir()
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=root, check=True)
    (root / "Cargo.toml").write_text(
        f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\n')
    return FreshRepo(root, binary("pubrel"))


@given(parsers.parse('pubrel "{args}" has run'))
def has_run(pkg, args: str):
    proc = pkg.pubrel_run(*args.split())
    assert proc.returncode == 0, proc.stdout + proc.stderr


@when(parsers.parse('I run pubrel "{args}"'), target_fixture="result")
def run_pubrel(pkg, args: str):
    return pkg.pubrel_run(*args.split())


@then(parsers.parse('release.json names the package "{name}" as "{slug}" with a "{kind}" manifest at "{path}"'))
def release_json_names(pkg, name: str, slug: str, kind: str, path: str):
    config = json.loads((pkg.root / "release.json").read_text())
    assert (config["name"], config["package"]) == (name, slug), config
    assert config["manifest"] == {"kind": kind, "path": path}, config


@then("it prints this pubrel's version as \"pubrel X.Y.Z\"")
def prints_version(result):
    import tomllib
    manifest = Path(__file__).resolve().parents[2] / "Cargo.toml"
    version = tomllib.loads(manifest.read_text())["package"]["version"]
    assert result.stdout.strip() == f"pubrel {version}", result.stdout


@then("stderr is empty")
def stderr_empty(result):
    assert result.stderr == "", result.stderr


# --- withdrawing ---------------------------------------------------------------


def objects_added(pkg, rev: str) -> set[str]:
    out = pkg.git("diff", "--name-only", "--diff-filter=A", f"main...{rev}", "--", "corpus/objects")
    return set(out.split())


@given(parsers.parse('a release is cut and its branch abandoned as "{name}"'))
def cut_and_abandon(pkg, name: str):
    branch = pkg.cut()
    pkg.git("branch", "-m", branch, name)
    pkg.git("checkout", "-q", "main")


@given(parsers.parse('main moves on with "{category}" "{change}"'))
def main_moves(pkg, category: str, change: str):
    pkg.add(category, change)
    pkg.commit("record another change")
    pkg.git("push", "-q", "origin", "main")


@given(parsers.parse('pubrel withdraw "{which}" has run'))
def withdrawn(pkg, which: str):
    pkg.ok(str(pkg.pubrel), "withdraw", which)


@given(parsers.parse('a branch "{name}" that changes only "{path}"'))
def plain_branch(pkg, name: str, path: str):
    pkg.git("checkout", "-q", "-b", name)
    (pkg.root / path).write_text("// changed\n")
    pkg.commit("a change")
    pkg.git("checkout", "-q", "main")


@when(parsers.parse('I run pubrel withdraw "{which}"'), target_fixture="result")
def run_withdraw(pkg, which: str):
    return pkg.pubrel_run("withdraw", which)


@when("the release is cut again")
def cut_again(pkg):
    pkg.cut()


@when("the abandoned branch's objects are added to the release branch")
def add_abandoned(pkg):
    files = sorted(objects_added(pkg, "abandoned"))
    pkg.git("checkout", "abandoned", "--", *files)
    pkg.commit("carry the abandoned release's objects")


@then(".draft-discards is ignored by git")
def discards_ignored(pkg):
    assert (pkg.root / "corpus" / ".draft-discards").exists()
    pkg.ok("git", "check-ignore", "-q", "corpus/.draft-discards")
    assert pkg.git("status", "--porcelain") == ""


@then(parsers.parse('CHANGELOG.md has one section for "{version}"'))
def one_section(pkg, version: str):
    log = (pkg.root / "CHANGELOG.md").read_text()
    assert log.count(f"\n## {version} ") == 1, log


@then("the release adds no object the abandoned branch added")
def nothing_abandoned(pkg):
    assert not objects_added(pkg, "HEAD") & objects_added(pkg, "abandoned")


@then("pubrel check against main passes on the release branch")
def release_checks(pkg):
    proc = pkg.pubrel_run("check", "origin/main")
    assert proc.returncode == 0, proc.stdout + proc.stderr


@given(parsers.parse('the abandoned release is pull request {number:d} from "{branch}", "{state}"'))
def abandoned_pr(pkg, number: int, branch: str, state: str):
    # As GitHub holds it: the branch, and the pull request's own ref. The
    # clone keeps a local branch of that name too, as prepare leaves one.
    pkg.git("push", "-q", "origin", f"abandoned:refs/heads/{branch}",
            f"abandoned:refs/pull/{number}/head")
    pkg.git("branch", branch, "abandoned")
    (pkg.tmp / "gh-pr-view.json").write_text(json.dumps(
        {"state": state, "headRefName": branch, "isCrossRepository": False}))


@then(parsers.parse('neither origin nor this clone has the branch "{branch}"'))
def branch_gone(pkg, branch: str):
    assert pkg.git("ls-remote", "--heads", "origin", branch) == ""
    assert pkg.git("branch", "--list", branch) == ""


@then("there is no .draft-discards")
def no_discards(pkg):
    assert not (pkg.root / "corpus" / ".draft-discards").exists()


@given(parsers.parse('release.json says a release installs with "{command}"'))
def install_command(pkg, command: str):
    path = pkg.root / "release.json"
    config = json.loads(path.read_text())
    config["install"] = command
    path.write_text(json.dumps(config, indent=2))
    pkg.commit("say how a release installs")
    pkg.git("push", "-q", "origin", "main")


@then(parsers.parse('the pull request\'s notes say "{text}"'))
def notes_say(pkg, text: str):
    log = pkg.gh_log.read_text()
    assert "pr\x1fcreate" in log and text in log, log


@given(parsers.parse('release.json serves "{path}" through jsDelivr'))
def serves_through_jsdelivr(pkg, path: str):
    config_path = pkg.root / "release.json"
    config = json.loads(config_path.read_text())
    config["jsdelivr"] = {"files": [path]}
    config_path.write_text(json.dumps(config, indent=2))
    pkg.commit("serve through jsDelivr")
    pkg.git("push", "-q", "origin", "main")


def fake_curl(pkg, code: int):
    curl = pkg.fake / "curl"
    log = pkg.tmp / "curl.log"
    curl.write_text(f'#!/bin/sh\nfor a; do last="$a"; done\necho "$last" >> "{log}"\n'
                    f'[ {code} -eq 0 ] || echo "curl: (22) failed" >&2\nexit {code}\n')
    curl.chmod(0o755)


@given(parsers.parse('the repository is "{repo}" with a recording curl'))
def recording_curl(pkg, repo: str):
    pkg.env["GITHUB_REPOSITORY"] = repo
    fake_curl(pkg, 0)


@given(parsers.parse('the repository is "{repo}" with a curl that fails'))
def failing_curl(pkg, repo: str):
    pkg.env["GITHUB_REPOSITORY"] = repo
    fake_curl(pkg, 22)


@then(parsers.parse('curl fetched "{url}"'))
def curl_fetched(pkg, url: str):
    fetched = (pkg.tmp / "curl.log").read_text().splitlines()
    assert url in fetched, fetched
