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

/// The worktree a prepare of `branch` works in, under the clone's git
/// directory.
///
/// # Errors
///
/// Returns a message if git cannot say where its directory is.
pub fn worktree_path(root: &Path, branch: &str) -> Result<PathBuf, String> {
    let common = root.join(git(root, &["rev-parse", "--git-common-dir"])?);
    Ok(common.join("pubrel").join(branch.replace('/', "-")))
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
    git(root, &["push", "-q", &lease, "origin", &target]).map_err(|_| {
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

/// Make the worktree of `branch` at `base`, on a new local branch, with its
/// `pub` workspaces linked to the checkout's for each of `corpora`.
///
/// # Errors
///
/// Returns a message if the branch or the worktree already exists here
/// (an unfinished prepare: `pubrel withdraw --unfinished`), or a link
/// cannot be made.
pub fn make(root: &Path, corpora: &[String], branch: &str, base: &str) -> Result<PathBuf, String> {
    let path = worktree_path(root, branch)?;
    if path.exists() {
        return Err(format!(
            "{} is left from a prepare that did not finish: `pubrel withdraw --unfinished`",
            path.display()
        ));
    }
    let at = path.to_string_lossy().into_owned();
    git(root, &["worktree", "add", "-q", "-b", branch, &at, base])?;
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
        // worktrees), so the link is never committed.
        crate::withdraw::keep_out_of_git(root, &format!("{corpus}/.draft-discards"))?;
        // Linked even when it is not there yet: a discard written in the
        // worktree then lands in this checkout's list.
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

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, at).map_err(|e| format!("{}: {e}", at.display()))
}

#[cfg(windows)]
fn link(target: &Path, at: &Path) -> Result<(), String> {
    std::os::windows::fs::symlink_file(target, at).map_err(|e| format!("{}: {e}", at.display()))
}

#[cfg(unix)]
fn link_dir(target: &Path, at: &Path) -> Result<(), String> {
    link(target, at)
}

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

/// End a prepare of `branch`: its worktree removed; and when it failed, its
/// local branch deleted and its claim on `origin`, if it made one, deleted
/// while the branch there is still that claim.
///
/// # Errors
///
/// Returns a message if a step fails, saying which.
pub fn end(
    root: &Path,
    branch: &str,
    failed: bool,
    claimed: Option<&str>,
) -> Result<String, String> {
    remove(root, &worktree_path(root, branch)?)?;
    if !failed {
        return Ok(String::new());
    }
    let local = format!("refs/heads/{branch}");
    if git(root, &["show-ref", "--verify", "-q", &local]).is_ok() {
        git(root, &["branch", "-q", "-D", branch])?;
    }
    if let Some(claim) = claimed {
        unclaim(root, branch, claim)?;
    }
    Ok(format!(
        "undid the unfinished prepare: its worktree and its branch {branch}{} removed; this \
         checkout was not touched",
        if claimed.is_some() {
            ", and its claim on origin,"
        } else {
            ""
        }
    ))
}

/// `pubrel withdraw --unfinished`: clear what a prepare that was killed
/// left for `branch`, the release the unreleased changes make: its
/// worktree, its local branch and its claim on `origin`.
///
/// # Errors
///
/// Returns a message if the branch holds a release commit (then it is a
/// release to withdraw by its pull request), or nothing is left.
pub fn withdraw(root: &Path, branch: &str) -> Result<String, String> {
    git(root, &["fetch", "-q", "origin", "main"])?;
    let main = git(root, &["rev-parse", "origin/main"])?;
    let mut done = Vec::new();
    let leftover = worktree_path(root, branch)?;
    if leftover.exists() {
        remove(root, &leftover)?;
        done.push("its worktree".to_owned());
    }
    let local = format!("refs/heads/{branch}");
    if git(root, &["show-ref", "--verify", "-q", &local]).is_ok() {
        if git(root, &["rev-list", "--count", &format!("{main}..{local}")])? != "0" {
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
        if !is_claim(root, tip) && git(root, &["merge-base", "--is-ancestor", tip, &main]).is_err()
        {
            return Err(format!(
                "{branch} on origin holds a release commit: withdraw it by its pull request"
            ));
        }
        unclaim(root, branch, tip)?;
        done.push(format!("its claim {branch} on origin"));
    }
    if done.is_empty() {
        return Err(format!(
            "nothing to undo: no worktree, branch or claim of {branch}"
        ));
    }
    Ok(format!(
        "undid the unfinished prepare: {} removed",
        done.join(", ")
    ))
}
