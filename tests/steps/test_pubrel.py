"""Steps for every pubrel feature."""

from __future__ import annotations

import json
import re
import shlex
import subprocess
from pathlib import Path

from pytest_bdd import given, parsers, scenarios, then, when

from pubtest.steps.git import git

from conftest import GH_ANSWER, checkout_cut, unreleased

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
scenarios("../features/unfinished.feature")
scenarios("../features/fragments.feature")


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
    pkg.cut()


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


# --- thens -------------------------------------------------------------------


@then(parsers.parse('the unreleased list holds "{category}" "{change}"'))
def holds(pubtest_ctx, category: str, change: str):
    assert {"category": category, "change": change} in unreleased(pubtest_ctx.directory)


@then("the unreleased list is empty")
def empty(pubtest_ctx):
    assert unreleased(pubtest_ctx.directory) == []


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


@then(parsers.parse('the published release record states "{version}" with "{category}"'))
def record_states(pkg, version: str, category: str):
    body = pkg.published("pkg.demo#release")["body"]
    assert f"demo {version} was released" in body["content"], body["content"]
    assert category in [row[0] for row in body["data"]["rows"]]


@then(parsers.parse('the tag "{tag}" exists and names the package publet'))
def tag_names_package(pkg, tag: str):
    message = pkg.git("tag", "-l", "--format=%(contents)", tag)
    assert "package pkg.demo [PKG-DEMOPK-10-2026]" in message, message
    assert tag in pkg.git("ls-remote", "--tags", "origin")


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


@then(parsers.parse('the release branch is "{branch}"'))
def release_branch(pkg, branch: str):
    assert pkg.last_branch == branch, pkg.last_branch


@then(parsers.parse('"{manifest}" states "{version}"'))
def manifest_at(pkg, manifest: str, version: str):
    assert pkg.manifest_version(manifest) == version


@then(parsers.parse('the workspace pins "{path}" at "{version}"'))
def workspace_pins(pkg, path: str, version: str):
    assert pkg.pinned(path) == version, (pkg.root / "Cargo.toml").read_text()


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


@given(parsers.parse('release.json declares the security check "{name}" that runs "{command}"'))
def check_runs(pkg, name: str, command: str):
    declare(pkg, "checks", {"name": name, "run": command})


@given(parsers.parse('tests/requirements.lock pins pubrules "{pin}" and .venv has pubrules "{have}"'))
def venv_has(pkg, pin: str, have: str):
    # As `make venv` leaves it: the lock committed, .venv ignored.
    (pkg.root / "tests").mkdir(exist_ok=True)
    (pkg.root / "tests" / "requirements.lock").write_text(
        f"pubrules @ https://github.com/schryer/pubrules/releases/download/v{pin}/pubrules-{pin}-py3-none-any.whl \\\n"
        "    --hash=sha256:00\n")
    site = pkg.root / ".venv" / "lib" / "python3.14" / "site-packages"
    (site / f"pubrules-{have}.dist-info").mkdir(parents=True)
    gitignore = pkg.root / ".gitignore"
    gitignore.write_text(gitignore.read_text() + "/.venv/\n")
    pkg.commit("pin pubrules")
    pkg.git("push", "-q", "origin", "main")


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

# pubtest's stand-in for cargo-semver-checks reports a break or an addition
# when told to, judged by the release type asked about -- a break passes
# only a major release, an addition a minor one or more -- as the real tool
# judges it. Each release type has a row of its own, so the stand-in answers
# a type the same however often it is asked.
SUMMARY = "     Summary no semver update required\n"
BREAK = "--- failure function_missing: pub fn removed or renamed ---\n"
ADDITION = "--- warning function_added: pub fn added in a patch release ---\n"


def semver_answers(pkg, failing: dict[str, str]) -> None:
    pkg.standins.add_turns("cargo-semver-checks", [
        {"args": f"* --release-type {level}", "exit": 1 if level in failing else 0,
         "stdout": failing.get(level, SUMMARY)}
        for level in ("major", "minor", "patch")])


@given("release.json checks the crate's API")
def checks_api(pkg):
    semver_answers(pkg, {})
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
    semver_answers(pkg, {"minor": BREAK, "patch": BREAK})


