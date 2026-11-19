//! Where a prepare does its work, how it claims its release, and undoing
//! one that did not finish.
//!
//! `pubrel prepare` changes a tree before it commits: the package publet,
//! the version, the lock, the unreleased changes it folds in, and the
//! objects `pub` exports. It does all of that in a worktree of its own
//! under the clone's git directory (`.git/pubrel/`), from `origin/main`, so
//! the checkout a person works in is never edited, and the worktree is
//! removed when the prepare ends, however it ends.
//!
//! The worktree shares the checkout's `pub` workspace without copying it:
//! its `corpus/.publet/` holds links to the store (`objects.redb`, which is
//! transactional and opened by one process at a time), the `config` and
//! the signing key, and a `drafts.json` of its own, since `pub` writes that
//! file in place. The corpus's `.draft-discards` is linked too, so what is
//! withdrawn stays withdrawn. Removing the worktree removes the links, never
//! what they point at.
//!
//! Before it changes anything, a prepare that will open a pull request
//! claims its release branch on `origin`: a commit of its own on
//! `origin/main`, naming who claims it, pushed with an empty lease. A push
//! of a branch that is already there at the same commit would succeed, so
//! the claim is a commit no other branch has: a second prepare, from this
//! clone or any other, is refused. The release commit replaces the claim,
//! leased on it. A prepare that fails deletes its claim, leased on it too,
//! so a branch that has gained its release commit is never deleted.
//! `pubrel withdraw --unfinished` clears a prepare that was killed: its
//! claim, if the branch holds no commit of its own, its worktree and its
//! local branch.

use std::path::{Path, PathBuf};

use crate::withdraw::git;

/// Where prepares make their worktrees: under the clone's git directory.
fn worktrees(root: &Path) -> Result<PathBuf, String> {
    Ok(root
        .join(git(root, &["rev-parse", "--git-common-dir"])?)
        .join("pubrel"))
}

/// What a prepare made, for undoing it: its release branch, once it made
/// it, and its claim on `origin`, once it pushed one.
#[derive(Default)]
pub struct Progress {
    /// The release branch this prepare created in its worktree.
    pub branch: Option<String>,
    /// The claim commit this prepare pushed to that branch on `origin`.
    pub claim: Option<String>,
}

/// Refuse a release branch this clone already has: one holding a release
/// commit is a release cut and not pushed; one that does not is left by a
/// prepare that stopped.
///
/// # Errors
///
/// Returns a message saying which, and what to do.
pub fn refuse_existing(root: &Path, branch: &str) -> Result<(), String> {
    let local = format!("refs/heads/{branch}");
    if git(root, &["show-ref", "--verify", "-q", &local]).is_err() {
        return Ok(());
    }
    let main = git(root, &["rev-parse", "origin/main"])?;
    let own = git(root, &["rev-list", "--count", &format!("{main}..{local}")])?;
    if own == "0" || is_claim(root, &local) {
        return Err(format!(
            "{branch} exists here, left by a prepare that stopped: `pubrel withdraw --unfinished`"
        ));
    }
    Err(format!(
        "{branch} exists here, holding a release commit: push it or delete it"
    ))
}

/// What the worktree's `pub` workspace links to in the checkout's.
const SHARED: [&str; 3] = ["objects.redb", "config", "signing.key"];

/// The first line of a claim commit's message.
const CLAIM: &str = "pubrel: claim ";

/// Claim `branch` on `origin`: a commit of its own on `base`, with
/// `base`'s tree, naming who claims it and when, pushed with an empty lease.
/// A branch already there differs from it, so the push fails; two claims
/// made at once differ from each other too. The claim commit, returned.
///
/// # Errors
///
/// Returns a message naming the branch and since when, when it is there.
pub fn claim(root: &Path, branch: &str, base: &str, who: &str) -> Result<String, String> {
    let message = format!("{CLAIM}{branch}\n\nBeing prepared by {who}.");
    let tree = format!("{base}^{{tree}}");
    let commit = git(root, &["commit-tree", &tree, "-p", base, "-m", &message])?;
    let lease = format!("--force-with-lease=refs/heads/{branch}:");
    let target = format!("{commit}:refs/heads/{branch}");
    git(root, &["push", "-q", &lease, "origin", &target]).map_err(|failed| {
        // Not there at all: the push failed for another reason, git's own.
        if git(root, &["ls-remote", "--heads", "origin", branch]).is_ok_and(|h| h.is_empty()) {
            return failed;
        }
        let since = git(
            root,
            &["fetch", "-q", "origin", &format!("refs/heads/{branch}")],
        )
        .and_then(|_| git(root, &["log", "-1", "--format=%cI", "FETCH_HEAD"]))
        .unwrap_or_else(|_| "an unknown time".to_owned());
        format!(
            "{branch} is being prepared, or was cut (branch on origin since {since}); withdraw \
             it first: `pubrel withdraw --unfinished` for a prepare that stopped, or \
             `pubrel withdraw PR` for a release's pull request"
        )
    })?;
    Ok(commit)
}

