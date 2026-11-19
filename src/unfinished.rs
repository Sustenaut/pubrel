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
//! Each prepare has a folder of its own, `.git/pubrel/prepare-PID/`, which
//! holds its record (`prepare.json`: pid, token, branch, claim, and every
//! link it made) and the worktree, nested deep enough (`tree`, or
//! `u/…/tree`) that each parent corpus `corpus.json` names by a relative
//! path (`../../publet-corpus`) resolves inside that folder, where a link
//! to the checkout's own parent sits. `corpus.json` is never edited. The
//! record is written before anything else and outlives git's worktree
//! record, so `pubrel withdraw --unfinished` knows what a dead prepare
//! made, and touches nothing else under `.git/pubrel/`.
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

/// A short random token for a claim, eight hex digits: std's hasher keys
/// are random per process, so this needs no crate and names nothing about
/// the machine.
#[must_use]
pub fn token() -> String {
    use std::hash::{BuildHasher as _, Hasher as _};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    format!("{:08x}", hasher.finish() & 0xffff_ffff)
}

/// The first line of a claim commit's message.
const CLAIM: &str = "pubrel: claim ";

/// Claim `branch` on `origin`: a commit of its own on `base`, with
/// `base`'s tree, naming its token, pid and time (never the host), pushed
/// with an empty lease.
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

/// A prepare's record: what it made, so that it, or `withdraw
/// --unfinished` after it was killed, undoes exactly that.
#[derive(Default)]
pub struct Record {
    /// The process the prepare ran in.
    pub pid: u32,
    /// The token its claim names.
    pub token: String,
    /// Its worktree, as an absolute path.
    pub tree: PathBuf,
    /// Every link it made, as an absolute path: removed, never followed.
    pub links: Vec<PathBuf>,
    /// The release branch it planned, once it did: written before it claims.
    pub branch: Option<String>,
    /// Whether it created that branch here.
    pub created: bool,
    /// The claim commit it pushed, once the push succeeded.
    pub claim: Option<String>,
}

/// A prepare in progress: its folder and its record, kept on disk as it
/// changes.
pub struct Prepared {
    /// `.git/pubrel/prepare-PID/`.
    pub dir: PathBuf,
    /// What it made.
    pub record: Record,
}

impl Prepared {
    /// Write the record, whole, by a rename, so a prepare killed at any
    /// point leaves a record that reads.
    ///
    /// # Errors
    ///
    /// Returns a message if it cannot be written.
    pub fn save(&self) -> Result<(), String> {
        save(&self.dir, &self.record)
    }
}

const RECORD: &str = "prepare.json";

impl Record {
    /// As JSON: a `serde_json::Value`, so pubrel needs no derive.
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "pid": self.pid,
            "token": self.token,
            "tree": self.tree.to_string_lossy(),
            "links": self.links.iter().map(|l| l.to_string_lossy().into_owned()).collect::<Vec<_>>(),
            "branch": self.branch,
            "created": self.created,
            "claim": self.claim,
        })
    }

    fn from_json(v: &serde_json::Value) -> Option<Self> {
        let text = |k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Some(Self {
            pid: u32::try_from(v.get("pid")?.as_u64()?).ok()?,
            token: text("token")?,
            tree: PathBuf::from(text("tree")?),
            links: v
                .get("links")?
                .as_array()?
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(PathBuf::from)
                .collect(),
            branch: text("branch"),
            created: v
                .get("created")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            claim: text("claim"),
        })
    }
}

fn save(dir: &Path, record: &Record) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&record.to_json()).map_err(|e| e.to_string())? + "\n";
    let temp = dir.join(format!("{RECORD}.new"));
    std::fs::write(&temp, text).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, dir.join(RECORD)).map_err(|e| format!("{}: {e}", dir.display()))
}

fn load(dir: &Path) -> Option<Record> {
    let text = std::fs::read_to_string(dir.join(RECORD)).ok()?;
    Record::from_json(&serde_json::from_str(&text).ok()?)
}