@given("the API grows")
def api_grows(pkg):
    semver_answers(pkg, {"patch": ADDITION})


@when(parsers.parse('the tag "{tag}" is deleted'))
def tag_deleted(pkg, tag: str):
    pkg.git("tag", "-d", tag)


@then(parsers.parse('the API of "{crate}" was last checked against "{tag}" as a "{level}" release'))
def api_last_checked(pkg, crate: str, tag: str, level: str):
    """pubtest's `was run with` reads any run; this reads the last."""
    calls = pkg.standins.runs("cargo-semver-checks")
    expected = ["semver-checks", "check-release", "--package", crate,
                "--baseline-rev", tag, "--release-type", level]
    assert calls and calls[-1] == expected, calls


# --- summaries -----------------------------------------------------------------


@given(parsers.parse('the summary "{text}"'))
def set_summary(pkg, text: str):
    pkg.ok(str(pkg.pubrel), "summary", text)
    pkg.commit("summarise the next release")
    pkg.git("push", "-q", "origin", "main")


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
    # The notes span lines, which no quoted value can: the summary follows
    # the line naming the commit and release record.
    bodies = [c[c.index("--body") + 1] for c in pkg.gh_calls() if c[:2] == ["pr", "create"] and "--body" in c]
    assert any(f"`.\n\n{text}\n\n### " in body for body in bodies), pkg.gh_calls()


@then(parsers.parse('the published summary is "{content}"'))
def published_summary(pkg, content: str):
    assert pkg.published("pkg.demo#summary")["body"]["content"] == content


# --- starting out ----------------------------------------------------------------