/// Whether `commit` is a claim: a commit [`claim`] made, which changes
/// nothing of its parent's tree.
fn is_claim(root: &Path, commit: &str) -> bool {
    let subject = git(root, &["log", "-1", "--format=%s", commit]).unwrap_or_default();
    subject.starts_with(CLAIM)
        && git(root, &["diff", "--quiet", &format!("{commit}^"), commit]).is_ok()
}

/// Delete the claim on `branch`, but only while it is still `claim`.
fn unclaim(root: &Path, branch: &str, claim: &str) -> Result<(), String> {
    let lease = format!("--force-with-lease=refs/heads/{branch}:{claim}");
    let target = format!(":refs/heads/{branch}");
    git(root, &["push", "-q", &lease, "origin", &target]).map(drop)
}

/// Make this prepare's worktree, detached at `base` and named by its
/// process, with its `pub` workspaces linked to the checkout's for each of
/// `corpora`.
///
/// # Errors
///
/// Returns a message if the worktree cannot be made, or a link cannot.
pub fn make(root: &Path, corpora: &[String], base: &str) -> Result<PathBuf, String> {
    let path = worktrees(root)?.join(format!("prepare-{}", std::process::id()));
    if path.exists() {
        remove(root, &path)?; // a prepare of an earlier process with this pid
    }
    let at = path.to_string_lossy().into_owned();
    git(root, &["worktree", "add", "-q", "--detach", &at, base])?;
    for corpus in corpora {
        let mine = root.join(corpus);
        let theirs = path.join(corpus);
        let workspace = theirs.join(".publet");
        std::fs::create_dir_all(&workspace).map_err(|e| format!("{}: {e}", workspace.display()))?;
        for name in SHARED {
            let target = mine.join(".publet").join(name);
            if target.exists() {
                link(&target, &workspace.join(name))?;
            }
        }
        // Out of git in every tree (the clone's exclude file is shared by its
        // worktrees), so the link is never committed; linked even when it is
        // not there yet, so a discard written in the worktree lands here.
        crate::withdraw::keep_out_of_git(root, &format!("{corpus}/.draft-discards"))?;
        link(
            &mine.join(".draft-discards"),
            &theirs.join(".draft-discards"),
        )?;
    }
    // The checkout's tools (the security checks run `.venv/bin/...`), linked.
    let venv = root.join(".venv");
    if venv.is_dir() {
        crate::withdraw::keep_out_of_git(root, ".venv")?;
        link_dir(&venv, &path.join(".venv"))?;
    }
    Ok(path)
}

/// Link `at` to the file `target`, which need not exist yet.
#[cfg(unix)]
fn link(target: &Path, at: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, at).map_err(|e| format!("{}: {e}", at.display()))
}

/// Link `at` to the file `target`. Windows makes the link only with
/// Developer Mode or the symlink privilege.
#[cfg(windows)]
fn link(target: &Path, at: &Path) -> Result<(), String> {
    std::os::windows::fs::symlink_file(target, at).map_err(|e| format!("{}: {e}", at.display()))
}

/// Link `at` to the directory `target`: on Unix, a link as any other.
#[cfg(unix)]
fn link_dir(target: &Path, at: &Path) -> Result<(), String> {
    link(target, at)
}

/// Link `at` to the directory `target`: Windows has a kind of link for
/// directories of its own.
#[cfg(windows)]
fn link_dir(target: &Path, at: &Path) -> Result<(), String> {
    std::os::windows::fs::symlink_dir(target, at).map_err(|e| format!("{}: {e}", at.display()))
}

