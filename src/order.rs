//! The order `prepare --all` releases several packages in: a package after
//! every package it depends on.
//!
//! Releasing a crate pins its new version in the workspace that depends on
//! it, and refreshes `Cargo.lock`. Cut in dependency order, each release
//! then builds against the versions its in-repository dependencies are
//! released at in the same pull request, and the lockfile records them.
//!
//! A package's crates are its own: a `cargo-package` is the crate at its
//! manifest, and a `cargo-workspace` is every member that no
//! `cargo-package` in `release.json` releases on its own. Only normal and
//! build dependencies order releases. A dev-dependency is not published
//! with the crate, and two crates may each dev-depend on the other.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::manifest::Kind;
use crate::record::Config;

/// The crates of a workspace, from `cargo metadata --no-deps`: each one's
/// name, the directory of its manifest, and the names it depends on to
/// build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The crate's name.
    pub name: String,
    /// The directory holding its `Cargo.toml`.
    pub dir: PathBuf,
    /// The names of its normal and build dependencies.
    pub depends_on: BTreeSet<String>,
}

/// The members `cargo metadata --no-deps --format-version 1` describes.
///
/// # Errors
///
/// Returns a message if the text is not JSON or lists no `packages`.
pub fn members(metadata: &str) -> Result<Vec<Member>, String> {
    let json: Value = serde_json::from_str(metadata).map_err(|e| format!("cargo metadata: {e}"))?;
    let packages = json
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("cargo metadata lists no packages")?;
    Ok(packages
        .iter()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?.to_owned();
            let dir = Path::new(p.get("manifest_path")?.as_str()?)
                .parent()?
                .to_path_buf();
            let depends_on = p
                .get("dependencies")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|d| d.get("kind").and_then(Value::as_str) != Some("dev"))
                .filter_map(|d| d.get("name")?.as_str().map(str::to_owned))
                .collect();
            Some(Member {
                name,
                dir,
                depends_on,
            })
        })
        .collect())
}

/// The crates each package releases, by its position in `packages`.
fn crates_of(packages: &[&Config], members: &[Member]) -> Vec<BTreeSet<String>> {
    let own_dirs: Vec<PathBuf> = packages
        .iter()
        .filter(|c| c.kind == Kind::CargoPackage)
        .map(|c| c.root.join(c.manifest_dir()))
        .collect();
    packages
        .iter()
        .map(|cfg| match cfg.kind {
            Kind::CargoPackage => {
                let dir = cfg.root.join(cfg.manifest_dir());
                members
                    .iter()
                    .filter(|m| m.dir == dir)
                    .map(|m| m.name.clone())
                    .collect()
            }
            Kind::CargoWorkspace => members
                .iter()
                .filter(|m| !own_dirs.contains(&m.dir))
                .map(|m| m.name.clone())
                .collect(),
            Kind::Pyproject | Kind::Npm => BTreeSet::new(),
        })
        .collect()
}