@given(parsers.parse('a fresh cargo repository named "{name}"'))
def fresh_repo(pubtest_ctx, monkeypatch, name: str):
    """A git repository holding only a Cargo.toml, what `pubrel init` starts
    from, in a directory of that name: init names the package after it. It
    is the scratch directory from here on."""
    root = pubtest_ctx.directory / name
    root.mkdir()
    pubtest_ctx.directory = root
    monkeypatch.chdir(root)
    git(pubtest_ctx, "init", "-q", "-b", "main")
    (root / "Cargo.toml").write_text(
        f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\n')


@given(parsers.parse('pubrel "{args}" has run'))
def has_run(pubtest_ctx, run, binary, args: str):
    proc = run(binary("pubrel"), *shlex.split(args), cwd=pubtest_ctx.directory)
    assert proc.code == 0, proc.describe()
    checkout_cut(pubtest_ctx, proc.code, proc.stdout)


@then("it prints this pubrel's version as \"pubrel X.Y.Z\"")
def prints_version(result):
    import tomllib
    manifest = Path(__file__).resolve().parents[2] / "Cargo.toml"
    version = tomllib.loads(manifest.read_text())["package"]["version"]
    assert result.stdout.strip() == f"pubrel {version}", result.stdout


@then("stderr is empty")
def stderr_empty(result):
    """pubtest has `stdout is empty`, not this."""
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


@given(parsers.parse('a branch "{name}" that changes only "{path}"'))
def plain_branch(pkg, name: str, path: str):
    pkg.git("checkout", "-q", "-b", name)
    (pkg.root / path).write_text("// changed\n")
    pkg.commit("a change")
    pkg.git("checkout", "-q", "main")


@when("the release is cut again")
def cut_again(pkg):
    pkg.cut()


@when("the abandoned branch's objects are added to the release branch")
def add_abandoned(pkg):
    files = sorted(objects_added(pkg, "abandoned"))
    pkg.git("checkout", "abandoned", "--", *files)
    pkg.commit("carry the abandoned release's objects")


@then("the release adds no object the abandoned branch added")
def nothing_abandoned(pkg):
    assert not objects_added(pkg, "HEAD") & objects_added(pkg, "abandoned")


@given(parsers.parse('the abandoned release is pull request {number:d} from "{branch}", "{state}"'))
def abandoned_pr(pkg, number: int, branch: str, state: str):
    # As GitHub holds it: the branch, and the pull request's own ref. The
    # clone keeps a local branch of that name too, as prepare leaves one.
    pkg.git("push", "-q", "origin", f"abandoned:refs/heads/{branch}",
            f"abandoned:refs/pull/{number}/head")
    pkg.git("branch", branch, "abandoned")
    pkg.standins.add_turns("gh", [
        {"args": f"pr view {number} *", "stdout": json.dumps(
            {"state": state, "headRefName": branch, "isCrossRepository": False}) + "\n"},
        {"args": "*", "stdout": GH_ANSWER}])


@then(parsers.parse('neither origin nor this clone has the branch "{branch}"'))
def branch_gone(pkg, branch: str):
    assert pkg.git("ls-remote", "--heads", "origin", branch) == ""
    assert pkg.git("branch", "--list", branch) == ""


@given(parsers.parse('release.json says a release installs with "{command}"'))
def install_command(pkg, command: str):
    path = pkg.root / "release.json"
    config = json.loads(path.read_text())
    config["install"] = command
    path.write_text(json.dumps(config, indent=2))
    pkg.commit("say how a release installs")
    pkg.git("push", "-q", "origin", "main")


@given(parsers.parse('release.json serves "{path}" through jsDelivr'))
def serves_through_jsdelivr(pkg, path: str):
    config_path = pkg.root / "release.json"
    config = json.loads(config_path.read_text())
    config["jsdelivr"] = {"files": [path]}
    config_path.write_text(json.dumps(config, indent=2))
    pkg.commit("serve through jsDelivr")
    pkg.git("push", "-q", "origin", "main")


# --- releasing several packages at once ---------------------------------------


@given(parsers.parse('"{name}" builds a crate that depends on "{dependency}", with a Cargo.lock'))
def workspace_crate_depends(pkg, name: str, dependency: str):
    # The workspace's own crate, crates/core, needs the crate released on
    # its own; `cargo update` keeps the lockfile beside them.
    assert name == "demo" and dependency == "demo-algo"
    manifest = pkg.root / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace(
        'path = "crates/algo", version = "0.1.0" }',
        'path = "crates/algo", version = "0.1.0", package = "demo-algo" }'))
    (pkg.root / "crates" / "core" / "Cargo.toml").write_text(
        '[package]\nname = "demo-core"\nversion.workspace = true\nedition.workspace = true\n\n'
        '[dependencies]\nalgo.workspace = true\n')
    (pkg.root / ".gitignore").write_text("/corpus/.publet/\n/target/\n")
    pkg.ok("cargo", "generate-lockfile", "--offline")
    pkg.commit("the workspace's crate depends on demo-algo")
    pkg.git("push", "-q", "origin", "main")


@given(parsers.parse('"{name}" records "{category}" "{change}" on main'))
def multi_record_pushed(pkg, name: str, category: str, change: str):
    pkg.add(category, change, package=name)
    pkg.commit(f"record a change to {name}")
    pkg.git("push", "-q", "origin", "main")


@given(parsers.parse('a quality audit check that finds no report for "{name}"'))
def audit_finds_none(pkg, name: str):
    # pubrules' message when the report is missing, from a stand-in
    # `pubrules` that fails only for that package: the check names the
    # package from $PUBREL_PACKAGE, which pubrel sets for it.
    pkg.standins.add_turns("pubrules", [
        {"args": f"audit --check {package}",
         "exit": 1 if package == name else 0,
         "stderr": f"pubrules: no audit/{package}.md: run the rust-crate-quality agent\n" if package == name else "",
         "stdout": "" if package == name else "audited\n"}
        for package in ("demo", "demo-algo")])
    declare(pkg, "checks", {"name": "quality audit current", "run": 'pubrules audit --check "$PUBREL_PACKAGE"'})


@when("the release branch is merged into main")
def merge_release(pkg):
    branch = pkg.git("rev-parse", "--abbrev-ref", "HEAD")
    pkg.git("checkout", "-q", "main")
    pkg.git("merge", "-q", "--ff-only", branch)
    pkg.git("push", "-q", "origin", "main")


@then(parsers.parse('the current branch is "{branch}"'))
def current_branch(pkg, branch: str):
    assert pkg.git("rev-parse", "--abbrev-ref", "HEAD") == branch


