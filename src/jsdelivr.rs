//! Serving a release through jsDelivr.
//!
//! jsDelivr serves any git tag of a public GitHub repository as soon as the
//! tag exists, at `cdn.jsdelivr.net/gh/OWNER/REPO@TAG/PATH`, so there is
//! nothing to publish. It fetches each file from GitHub the first time
//! anyone asks for it, though, and it caches what a version range such as
//! `@1` or `@1.2` resolves to. So after tagging, `pubrel tag` asks for
//! every listed file at the new tag, and purges the ranges that now
//! resolve to it. Neither step can undo the release, so a failure is a
//! warning: the files are served all the same, only more slowly at first.

use std::path::Path;
use std::process::Command;

use crate::version::Version;

/// The URLs to warm (at `tag`) and to purge (the ranges `version` falls
/// in, and the unversioned URL), for each of `files` in `repo`. Ranges
/// resolve only against plain `vX.Y.Z` or `X.Y.Z` tags, so a tag with any
/// other prefix is warmed and nothing is purged.
pub fn urls(
    repo: &str,
    tag: &str,
    version: &Version,
    files: &[String],
) -> (Vec<String>, Vec<String>) {
    let warm = files
        .iter()
        .map(|f| format!("https://cdn.jsdelivr.net/gh/{repo}@{tag}/{f}"))
        .collect();
    let plain = tag == version.to_string() || tag == format!("v{version}");
    let ranges = if plain {
        vec![
            format!("@{}", version.major),
            format!("@{}.{}", version.major, version.minor),
            String::new(),
        ]
    } else {
        Vec::new()
    };
    let purge = ranges
        .iter()
        .flat_map(|r| {
            files
                .iter()
                .map(move |f| format!("https://purge.jsdelivr.net/gh/{repo}{r}/{f}"))
        })
        .collect();
    (warm, purge)
}

/// The repository as `OWNER/REPO`: `GITHUB_REPOSITORY` in GitHub Actions,
/// otherwise what `gh` says the checkout at `root` is.
fn repository(root: &Path) -> Option<String> {
    if let Ok(repo) = std::env::var("GITHUB_REPOSITORY")
        && !repo.is_empty()
    {
        return Some(repo);
    }
    let out = Command::new("gh")
        .args([
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    let repo = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !repo.is_empty()).then_some(repo)
}

fn fetch(url: &str) -> Result<(), String> {
    let out = Command::new("curl")
        .args([
            "-fsS",
            "-o",
            "/dev/null",
            "--retry",
            "3",
            "--max-time",
            "120",
            url,
        ])
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// Warm `files` at `tag` and purge the ranges that resolve to it,
/// printing what was done; every failure is a warning on stderr.
pub fn refresh(root: &Path, tag: &str, version: &Version, files: &[String]) {
    if files.is_empty() {
        return;
    }
    let Some(repo) = repository(root) else {
        eprintln!(
            "warning: jsDelivr not refreshed: set GITHUB_REPOSITORY or run where `gh repo view` works"
        );
        return;
    };
    let (warm, purge) = urls(&repo, tag, version, files);
    let mut failed = 0;
    for url in warm.iter().chain(&purge) {
        if let Err(e) = fetch(url) {
            eprintln!("warning: jsDelivr: {url}: {e}");
            failed += 1;
        }
    }
    println!(
        "jsDelivr: warmed {} and purged {} URLs for {tag}{}",
        warm.len(),
        purge.len(),
        if failed > 0 {
            format!(" ({failed} failed)")
        } else {
            String::new()
        }
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // covers: jsdelivr::urls
    #[test]
    fn a_plain_tag_is_warmed_and_its_ranges_purged() {
        let v = crate::version::parse("1.2.3").unwrap();
        let (warm, purge) = urls("o/r", "v1.2.3", &v, &["app/a.js".to_owned()]);
        assert_eq!(warm, ["https://cdn.jsdelivr.net/gh/o/r@v1.2.3/app/a.js"]);
        assert_eq!(
            purge,
            [
                "https://purge.jsdelivr.net/gh/o/r@1/app/a.js",
                "https://purge.jsdelivr.net/gh/o/r@1.2/app/a.js",
                "https://purge.jsdelivr.net/gh/o/r/app/a.js",
            ]
        );
    }

    // covers: jsdelivr::urls
    #[test]
    fn a_prefixed_tag_is_only_warmed() {
        let v = crate::version::parse("0.3.0").unwrap();
        let (warm, purge) = urls("o/r", "core-v0.3.0", &v, &["a.js".to_owned()]);
        assert_eq!(warm, ["https://cdn.jsdelivr.net/gh/o/r@core-v0.3.0/a.js"]);
        assert!(purge.is_empty());
    }
}
