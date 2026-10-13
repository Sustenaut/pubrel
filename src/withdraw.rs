//! Withdrawing a release that was cut but never merged.
//!
//! `pubrel prepare` publishes the package publet into `pub`'s local store,
//! then commits what it exported to `objects/` on a release branch. Closing
//! that release's pull request removes the objects from git, but not from
//! the store. And `pub` exports every object it holds that `objects/` lacks,
//! except those listed in the corpus's `.draft-discards`. So the next
//! `prepare` would publish the withdrawn release's records beside the new
//! one, and both would be in the package publet for good.
//!
//! `pubrel withdraw` lists the objects a release commit added in
//! `.draft-discards`, and keeps that file out of git, so they are never
//! exported again. As a backstop, `prepare` and `check` refuse a release
//! that carries records naming any other commit.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use crate::record::Config;

/// The commit a release record names: `… from commit <sha> …`.
#[must_use]
pub fn commit_named(content: &str) -> Option<&str> {
    let rest = content.split(" from commit ").nth(1)?;
    let word = rest.split_whitespace().next()?;
    Some(word.trim_end_matches('.'))
}

/// The release records among `objects` that name a commit other than
/// `commit`: each as its content. `content_of` reads an object's content,
/// if it is a claim.
pub fn strays<'a>(
    objects: impl IntoIterator<Item = &'a str>,
    commit: &str,
    content_of: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    objects
        .into_iter()
        .filter_map(&content_of)
        .filter(|c| commit_named(c).is_some_and(|named| named != commit))
        .collect()
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The identifiers of the object files `paths` names.
fn cids_of(paths: &str) -> Vec<String> {
    paths
        .lines()
        .filter_map(|p| Path::new(p).file_name()?.to_str()?.strip_suffix(".cbor"))
        .map(|stem| stem.replace('_', ":"))
        .collect()
}

/// Withdraw the release `which` names: a pull request number (`23`, `#23`),
/// fetched from `origin`, or any git revision, such as a release branch. A
/// pull request must be closed and unmerged, and its branch is then
/// deleted, from `origin` and from this clone, unless it is a fork's.
///
/// # Errors
///
/// Returns a message if a pull request is open or merged, or `gh` cannot
/// describe it; if the revision cannot be resolved, is already part of
/// this branch's history, adds no objects, or adds no release record; or
/// if `.draft-discards` or git's exclude file cannot be written.
pub fn withdraw(cfg: &Config, which: &str) -> Result<String, String> {
    let root = &cfg.root;
    let number = which.trim_start_matches('#').parse::<u64>().ok();
    let pr = number.map(|n| pull_request(root, n)).transpose()?;
    let rev = match number {
        Some(number) => {
            let reference = format!("refs/pubrel/withdrawn/{number}");
            git(
                root,
                &[
                    "fetch",
                    "-q",
                    "origin",
                    &format!("pull/{number}/head:{reference}"),
                ],
            )?;
            reference
        }
        None => which.to_owned(),
    };
    let commit = git(
        root,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    )
    .map_err(|_| format!("{which} names no commit here"))?;
    if git(root, &["merge-base", "--is-ancestor", &commit, "HEAD"]).is_ok() {
        return Err(format!(
            "{which} is part of this branch's history: a merged release cannot be withdrawn"
        ));
    }
    let base = git(root, &["merge-base", "HEAD", &commit])?;
    let objects_dir = cfg.corpus.join("objects");
    let objects_rel = objects_dir.to_string_lossy();
    let added = cids_of(&git(
        root,
        &[
            "diff",
            "--name-only",
            "--diff-filter=A",
            &base,
            &commit,
            "--",
            &objects_rel,
        ],
    )?);
    if added.is_empty() {
        return Err(format!("{which} adds no objects to {objects_rel}"));
    }
    let content_at = |cid: &str| -> Option<String> {
        crate::published::claim_content(&object_at(root, &commit, &objects_rel, cid)?)
    };
    let records: Vec<String> = added
        .iter()
        .filter_map(|c| content_at(c))
        .filter(|c| commit_named(c).is_some())
        .collect();
    let Some(record) = records.first() else {
        return Err(format!(
            "{which} adds no release record: it is not a release"
        ));
    };

    let (kept, added): (Vec<String>, Vec<String>) = added
        .into_iter()
        .partition(|c| corpus_identity(object_at(root, &commit, &objects_rel, c).as_deref()));
    let discards = root.join(&cfg.corpus).join(".draft-discards");
    let fresh = list_discards(&discards, &format!("withdrawn {which}"), &added)?;
    let rel = discards
        .strip_prefix(root)
        .unwrap_or(&discards)
        .to_string_lossy()
        .into_owned();
    keep_out_of_git(root, &rel)?;
    let removed = match &pr {
        Some(pr) if !pr.fork => remove_branch(root, &pr.branch)?,
        _ => String::new(),
    };
    let kept = if kept.is_empty() {
        String::new()
    } else {
        format!(
            "; {} key or rotation object(s) kept, which belong to the corpus, not the release",
            kept.len()
        )
    };
    Ok(format!(
        "withdrew {which} ({record}): {} object(s) listed in {rel}, {} of them already; \
         pub will not export them again{kept}{removed}",
        added.len(),
        added.len() - fresh
    ))
}