@then(parsers.parse('Cargo.lock records "{crate}" at "{version}"'))
def lock_records(pkg, crate: str, version: str):
    text = (pkg.root / "Cargo.lock").read_text()
    assert f'name = "{crate}"\nversion = "{version}"' in text, text


@then(parsers.parse('the unreleased list of "{slug}" is empty'))
def unreleased_of_empty(pkg, slug: str):
    assert pkg.unreleased(slug) == []


@then(parsers.parse('the unreleased list of "{slug}" holds "{category}" "{change}"'))
def unreleased_of_holds(pkg, slug: str, category: str, change: str):
    assert {"category": category, "change": change} in pkg.unreleased(slug)


# --- one file per change ---------------------------------------------------------


def unreleased_dir(pkg) -> Path:
    return pkg.root / "corpus" / "publets" / "pkg.demo" / "unreleased"


@given(parsers.parse('a branch "{name}" that records "{category}" "{change}"'))
def branch_records(pkg, name: str, category: str, change: str):
    pkg.git("checkout", "-q", "-b", name, "main")
    pkg.add(category, change)
    pkg.commit(f"record {change}")
    pkg.git("checkout", "-q", "main")


@given(parsers.parse('the change file "{name}" records "{category}" "{change}"'))
def change_file(pkg, name: str, category: str, change: str):
    path = unreleased_dir(pkg) / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"category": category, "change": change}, indent=2) + "\n")


@given(parsers.parse('a pubrel 0.5 unreleased list holding "{category}" "{change}" '
                     'and the summary "{summary}"'))
def legacy_list(pkg, category: str, change: str, summary: str):
    path = pkg.root / "corpus" / "publets" / "pkg.demo" / "unreleased.json"
    path.write_text(json.dumps({"changes": [{"category": category, "change": change}],
                                "summary": summary}, indent=2) + "\n")


@given("they are pushed to main")
def pushed(pkg):
    if pkg.git("status", "--porcelain"):
        pkg.commit("record changes")
    pkg.git("push", "-q", "origin", "main")


@when(parsers.parse('"{a}" and "{b}" are merged into main'))
def merge_both(pkg, a: str, b: str):
    for branch in (a, b):
        pkg.git("merge", "-q", "--no-ff", "-m", f"Merge {branch}", branch)


@when(parsers.parse('a branch changes "{path}" and rewords the change file "{name}"'))
def branch_rewords(pkg, path: str, name: str):
    branch_changes(pkg, path)
    target = unreleased_dir(pkg) / name
    row = json.loads(target.read_text())
    target.write_text(json.dumps({**row, "change": row["change"] + ", reworded"}) + "\n")
    pkg.commit("reword a change")


@when(parsers.parse('a branch appends "{category}" "{change}" to the pubrel 0.5 unreleased list'))
def branch_appends_legacy(pkg, category: str, change: str):
    branch_changes(pkg, pkg.source)
    path = pkg.root / "corpus" / "publets" / "pkg.demo" / "unreleased.json"
    rows = json.loads(path.read_text()) if path.exists() else {"changes": []}
    rows["changes"].append({"category": category, "change": change})
    path.write_text(json.dumps(rows, indent=2) + "\n")
    pkg.commit("record it the 0.5 way")


@when(parsers.parse('the release branch also records "{category}" "{change}"'))
def release_branch_records(pkg, category: str, change: str):
    pkg.add(category, change)
    pkg.commit("a change main gained")


@then(parsers.parse('the unreleased directory holds {count:d} change files'))
def holds_files(pkg, count: int):
    names = [p.name for p in unreleased_dir(pkg).glob("*.json")]
    assert len(names) == count, names


@then(parsers.parse('the change file is named after when it was recorded and "{slug}"'))
def named(pkg, slug: str):
    import re
    names = [p.name for p in unreleased_dir(pkg).glob("*.json")]
    assert len(names) == 1, names
    assert re.fullmatch(rf"\d{{8}}T\d{{6}}Z-{re.escape(slug)}\.json", names[0]), names


@then(parsers.parse('the published release record lists "{first}" before "{second}"'))
def record_order(pkg, first: str, second: str):
    rows = [row[1] for row in pkg.published("pkg.demo#release")["body"]["data"]["rows"]]
    assert first in rows and second in rows and rows.index(first) < rows.index(second), rows