/// Remove the worktree at `path`: its files and the links in it, never
/// what a link points at.
fn remove(root: &Path, path: &Path) -> Result<(), String> {
    if path.exists() {
        let at = path.to_string_lossy().into_owned();
        if git(root, &["worktree", "remove", "--force", &at]).is_err() {
            // Not a worktree git knows (any more): its directory, links unfollowed.
            std::fs::remove_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    git(root, &["worktree", "prune"]).map(drop)
}

/// End a prepare that worked in `tree`: the worktree removed; and when it
/// failed, the release branch it created deleted, and its claim on
/// `origin`, while the branch there is still that claim.
///
/// # Errors
///
/// Returns a message if a step fails, saying which.
pub fn end(root: &Path, tree: &Path, made: &Progress, failed: bool) -> Result<String, String> {
    remove(root, tree)?;
    if !failed {
        return Ok(String::new());
    }
    let mut undone = vec!["its worktree".to_owned()];
    if let Some(branch) = &made.branch {
        git(root, &["branch", "-q", "-D", branch])?;
        undone.push(format!("its branch {branch}"));
    }
    if let (Some(branch), Some(claim)) = (&made.branch, &made.claim) {
        unclaim(root, branch, claim)?;
        undone.push("its claim on origin".to_owned());
    } else if let Some(claim) = &made.claim {
        // Claimed, then failed before making the branch: the claim names it.
        let subject = git(root, &["log", "-1", "--format=%s", claim])?;
        if let Some(branch) = subject.strip_prefix(CLAIM) {
            unclaim(root, branch, claim)?;
            undone.push("its claim on origin".to_owned());
        }
    }
    Ok(format!(
        "undid the unfinished prepare: {} removed; this checkout was not touched",
        undone.join(", ")
    ))
}

/// Whether the process `pid` may still run: `kill -0` says, and where it
/// cannot tell (it does not run, or there is no `kill`), it may.
#[cfg(unix)]
fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .output()
        .map_or(true, |o| o.status.success())
}

/// Whether the process `pid` may still run: Windows has no `kill -0`, and a
/// worktree whose prepare cannot be told dead is left for the BRANCH form.
#[cfg(not(unix))]
fn alive(_pid: &str) -> bool {
    true
}

/// `pubrel withdraw --unfinished [BRANCH]`: clear what prepares that were
/// killed left. Without `branch`, only what this clone left: the worktrees
/// of prepares no longer running, the branches they were on, and the claims
/// those branches name; other claims on `origin` are listed, with who is
/// preparing them, and kept, since a prepare elsewhere may still run. With
/// `branch`, that release's worktree, branch and claim. Never a branch that
/// holds a release commit.
///
/// # Errors
///
/// Returns a message if a branch holds a release commit (then it is a
/// release to withdraw by its pull request), or nothing is left.
pub fn withdraw(root: &Path, branch: Option<&str>) -> Result<String, String> {
    git(root, &["fetch", "-q", "origin", "main"])?;
    let main = git(root, &["rev-parse", "origin/main"])?;
    let mut done = Vec::new();
    let mut branches: Vec<String> = branch.map(str::to_owned).into_iter().collect();
    if let Ok(entries) = std::fs::read_dir(worktrees(root)?) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let pid = name.strip_prefix("prepare-").unwrap_or_default();
            if !pid.is_empty() && alive(pid) {
                continue; // a prepare that still runs
            }
            let on =
                git(&entry.path(), &["symbolic-ref", "-q", "--short", "HEAD"]).unwrap_or_default();
            if branch.is_none() && on.starts_with("release/") {
                branches.push(on);
            }
            remove(root, &entry.path())?;
            done.push(format!("the worktree {name}"));
        }
    }
    for branch in &branches {
        let local = format!("refs/heads/{branch}");
        if git(root, &["show-ref", "--verify", "-q", &local]).is_ok() {
            if !is_claim(root, &local)
                && git(root, &["rev-list", "--count", &format!("{main}..{local}")])? != "0"
            {
                return Err(format!(
                    "{branch} holds a release commit: withdraw it by its branch or pull request"
                ));
            }
            git(root, &["branch", "-q", "-D", branch])?;
            done.push(format!("its branch {branch}"));
        }
        let remote = git(root, &["ls-remote", "--heads", "origin", branch])?;
        if let Some(tip) = remote.split_whitespace().next() {
            git(
                root,
                &["fetch", "-q", "origin", &format!("refs/heads/{branch}")],
            )?;
            if !is_claim(root, tip)
                && git(root, &["merge-base", "--is-ancestor", tip, &main]).is_err()
            {
                return Err(format!(
                    "{branch} on origin holds a release commit: withdraw it by its pull request"
                ));
            }
            unclaim(root, branch, tip)?;
            done.push(format!("its claim {branch} on origin"));
        }
    }
    // Claims no prepare here left: another clone's, maybe still running, or
    // one this clone made before its branch. Named, never deleted, unless
    // BRANCH names one.
    let others = if branch.is_none() {
        claims_elsewhere(root, &branches)?
    } else {
        Vec::new()
    };
    let left = if others.is_empty() {
        String::new()
    } else {
        format!(
            "\nleft alone, as no prepare here left them (name one to clear it: \
             `pubrel withdraw --unfinished BRANCH`):\n  {}",
            others.join("\n  ")
        )
    };
    if done.is_empty() {
        return Err(match branch {
            Some(b) => format!("nothing to undo: no worktree, branch or claim of {b}"),
            None => {
                format!("nothing to undo: no prepare here left a worktree, branch or claim{left}")
            }
        });
    }
    Ok(format!(
        "undid the unfinished prepare: {} removed{left}",
        done.join(", ")
    ))
}

/// The claims on `origin` other than `ours`, each with who is preparing it.
fn claims_elsewhere(root: &Path, ours: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let heads = git(root, &["ls-remote", "--heads", "origin", "release/*"])?;
    for line in heads.lines() {
        let mut parts = line.split_whitespace();
        let (Some(tip), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let name = name.trim_start_matches("refs/heads/");
        if ours.iter().any(|b| b == name) {
            continue;
        }
        git(
            root,
            &["fetch", "-q", "origin", &format!("refs/heads/{name}")],
        )?;
        if is_claim(root, tip) {
            let by = git(root, &["log", "-1", "--format=%b", tip])?;
            let by = by
                .lines()
                .find(|l| l.starts_with("Being prepared by"))
                .unwrap_or("");
            out.push(format!("{name}: {}", by.trim_end_matches('.')));
        }
    }
    Ok(out)
}