/// The bytes of the object `cid` in `objects_rel` at `commit`.
fn object_at(root: &Path, commit: &str, objects_rel: &str, cid: &str) -> Option<Vec<u8>> {
    let path = format!("{objects_rel}/{}.cbor", cid.replace(':', "_"));
    Command::new("git")
        .args(["show", &format!("{commit}:{path}")])
        .current_dir(root)
        .output()
        .ok()
        .map(|o| o.stdout)
}

/// Whether an object is the corpus's identity rather than a release's: a
/// key, or a signature that rotates one. The first release exports the
/// corpus's key, and the release cut again in its place needs it too.
fn corpus_identity(bytes: Option<&[u8]>) -> bool {
    bytes
        .and_then(crate::published::kind_and_purpose)
        .is_some_and(|(kind, purpose)| {
            kind == "key" || (kind == "sig" && purpose.as_deref() == Some("rotation"))
        })
}

/// What `gh` says of a pull request.
struct PullRequest {
    branch: String,
    /// Whether its branch lives in another repository, which is not ours
    /// to delete.
    fork: bool,
}

/// The pull request `number`, which must be closed and unmerged.
fn pull_request(root: &Path, number: u64) -> Result<PullRequest, String> {
    let out = Command::new("gh")
        .args([
            "pr",
            "view",
            &number.to_string(),
            "--json",
            "state,headRefName,isCrossRepository",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run gh to look up pull request #{number}: {e}"))?;
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        format!(
            "gh could not describe pull request #{number}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )
    })?;
    let text = |k: &str| {
        json.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    };
    match text("state") {
        "CLOSED" => Ok(PullRequest {
            branch: text("headRefName").to_owned(),
            fork: json
                .get("isCrossRepository")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        }),
        "MERGED" => Err(format!(
            "pull request #{number} is merged: a merged release cannot be withdrawn"
        )),
        "OPEN" => Err(format!(
            "pull request #{number} is still open: close it, unmerged, then withdraw it"
        )),
        other => Err(format!(
            "pull request #{number} is in a state pubrel does not know: {other:?}"
        )),
    }
}

/// Delete a withdrawn release's branch from `origin` and from this clone,
/// where it still exists, so the next release can use its name; say what
/// was deleted. The objects are already discarded, and the pull request
/// keeps its commits.
fn remove_branch(root: &Path, branch: &str) -> Result<String, String> {
    if branch.is_empty() {
        return Ok(String::new());
    }
    let mut removed = Vec::new();
    if git(
        root,
        &["ls-remote", "--exit-code", "--heads", "origin", branch],
    )
    .is_ok()
    {
        git(root, &["push", "-q", "origin", "--delete", branch])?;
        removed.push("origin");
    }
    let local = format!("refs/heads/{branch}");
    let current = git(root, &["symbolic-ref", "-q", "HEAD"]).unwrap_or_default();
    if current != local && git(root, &["show-ref", "--verify", "-q", &local]).is_ok() {
        git(root, &["branch", "-q", "-D", branch])?;
        removed.push("this clone");
    }
    Ok(if removed.is_empty() {
        String::new()
    } else {
        format!(
            "; deleted its branch {branch} from {}",
            removed.join(" and ")
        )
    })
}

/// Add each of `cids` not already listed to the discards file at `path`,
/// under `label`; return how many were added.
fn list_discards(path: &Path, label: &str, cids: &[String]) -> Result<usize, String> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let listed: Vec<&str> = existing
        .lines()
        .filter_map(|l| l.split_once('=').map(|(_, c)| c.trim()))
        .collect();
    // pub reads the identifier after the first `=`, so the label has none.
    let label = label.replace('=', "-");
    let mut text = existing.clone();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    let mut added = 0;
    for cid in cids.iter().filter(|c| !listed.contains(&c.as_str())) {
        let _ = writeln!(text, "{label}={cid}");
        added += 1;
    }
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(added)
}

/// Keep `rel` out of git with the clone's own exclude file, unless it is
/// already ignored: the list is local, since other clones never held the
/// objects it names.
fn keep_out_of_git(root: &Path, rel: &str) -> Result<(), String> {
    if git(root, &["check-ignore", "-q", rel]).is_ok() {
        return Ok(());
    }
    let exclude = root.join(git(root, &["rev-parse", "--git-path", "info/exclude"])?);
    let mut lines = std::fs::read_to_string(&exclude).unwrap_or_default();
    if !lines.is_empty() && !lines.ends_with('\n') {
        lines.push('\n');
    }
    let _ = writeln!(lines, "/{rel}");
    std::fs::write(&exclude, lines).map_err(|e| format!("{}: {e}", exclude.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: withdraw::commit_named, withdraw::strays
    #[test]
    fn records_name_their_commit_and_others_are_strays() {
        let identity = "pubrel 0.5.0 is released as v0.5.0 from commit abc123.";
        let release =
            "pubrel 0.5.0 was released on 2026-10-09 from commit abc123 with these changes.";
        assert_eq!(commit_named(identity), Some("abc123"));
        assert_eq!(commit_named(release), Some("abc123"));
        assert_eq!(commit_named("pubrel 0.5.0 in brief: from here on."), None);
        let contents = [
            identity.to_owned(),
            "pubrel 0.5.0 was released on 2026-10-09 from commit def456 with these changes."
                .to_owned(),
        ];
        let found = strays(["a", "b"], "abc123", |id| {
            Some(contents[usize::from(id == "b")].clone())
        });
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("def456"));
    }
}