@then(parsers.parse('the published release record lists "{change}"'))
def record_lists(pkg, change: str):
    rows = [row[1] for row in pkg.published("pkg.demo#release")["body"]["data"]["rows"]]
    assert change in rows, rows


@then("the release deletes every unreleased file and adds none")
def release_only_deletes(pkg):
    publet = "corpus/publets/pkg.demo"
    out = pkg.git("diff", "--name-status", "origin/main...HEAD", "--",
                  f"{publet}/unreleased", f"{publet}/unreleased.json")
    rows = [line.split("\t") for line in out.splitlines()]
    assert rows and all(status == "D" for status, _ in rows), out
    assert not unreleased_dir(pkg).exists() or not any(unreleased_dir(pkg).iterdir())


# --- A prepare that does not finish (unfinished.feature) -------------------


@given("git refuses commits")
def refuse_commits(pkg):
    hook = pkg.root / ".git" / "hooks" / "pre-commit"
    hook.write_text("#!/bin/sh\necho 'refused by the test' >&2\nexit 1\n")
    hook.chmod(0o755)


@when("git takes commits again")
@given("git takes commits again")
def take_commits(pkg):
    (pkg.root / ".git" / "hooks" / "pre-commit").unlink()


@given(parsers.parse('the date is "{day}"'))
def the_date(pkg, day: str):
    """`date -u` as prepare runs it: the day, or the moment the lock stamps."""
    fake = pkg.standins.bin / "date"
    fake.write_text(f'#!/bin/sh\ncase "$*" in *%H*) echo "{day}T12:00:00Z";; *) echo "{day}";; esac\n')
    fake.chmod(0o755)


@given("a prepare has failed and undone itself")
def failed_prepare(pkg):
    done = pkg.run(str(pkg.pubrel), "prepare", "--no-pr")
    assert done.returncode != 0 and "undid the unfinished prepare" in done.stderr, done.stdout + done.stderr


@then(parsers.parse('this clone has no branch "{branch}"'))
def clone_lacks(pkg, branch: str):
    assert pkg.git("branch", "--list", branch) == ""


def _hashes(pkg) -> dict[str, str]:
    """Every file of this checkout, outside .git, by its sha256: the pub
    store and the signing key under corpus/.publet among them."""
    import hashlib  # noqa: PLC0415
    out = {}
    for path in sorted(pkg.root.rglob("*")):
        rel = path.relative_to(pkg.root)
        if rel.parts[0] == ".git" or not path.is_file():
            continue
        out[str(rel)] = hashlib.sha256(path.read_bytes()).hexdigest()
    return out


@given("this checkout, its signing key and its store are hashed")
def hashed(pkg):
    pkg.hashed = _hashes(pkg)
    assert "corpus/.publet/signing.key" in pkg.hashed and "corpus/.publet/objects.redb" in pkg.hashed


@then("this checkout, its signing key and its store are as they were hashed")
def as_hashed(pkg):
    now = _hashes(pkg)
    # The store is pub's: a prepare adds what it publishes, and keeps all it held.
    store = "corpus/.publet/objects.redb"
    assert now.pop(store, None) and pkg.hashed.get(store)
    before = {k: v for k, v in pkg.hashed.items() if k != store}
    assert now == before, set(now.items()) ^ set(before.items())


@then("no worktree of a prepare is left")
def no_worktree(pkg):
    left = pkg.root / ".git" / "pubrel"
    assert not left.exists() or not any(left.iterdir()), list(left.iterdir())
    assert "pubrel" not in pkg.git("worktree", "list")


@then(parsers.parse('origin has no branch "{branch}"'))
def origin_lacks(pkg, branch: str):
    assert pkg.git("ls-remote", "--heads", "origin", branch) == ""


@given("another clone has claimed the release on origin")
def other_clone_claims(pkg):
    other = pkg.tmp / "other"
    pkg.ok("git", "clone", "-q", str(pkg.remote), str(other))
    pkg.ok("git", "-C", str(other), "push", "-q", "--force-with-lease=refs/heads/release/v0.1.1:",
           "origin", "HEAD:refs/heads/release/v0.1.1")


