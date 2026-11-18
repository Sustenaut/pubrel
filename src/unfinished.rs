//! A prepare that did not finish, and one prepare at a time.
//!
//! `pubrel prepare` changes the tree before it commits: the package publet,
//! the manifest's version, the lock, the unreleased changes it folds in, and
//! the objects `pub` exports. A step that fails before the commit used to
//! leave all of that behind, with only a printed `git` line to undo it, and
//! a prepare that was killed left it too. Now a prepare that fails undoes
//! itself (`undo`), and `pubrel withdraw --unfinished` undoes one that was
//! killed: the tracked files as the commit it started from has them, the
//! untracked files under each corpus removed, the branch it made deleted,
//! and the checkout back where it was.
//!
//! A prepare holds a lock in the clone's git directory, which `git status`
//! never shows, so a second prepare in the same checkout, or a withdraw of
//! an unfinished one, stops while the first runs.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::withdraw::git;

/// The lock file of the clone at `root`, in its git directory.
fn lock_path(root: &Path) -> Result<PathBuf, String> {
    Ok(root.join(git(
        root,
        &["rev-parse", "--git-path", "pubrel-prepare.lock"],
    )?))
}

/// Whether the process `pid` still runs.
fn alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// What holds the lock, if a running process does: `(pid, since)`; a lock
/// whose process has ended is stale, and `None`.
fn holder(root: &Path) -> Result<Option<(String, String)>, String> {
    let path = lock_path(root)?;
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let mut words = text.split_whitespace();
    let pid = words.next().unwrap_or_default().to_owned();
    let since = words.next().unwrap_or_default().to_owned();
    Ok((!pid.is_empty() && alive(&pid)).then_some((pid, since)))
}

/// The prepare lock: removed when dropped, so a prepare that fails or
/// returns early leaves none.
pub struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Take the prepare lock of the clone at `root`, which `now` stamps.
///
/// # Errors
///
/// Returns a message naming the process when another prepare holds it, or
/// if the lock cannot be written.
pub fn lock(root: &Path, now: &str) -> Result<Lock, String> {
    if let Some((pid, since)) = holder(root)? {
        return Err(format!(
            "a prepare is running here (pid {pid}, since {since}): wait for it to end"
        ));
    }
    let path = lock_path(root)?;
    std::fs::write(&path, format!("{} {now}\n", std::process::id()))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Lock(path))
}

/// Where a prepare started: the branch it was on, or the commit when the
/// checkout was detached.
///
/// # Errors
///
/// Returns a message if git cannot say.
pub fn start(root: &Path) -> Result<String, String> {
    match git(root, &["symbolic-ref", "-q", "--short", "HEAD"]) {
        Ok(branch) if !branch.is_empty() => Ok(branch),
        _ => git(root, &["rev-parse", "HEAD"]),
    }
}

/// Undo a prepare that made `branch` from `start` and changed the tree but
/// did not commit: the tracked files back, the untracked files under each of
/// `corpora` removed, the checkout back at `start`, and `branch` deleted.
/// What it did, as a line to print.
///
/// # Errors
///
/// Returns a message if a git step fails, saying which.
pub fn undo(root: &Path, corpora: &[String], branch: &str, start: &str) -> Result<String, String> {
    // Staged too, when the commit itself failed: the index and the tree as the commit has them.
    git(root, &["reset", "-q", "--hard", "HEAD"])?;
    let mut clean = vec!["clean", "-q", "-fd", "--"];
    clean.extend(corpora.iter().map(String::as_str));
    git(root, &clean)?;
    let on = git(root, &["symbolic-ref", "-q", "--short", "HEAD"]).unwrap_or_default();
    if !branch.is_empty() && on == branch {
        let local = format!("refs/heads/{start}");
        if git(root, &["show-ref", "--verify", "-q", &local]).is_ok() {
            git(root, &["checkout", "-q", start])?;
        } else {
            git(root, &["checkout", "-q", "--detach", start])?;
        }
        git(root, &["branch", "-q", "-D", branch])?;
    }
    Ok(format!(
        "undid the unfinished prepare: the tree is as {start} has it, and {} removed",
        if branch.is_empty() {
            "its objects".to_owned()
        } else {
            format!("its objects and its branch {branch}")
        }
    ))
}

/// `pubrel withdraw --unfinished`: undo a prepare that was killed before
/// it committed. It is on its release branch, or wherever it started when
/// it had not made one yet.
///
/// # Errors
///
/// Returns a message if a prepare still runs here, if the release branch
/// already holds a commit of its own (then it is a release to withdraw by
/// its branch), or if there is nothing to undo.
pub fn withdraw(root: &Path, corpora: &[String]) -> Result<String, String> {
    if let Some((pid, since)) = holder(root)? {
        return Err(format!(
            "a prepare is running here (pid {pid}, since {since}): it is not unfinished yet"
        ));
    }
    let on = git(root, &["symbolic-ref", "-q", "--short", "HEAD"]).unwrap_or_default();
    let (branch, start) = if on.starts_with("release/") {
        let own = git(root, &["rev-list", "--count", "origin/main..HEAD"])?;
        if own != "0" {
            return Err(format!(
                "{on} holds a commit: it is a release cut, so withdraw it by its branch or \
                 pull request"
            ));
        }
        // Back to main when the branch was made from it, as prepare makes it.
        let at = git(root, &["rev-parse", "HEAD"])?;
        let main =
            git(root, &["rev-parse", "-q", "--verify", "refs/heads/main"]).unwrap_or_default();
        (on, if main == at { "main".to_owned() } else { at })
    } else {
        (String::new(), start(root)?)
    };
    if branch.is_empty() && git(root, &["status", "--porcelain"])?.is_empty() {
        return Err(
            "nothing to undo: the tree is clean and no release branch is checked out".to_owned(),
        );
    }
    let _ = std::fs::remove_file(lock_path(root)?);
    undo(root, corpora, &branch, &start)
}