/// How many levels above the repository's root the relative `path` reaches,
/// from the corpus directory `corpus` (relative to the root): 0 for one
/// that stays inside.
fn levels_above(corpus: &str, path: &str) -> usize {
    let mut depth: isize = Path::new(corpus)
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .count()
        .try_into()
        .unwrap_or(isize::MAX);
    let mut lowest = 0;
    for part in Path::new(path).components() {
        match part {
            std::path::Component::ParentDir => depth -= 1,
            std::path::Component::Normal(_) => depth += 1,
            _ => {}
        }
        lowest = lowest.min(depth);
    }
    usize::try_from(-lowest).unwrap_or(0)
}

/// `base`'s path joined with `rel`, `..` taken lexically: where a relative
/// path from a directory lands, whether or not it exists.
fn lexical(base: &Path, rel: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in Path::new(rel).components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::Normal(p) => out.push(p),
            _ => {}
        }
    }
    out
}

/// The relative parent paths a corpus's `corpus.json` at `base` names.
fn relative_parents(root: &Path, base: &str, corpus: &str) -> Vec<String> {
    let text = git(root, &["show", &format!("{base}:{corpus}/corpus.json")]).unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    json.get("parents")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get("path").and_then(serde_json::Value::as_str))
        .filter(|p| Path::new(p).is_relative())
        .map(str::to_owned)
        .collect()
}

/// Make this prepare's folder and worktree, detached at `base`: the record
/// first, then the worktree, nested so each relative parent of `corpora`
/// lands in the folder, then the links: the `pub` workspaces, the
/// discards, `.venv`, and each relative parent, to the checkout's own.
///
/// # Errors
///
/// Returns a message if the worktree cannot be made, or a link cannot.
pub fn make(root: &Path, corpora: &[String], base: &str, token: &str) -> Result<Prepared, String> {
    let dir = worktrees(root)?.join(format!("prepare-{}", std::process::id()));
    if dir.exists() {
        clear(root, &dir)?; // a prepare of an earlier process with this pid
    }
    let parents: Vec<(String, String)> = corpora
        .iter()
        .flat_map(|c| {
            relative_parents(root, base, c)
                .into_iter()
                .map(move |p| (c.clone(), p))
        })
        .collect();
    let up = parents
        .iter()
        .map(|(c, p)| levels_above(c, p))
        .max()
        .unwrap_or(0);
    let mut tree = dir.clone();
    for _ in 1..up {
        tree.push("u");
    }
    tree.push("tree");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut made = Prepared {
        dir,
        record: Record {
            pid: std::process::id(),
            token: token.to_owned(),
            tree: tree.clone(),
            ..Record::default()
        },
    };
    made.save()?;
    if let Some(up) = tree.parent() {
        std::fs::create_dir_all(up).map_err(|e| format!("{}: {e}", up.display()))?;
    }
    let at = tree.to_string_lossy().into_owned();
    git(root, &["worktree", "add", "-q", "--detach", &at, base])?;
    for corpus in corpora {
        let mine = root.join(corpus);
        let theirs = tree.join(corpus);
        let workspace = theirs.join(".publet");
        std::fs::create_dir_all(&workspace).map_err(|e| format!("{}: {e}", workspace.display()))?;
        for name in SHARED {
            let target = mine.join(".publet").join(name);
            if target.exists() {
                made.link(root, &target, &workspace.join(name), false)?;
            }
        }
        // Linked even when it is not there yet: a discard written in the
        // worktree then lands in this checkout's list.
        made.link(
            root,
            &mine.join(".draft-discards"),
            &theirs.join(".draft-discards"),
            false,
        )?;
    }
    // The checkout's tools (the security checks run `.venv/bin/...`).
    let venv = root.join(".venv");
    if venv.is_dir() {
        made.link(root, &venv, &tree.join(".venv"), true)?;
    }
    // Each relative parent, where it lands from the worktree's corpus: in
    // this prepare's folder, linked to where it lands from the checkout's.
    for (corpus, parent) in &parents {
        let at = lexical(&tree.join(corpus), parent);
        if at.starts_with(&tree) || at.exists() || at.symlink_metadata().is_ok() {
            continue; // inside the repository, or not this prepare's to make
        }
        let target = lexical(&root.join(corpus), parent);
        if let Some(up) = at.parent() {
            std::fs::create_dir_all(up).map_err(|e| format!("{}: {e}", up.display()))?;
        }
        made.link(root, &target, &at, true)?;
    }
    Ok(made)
}