@given("a prepare was killed after claiming the release and making its worktree")
def killed(pkg):
    base = pkg.git("rev-parse", "origin/main")
    pkg.git("push", "-q", "origin", f"{base}:refs/heads/release/v0.1.1")
    finished = subprocess.Popen(["true"])
    finished.wait()  # the killed prepare's pid: one that no longer runs
    tree = pkg.root / ".git" / "pubrel" / f"prepare-{finished.pid}"
    pkg.git("worktree", "add", "-q", "-b", "release/v0.1.1", str(tree), base)
    (tree / pkg.manifest).write_text((tree / pkg.manifest).read_text().replace('"0.1.0"', '"0.1.1"'))


@given(parsers.parse('a release is cut and pushed as "{branch}" without its pull request'))
def cut_and_pushed(pkg, branch: str):
    pkg.cut()
    pkg.git("push", "-q", "origin", branch)
    pkg.git("checkout", "-q", "main")


@given("nothing is checked out here after a prepare")
def keep_checkout(pubtest_ctx):
    """The suite checks out the branch a prepare cut, for the steps that
    read it (conftest's after-step hook); this scenario reads the checkout
    as prepare left it."""
    pubtest_ctx.keep_checkout = True


@then(parsers.parse('the release branch it cut has "{path}" holding "{text}"'))
def cut_branch_holds(pkg, pubtest_ctx, path: str, text: str):
    branch = re.search(r"committed on (\S+);", pubtest_ctx.outcome.stdout).group(1)
    assert pkg.git("show", f"{branch}:{path}") == text


@then(".draft-discards lists the unfinished attempt's records and a signature of one")
def discards_signatures(pkg):
    listed = [line.split("=", 1)[1] for line in (pkg.root / "corpus" / ".draft-discards").read_text().splitlines()
              if line.startswith("unfinished ")]
    kinds = [json.loads(pkg.pub("read", "--json", cid, cwd=pkg.root / "corpus").stdout)["object"]["type"]
             for cid in listed]
    assert "sig" in kinds and "claim.prose" in kinds, kinds


@given("main has a change this checkout has not pulled")
def main_ahead(pkg):
    pkg.add("fixed", "a fix this checkout never saw")
    pkg.commit("record a fix")
    pkg.git("push", "-q", "origin", "main")
    pkg.git("reset", "-q", "--hard", "HEAD~1")


@given("a release has been cut by a --no-pr prepare, and this checkout is still on main")
def prepared_once(pkg):
    first = pkg.ok(str(pkg.pubrel), "prepare", "--no-pr")
    pkg.first_branch = re.search(r"committed on (\S+);", first.stdout).group(1)
    pkg.first_tip = pkg.git("rev-parse", pkg.first_branch)


@then("the first prepare's release branch is kept, with its release commit")
def first_kept(pkg):
    assert pkg.git("rev-parse", pkg.first_branch) == pkg.first_tip
    assert pkg.git("log", "-1", "--format=%s", pkg.first_branch).startswith("Release ")


@given("origin cannot be reached")
def origin_unreachable(pkg):
    """Fetches still read origin's objects; pushes fail, as with a network or auth fault."""
    hook = pkg.remote / "hooks" / "pre-receive"
    hook.write_text("#!/bin/sh\necho 'the remote is unreachable (test)' >&2\nexit 1\n")
    hook.chmod(0o755)


@given(parsers.parse('another clone\'s prepare holds the claim on "{branch}"'))
def other_clone_claim_commit(pkg, branch: str):
    other = pkg.tmp / "elsewhere"
    pkg.ok("git", "clone", "-q", str(pkg.remote), str(other))
    message = (f"pubrel: claim {branch}\n\nBeing prepared by pubrel 9.9.9 on elsewhere, pid 4242, "
               "at 2026-01-01T12:00:00Z.")
    claim = pkg.ok("git", "-C", str(other), "commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", message).stdout.strip()
    pkg.ok("git", "-C", str(other), "push", "-q", f"--force-with-lease=refs/heads/{branch}:", "origin",
           f"{claim}:refs/heads/{branch}")