/// `packages` in an order that puts each after every package it depends
/// on. Packages with no order between them keep the order `release.json`
/// lists them in. `all` is every package `release.json` lists, so that a
/// workspace's crates exclude those released on their own even when only
/// the workspace is being released.
///
/// # Errors
///
/// Returns a message naming the packages if they depend on each other in
/// a cycle.
pub fn by_dependency<'a>(
    packages: &[&'a Config],
    all: &[Config],
    members: &[Member],
) -> Result<Vec<&'a Config>, String> {
    let every: Vec<&Config> = all.iter().collect();
    let crates_all = crates_of(&every, members);
    let crates: Vec<BTreeSet<String>> = packages
        .iter()
        .map(|p| {
            every
                .iter()
                .position(|c| c.name == p.name)
                .and_then(|i| crates_all.get(i).cloned())
                .unwrap_or_default()
        })
        .collect();
    let needs = |crates: &BTreeSet<String>| -> BTreeSet<String> {
        members
            .iter()
            .filter(|m| crates.contains(&m.name))
            .flat_map(|m| m.depends_on.iter().cloned())
            .filter(|d| !crates.contains(d))
            .collect()
    };
    // after[i] holds the packages i must be released after.
    let mut after: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (i, own) in crates.iter().enumerate() {
        let wanted = needs(own);
        let deps = crates
            .iter()
            .enumerate()
            .filter(|&(j, other)| j != i && !other.is_disjoint(&wanted))
            .map(|(j, _)| j)
            .collect();
        after.insert(i, deps);
    }
    let mut order = Vec::with_capacity(packages.len());
    let mut done = BTreeSet::new();
    while order.len() < packages.len() {
        let next = (0..packages.len())
            .find(|i| !done.contains(i) && after.get(i).is_none_or(|deps| deps.is_subset(&done)));
        let Some(i) = next else {
            let stuck: Vec<&str> = (0..packages.len())
                .filter(|i| !done.contains(i))
                .filter_map(|i| packages.get(i).map(|p| p.name.as_str()))
                .collect();
            return Err(format!(
                "{} depend on each other, so no order releases each after its dependencies",
                stuck.join(", ")
            ));
        };
        done.insert(i);
        if let Some(p) = packages.get(i) {
            order.push(*p);
        }
    }
    Ok(order)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn configs(root: &Path) -> Vec<Config> {
        let text = r#"{"corpus": "corpus", "packages": [
            {"name": "cli", "package": "pkg.cli", "tag": "PKG-AAAAAA-10-2026",
             "manifest": {"kind": "cargo-workspace", "path": "Cargo.toml"}, "code": ["crates/"]},
            {"name": "core", "package": "pkg.core", "tag": "PKG-BBBBBB-10-2026",
             "manifest": {"kind": "cargo-package", "path": "crates/core/Cargo.toml"}, "code": ["crates/core/"]},
            {"name": "algo", "package": "pkg.algo", "tag": "PKG-CCCCCC-10-2026",
             "manifest": {"kind": "cargo-package", "path": "crates/algo/Cargo.toml"}, "code": ["crates/algo/"]}
        ]}"#;
        Config::parse_all(root, text, "release.json").unwrap()
    }

    fn member(root: &Path, name: &str, dir: &str, deps: &[&str]) -> Member {
        Member {
            name: name.to_owned(),
            dir: root.join(dir),
            depends_on: deps.iter().map(|d| (*d).to_owned()).collect(),
        }
    }

    fn names(order: &[&Config]) -> Vec<String> {
        order.iter().map(|c| c.name.clone()).collect()
    }

    // covers: order::by_dependency
    #[test]
    fn a_package_is_released_after_what_it_depends_on() {
        let root = Path::new("/repo");
        let all = configs(root);
        // The workspace's binary needs core, and core needs algo.
        let members = vec![
            member(root, "cli-bin", "bin/cli", &["core", "serde"]),
            member(root, "core", "crates/core", &["algo"]),
            member(root, "algo", "crates/algo", &[]),
        ];
        let order = by_dependency(&all.iter().collect::<Vec<_>>(), &all, &members).unwrap();
        assert_eq!(names(&order), ["algo", "core", "cli"]);
    }

    #[test]
    fn packages_with_no_order_between_them_keep_release_json_order() {
        let root = Path::new("/repo");
        let all = configs(root);
        let members = vec![
            member(root, "cli-bin", "bin/cli", &[]),
            member(root, "core", "crates/core", &[]),
            member(root, "algo", "crates/algo", &[]),
        ];
        let order = by_dependency(&all.iter().collect::<Vec<_>>(), &all, &members).unwrap();
        assert_eq!(names(&order), ["cli", "core", "algo"]);
    }

    #[test]
    fn a_cycle_is_refused() {
        let root = Path::new("/repo");
        let all = configs(root);
        let members = vec![
            member(root, "core", "crates/core", &["algo"]),
            member(root, "algo", "crates/algo", &["core"]),
        ];
        let chosen: Vec<&Config> = all.iter().skip(1).collect();
        let err = by_dependency(&chosen, &all, &members)
            .map(|_| ())
            .unwrap_err();
        assert!(err.contains("core, algo depend on each other"), "{err}");
    }

    // covers: order::members
    #[test]
    fn dev_dependencies_do_not_order_releases() {
        let metadata = r#"{"packages": [
            {"name": "core", "manifest_path": "/repo/crates/core/Cargo.toml",
             "dependencies": [{"name": "algo", "kind": null}, {"name": "proptest", "kind": "dev"}]},
            {"name": "algo", "manifest_path": "/repo/crates/algo/Cargo.toml",
             "dependencies": [{"name": "core", "kind": "dev"}, {"name": "cc", "kind": "build"}]}
        ]}"#;
        let found = members(metadata).unwrap();
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .any(|m| m.name == "core" && m.depends_on.contains("algo"))
        );
        assert!(found.iter().any(|m| m.name == "algo"
            && !m.depends_on.contains("core")
            && m.depends_on.contains("cc")));
        assert_eq!(found[0].dir, Path::new("/repo/crates/core"));
    }
}