impl Prepared {
    /// Make the link `at` to `target`, recorded before it is made, and kept
    /// out of git by its exact path when it is in the worktree.
    fn link(&mut self, root: &Path, target: &Path, at: &Path, dir: bool) -> Result<(), String> {
        if let Ok(rel) = at.strip_prefix(&self.record.tree) {
            exclude_exactly(root, &rel.to_string_lossy())?;
        }
        self.record.links.push(at.to_path_buf());
        self.save()?;
        if dir {
            link_dir(target, at)
        } else {
            link(target, at)
        }
    }
}

/// Keep the path `rel` out of git by a line of its own in the clone's
/// exclude file (shared by its worktrees): `/rel`, which matches a link as
/// it matches a file, where a pattern such as `.venv/` matches only a
/// directory.
fn exclude_exactly(root: &Path, rel: &str) -> Result<(), String> {
    let exclude = root.join(git(root, &["rev-parse", "--git-path", "info/exclude"])?);
    let line = format!("/{}", rel.replace('\\', "/"));
    let mut text = std::fs::read_to_string(&exclude).unwrap_or_default();
    if text.lines().any(|l| l == line) {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&line);
    text.push('\n');
    if let Some(up) = exclude.parent() {
        std::fs::create_dir_all(up).map_err(|e| format!("{}: {e}", up.display()))?;
    }
    std::fs::write(&exclude, text).map_err(|e| format!("{}: {e}", exclude.display()))
}

/// The links among what is staged in `tree`, by path: a release commits
/// none.
///
/// # Errors
///
/// Returns a message if git cannot say.
pub fn staged_links(tree: &Path) -> Result<Vec<String>, String> {
    let raw = git(tree, &["diff", "--cached", "--raw", "--no-renames"])?;
    Ok(raw
        .lines()
        .filter_map(|l| {
            let (meta, path) = l.split_once('\t')?;
            meta.split_whitespace()
                .nth(1)
                .filter(|m| *m == "120000")
                .map(|_| path.to_owned())
        })
        .collect())
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

/// Clear the prepare folder `dir`: its worktree, each link its record names
/// (a link only, never what it points at, and only while it is a link),
/// the record, and the folder once nothing else is in it. A folder of
/// pubrel 0.8's layout, its worktree directly in `dir`, is that worktree.
fn clear(root: &Path, dir: &Path) -> Result<(), String> {
    let Some(record) = load(dir) else {
        if dir.join(".git").is_file() {
            return remove(root, dir); // 0.8's layout: the folder is the worktree
        }
        return Ok(()); // no record: not this prepare's to touch
    };
    for at in record.links.iter().rev() {
        if at
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            remove_link(at)?;
        }
    }
    remove(root, &record.tree)?;
    let _ = std::fs::remove_file(dir.join(RECORD));
    // The folders it made, from the deepest, while empty.
    let mut empty: Vec<PathBuf> = record
        .links
        .iter()
        .chain(std::iter::once(&record.tree))
        .flat_map(|p| {
            p.ancestors()
                .skip(1)
                .map(Path::to_path_buf)
                .collect::<Vec<_>>()
        })
        .filter(|p| p.starts_with(dir))
        .collect();
    empty.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    empty.dedup();
    for folder in empty {
        let _ = std::fs::remove_dir(folder);
    }
    Ok(())
}

#[cfg(unix)]
fn remove_link(at: &Path) -> Result<(), String> {
    std::fs::remove_file(at).map_err(|e| format!("{}: {e}", at.display()))
}

#[cfg(windows)]
fn remove_link(at: &Path) -> Result<(), String> {
    std::fs::remove_file(at)
        .or_else(|_| std::fs::remove_dir(at))
        .map_err(|e| format!("{}: {e}", at.display()))
}

/// End the prepare `made`: its folder cleared; and when it failed, the
/// release branch it created deleted, and its claim on `origin`, while the
/// branch there is still that claim.
///
/// # Errors
///
/// Returns a message if a step fails, saying which.
pub fn end(root: &Path, made: &Prepared, failed: bool) -> Result<String, String> {
    let record = &made.record;
    let mut undone = vec!["its worktree".to_owned()];
    clear(root, &made.dir)?; // the worktree first: its branch is then free to go
    if failed {
        if let (Some(branch), true) = (&record.branch, record.created) {
            git(root, &["branch", "-q", "-D", branch])?;
            undone.push(format!("its branch {branch}"));
        }
        if let (Some(branch), Some(claim)) = (&record.branch, &record.claim) {
            unclaim(root, branch, claim)?;
            undone.push("its claim on origin".to_owned());
        }
    }
    Ok(if failed {
        format!(
            "undid the unfinished prepare: {} removed; this checkout was not touched",
            undone.join(", ")
        )
    } else {
        String::new()
    })
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
    let branch_named = branch.is_some();
    let mut done = Vec::new();
    let mut branches: Vec<String> = branch.map(str::to_owned).into_iter().collect();
    let tokens = dead_prepares(root, branch, &mut done, &mut branches)?;
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
            // Without BRANCH, only a claim that names a dead prepare's token here.
            let mine = tokens.iter().any(|(b, t)| {
                b == branch
                    && git(root, &["log", "-1", "--format=%b", tip])
                        .is_ok_and(|body| body.contains(&format!("token {t},")))
            });
            if is_claim(root, tip) && !mine && !branch_named {
                continue;
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

/// Clear the folders of prepares here that no longer run (all of them, or,
/// with `branch`, the one on it), noting each in `done` and the branches
/// they were on in `branches`: their tokens, by branch, returned. Nothing
/// else under `.git/pubrel/` is touched: an entry not named `prepare-PID`,
/// a link, or a folder with no record and no worktree.
fn dead_prepares(
    root: &Path,
    branch: Option<&str>,
    done: &mut Vec<String>,
    branches: &mut Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let mut tokens: Vec<(String, String)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(worktrees(root)?) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(pid) = name.strip_prefix("prepare-") else {
                continue; // not a prepare's: left as it is
            };
            if !entry.path().is_dir()
                || entry
                    .path()
                    .symlink_metadata()
                    .is_ok_and(|m| m.file_type().is_symlink())
                || alive(pid)
            {
                continue; // a link someone made, or a prepare that still runs
            }
            let record = load(&entry.path());
            let legacy = entry.path().join(".git").is_file(); // 0.8's layout: the folder is the worktree
            if record.is_none() && !legacy {
                continue; // neither a record nor a worktree: not a prepare's
            }
            let on = record.as_ref().map_or_else(
                || {
                    git(&entry.path(), &["symbolic-ref", "-q", "--short", "HEAD"])
                        .unwrap_or_default()
                },
                |r| r.branch.clone().unwrap_or_default(),
            );
            if let Some(r) = record.as_ref().filter(|_| !on.is_empty()) {
                tokens.push((on.clone(), r.token.clone()));
            }
            let ours = branch.is_none_or(|b| b == on);
            if branch.is_none() && on.starts_with("release/") && !branches.contains(&on) {
                branches.push(on);
            }
            if ours {
                clear(root, &entry.path())?;
                done.push(format!("the worktree {name}"));
            }
        }
    }
    Ok(tokens)
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
