//! Where a package states its version, and how to change it.
//!
//! Edits go through `toml_edit`, so a manifest keeps its comments and
//! layout: a release changes the version and nothing else about the file.
//! A `package.json` is edited in place the same way: only the bytes of its
//! version string change, so its indentation and key order stay npm's.

use std::path::Path;
use std::process::Command;

use toml_edit::{DocumentMut, Item, value};

use crate::version::{self, Version};

/// The kinds of manifest a package may keep its version in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `[workspace.package].version`, which every internal path dependency
    /// in `[workspace.dependencies]` also pins.
    CargoWorkspace,
    /// `[package].version`.
    CargoPackage,
    /// `[project].version` in a `pyproject.toml`.
    Pyproject,
    /// The top-level `version` in a `package.json`, and the same version
    /// in a `package-lock.json` beside it.
    Npm,
}

impl Kind {
    /// Read a kind from its name in `release.json`.
    ///
    /// # Errors
    ///
    /// Returns a message naming the known kinds if `name` is not
    /// `cargo-workspace`, `cargo-package`, `pyproject` or `npm`.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "cargo-workspace" => Ok(Self::CargoWorkspace),
            "cargo-package" => Ok(Self::CargoPackage),
            "pyproject" => Ok(Self::Pyproject),
            "npm" => Ok(Self::Npm),
            other => Err(format!(
                "unknown manifest kind `{other}` (cargo-workspace, cargo-package, pyproject, npm)"
            )),
        }
    }

    fn table(self) -> [&'static str; 2] {
        match self {
            Self::CargoWorkspace => ["workspace", "package"],
            Self::CargoPackage => ["package", ""],
            Self::Pyproject => ["project", ""],
            Self::Npm => ["", ""],
        }
    }
}

fn version_item(doc: &DocumentMut, kind: Kind) -> Option<&Item> {
    let [outer, inner] = kind.table();
    let table = doc.get(outer)?;
    let table = if inner.is_empty() {
        table
    } else {
        table.get(inner)?
    };
    table.get("version")
}

/// The version a manifest states.
///
/// # Errors
///
/// Returns a message if the text is not TOML, states no version where
/// `kind` keeps it, or states one that is not plain `MAJOR.MINOR.PATCH`
/// (see [`version::parse`]).
pub fn read(text: &str, kind: Kind) -> Result<Version, String> {
    if kind == Kind::Npm {
        let doc: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        let stated = doc
            .get("version")
            .and_then(serde_json::Value::as_str)
            .ok_or("the manifest states no top-level `version`, where a package.json does")?;
        return version::parse(stated);
    }
    let doc: DocumentMut = text.parse().map_err(|e| format!("not TOML: {e}"))?;
    let stated = version_item(&doc, kind)
        .and_then(Item::as_str)
        .ok_or_else(|| format!("the manifest states no version where a {kind:?} does"))?;
    version::parse(stated)
}

/// The manifest with its version set to `version`. For a workspace, every
/// internal path dependency pinned at the old version moves with it, or
/// the workspace no longer resolves -- except those under `own`: crates
/// released with versions of their own, whose pins only their own
/// releases change (see [`pin`]), even when the two versions coincide.
///
/// # Errors
///
/// Returns a message if [`read`] fails on the text.
pub fn write(text: &str, kind: Kind, version: &Version, own: &[String]) -> Result<String, String> {
    let old = read(text, kind)?.to_string();
    if kind == Kind::Npm {
        return set_json_string(text, &["version"], &version.to_string())?
            .ok_or_else(|| "the manifest states no top-level `version`".to_owned());
    }
    let mut doc: DocumentMut = text.parse().map_err(|e| format!("not TOML: {e}"))?;
    let [outer, inner] = kind.table();
    let table = if inner.is_empty() {
        &mut doc[outer]
    } else {
        &mut doc[outer][inner]
    };
    table["version"] = value(version.to_string());

    if kind == Kind::CargoWorkspace
        && let Some(deps) = doc
            .get_mut("workspace")
            .and_then(|w| w.get_mut("dependencies"))
            .and_then(Item::as_table_like_mut)
    {
        for (_, dep) in deps.iter_mut() {
            let Some(dep) = dep.as_table_like_mut() else {
                continue;
            };
            let Some(path) = dep.get("path").and_then(Item::as_str) else {
                continue;
            };
            if own.iter().any(|o| same_dir(o, path)) {
                continue;
            }
            if dep.get("version").and_then(Item::as_str) == Some(old.as_str()) {
                dep.insert("version", value(version.to_string()));
            }
        }
    }
    Ok(doc.to_string())
}

fn same_dir(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// A workspace manifest with the dependency at `dir` pinned at `version`,
/// if `[workspace.dependencies]` names one there; `None` if it does not.
///
/// # Errors
///
/// Returns a message if the text is not TOML.
pub fn pin(text: &str, dir: &str, version: &Version) -> Result<Option<String>, String> {
    let mut doc: DocumentMut = text.parse().map_err(|e| format!("not TOML: {e}"))?;
    let Some(deps) = doc
        .get_mut("workspace")
        .and_then(|w| w.get_mut("dependencies"))
        .and_then(Item::as_table_like_mut)
    else {
        return Ok(None);
    };
    let mut found = false;
    for (_, dep) in deps.iter_mut() {
        let Some(dep) = dep.as_table_like_mut() else {
            continue;
        };
        if dep
            .get("path")
            .and_then(Item::as_str)
            .is_some_and(|p| same_dir(p, dir))
        {
            dep.insert("version", value(version.to_string()));
            found = true;
        }
    }
    Ok(found.then(|| doc.to_string()))
}

/// Bring the lockfile in line with the new version, where there is one.
/// For a Cargo manifest with a `Cargo.lock` at `root`, this runs
/// `cargo update --offline`, for the workspace or for the package `name`.
/// For an npm manifest with a `package-lock.json` beside it, this sets the
/// lockfile's own two copies of the version, as `npm version` would,
/// without running npm. For anything else it does nothing.
///
/// # Errors
///
/// Returns a message if `cargo` cannot be started or exits non-zero, or
/// if a `package-lock.json` is not JSON or states no version.
pub fn refresh_lock(
    root: &Path,
    kind: Kind,
    name: &str,
    manifest: &Path,
    version: &Version,
) -> Result<(), String> {
    let args: Vec<&str> = match kind {
        Kind::CargoWorkspace => vec!["update", "--workspace", "--offline"],
        Kind::CargoPackage => vec!["update", "-p", name, "--offline"],
        Kind::Pyproject => return Ok(()),
        Kind::Npm => return refresh_npm_lock(&root.join(manifest), version),
    };
    if !root.join("Cargo.lock").exists() {
        return Ok(());
    }
    let status = Command::new("cargo")
        .args(&args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

fn refresh_npm_lock(manifest: &Path, version: &Version) -> Result<(), String> {
    let path = manifest.with_file_name("package-lock.json");
    let Ok(mut text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let version = version.to_string();
    // The lockfile's top-level version, and (lockfileVersion 2 and 3) the
    // root package's entry under `packages`.
    text = set_json_string(&text, &["version"], &version)?
        .ok_or("package-lock.json states no top-level `version`")?;
    if let Some(edited) = set_json_string(&text, &["packages", "", "version"], &version)? {
        text = edited;
    }
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// `text`, a JSON document, with the string at `path` (object keys from
/// the top) replaced by `value`, written as a JSON string; `None` if
/// nothing is there or it is not a string. Only the bytes of that string
/// change. Where a key is repeated, the last one counts, as for
/// `serde_json`.
///
/// # Errors
///
/// Returns a message if `text` is not JSON.
fn set_json_string(text: &str, path: &[&str], value: &str) -> Result<Option<String>, String> {
    serde_json::from_str::<serde_json::Value>(text).map_err(|e| format!("not JSON: {e}"))?;
    let mut walk = Scan {
        text: text.as_bytes(),
        at: 0,
    };
    let Some(span) = walk.find(path) else {
        return Ok(None);
    };
    let quoted = serde_json::to_string(value).map_err(|e| e.to_string())?;
    Ok(Some(format!(
        "{}{quoted}{}",
        &text[..span.start],
        &text[span.end..]
    )))
}

/// A walk over a JSON document already known to be valid.
struct Scan<'a> {
    text: &'a [u8],
    at: usize,
}

impl Scan<'_> {
    fn space(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    /// The span of the string starting here, quotes included.
    fn string(&mut self) -> std::ops::Range<usize> {
        let start = self.at;
        self.at += 1;
        while let Some(&b) = self.text.get(self.at) {
            self.at += 1;
            match b {
                b'\\' => self.at += 1,
                b'"' => break,
                _ => {}
            }
        }
        start..self.at
    }

    /// Skip the value starting here.
    fn skip(&mut self) {
        self.space();
        match self.text.get(self.at) {
            Some(b'"') => {
                self.string();
            }
            Some(b'{' | b'[') => {
                let mut depth = 0usize;
                while let Some(&b) = self.text.get(self.at) {
                    match b {
                        b'"' => {
                            self.string();
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                self.at += 1;
                                return;
                            }
                        }
                        _ => {}
                    }
                    self.at += 1;
                }
            }
            _ => {
                while self
                    .text
                    .get(self.at)
                    .is_some_and(|b| !matches!(b, b',' | b'}' | b']') && !b.is_ascii_whitespace())
                {
                    self.at += 1;
                }
            }
        }
    }

    /// The span of the string at `path` within the value starting here.
    fn find(&mut self, path: &[&str]) -> Option<std::ops::Range<usize>> {
        self.space();
        let Some((key, rest)) = path.split_first() else {
            return (self.text.get(self.at) == Some(&b'"')).then(|| self.string());
        };
        if self.text.get(self.at) != Some(&b'{') {
            return None;
        }
        self.at += 1;
        let mut found = None;
        loop {
            self.space();
            match self.text.get(self.at) {
                Some(b'"') => {}
                _ => return found,
            }
            let name = self.string();
            let name: String = serde_json::from_slice(&self.text[name]).ok()?;
            self.space();
            self.at += 1; // the colon
            if name == *key {
                let start = self.at;
                found = self.find(rest).or(found);
                self.at = start;
            }
            self.skip();
            self.space();
            if self.text.get(self.at) == Some(&b',') {
                self.at += 1;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const WORKSPACE: &str = r#"[workspace]
members = ["crates/*"]

# The version every crate shares.
[workspace.package]
version = "0.0.1"
edition = "2024"

[workspace.dependencies]
graphset = { git = "https://example.org/g", rev = "abc", version = "0.1.0" }
core = { path = "crates/core", version = "0.0.1" }
"#;

    // covers: manifest::write, manifest::read
    #[test]
    fn a_workspace_moves_with_its_internal_dependencies() {
        let v = &version::parse("0.1.0").unwrap();
        let out = write(WORKSPACE, Kind::CargoWorkspace, v, &[]).unwrap();
        assert_eq!(read(&out, Kind::CargoWorkspace).unwrap(), *v);
        assert!(
            out.contains(r#"core = { path = "crates/core", version = "0.1.0" }"#),
            "{out}"
        );
        assert!(out.contains(r#"rev = "abc", version = "0.1.0""#), "{out}");
        assert!(
            out.contains("# The version every crate shares."),
            "comments kept"
        );
    }

    // covers: manifest::write, manifest::read
    #[test]
    fn a_package_and_a_pyproject_change_only_their_version() {
        let v = &version::parse("0.2.0").unwrap();
        let cargo = "[package]\nname = \"x\"\nversion = \"0.1.0\" # keep\n";
        let out = write(cargo, Kind::CargoPackage, v, &[]).unwrap();
        assert_eq!(read(&out, Kind::CargoPackage).unwrap(), *v);
        assert!(out.contains("name = \"x\""));
        let py = "[project]\nname = \"y\"\nversion = \"0.1.0\"\n";
        let out = write(py, Kind::Pyproject, v, &[]).unwrap();
        assert_eq!(read(&out, Kind::Pyproject).unwrap(), *v);
    }

    // covers: manifest::write
    #[test]
    fn a_crate_with_its_own_version_keeps_its_pin_through_a_workspace_bump() {
        // core shares the workspace's old version by coincidence; it is
        // released on its own, so the workspace bump leaves it alone.
        let v = &version::parse("0.1.0").unwrap();
        let out = write(
            WORKSPACE,
            Kind::CargoWorkspace,
            v,
            &["crates/core".to_owned()],
        )
        .unwrap();
        assert_eq!(read(&out, Kind::CargoWorkspace).unwrap(), *v);
        assert!(
            out.contains(r#"core = { path = "crates/core", version = "0.0.1" }"#),
            "{out}"
        );
    }

    // covers: manifest::pin
    #[test]
    fn its_own_release_pins_it() {
        let v = &version::parse("0.3.0").unwrap();
        let out = pin(WORKSPACE, "crates/core/", v).unwrap().unwrap();
        assert!(
            out.contains(r#"core = { path = "crates/core", version = "0.3.0" }"#),
            "{out}"
        );
        assert_eq!(
            read(&out, Kind::CargoWorkspace).unwrap().to_string(),
            "0.0.1"
        );
        assert!(pin(WORKSPACE, "crates/other", v).unwrap().is_none());
    }

    // covers: manifest::read, manifest::Kind::parse
    #[test]
    fn a_manifest_without_a_version_there_is_refused() {
        assert!(read("[package]\nname = \"x\"\n", Kind::CargoPackage).is_err());
        assert!(Kind::parse("maven").is_err());
        assert!(read("{\"name\": \"x\"}", Kind::Npm).is_err());
        assert!(read("{\"version\": 1}", Kind::Npm).is_err());
        assert!(read("not json", Kind::Npm).is_err());
    }

    const PACKAGE: &str = r#"{
  "name": "demo",
  "version": "0.1.0",
  "description": "keeps \"version\": \"0.1.0\" here",
  "dependencies": {
    "version": "^1.0.0"
  },
  "scripts": { "test": "node --test" }
}
"#;

    // covers: manifest::write, manifest::read, manifest::Kind::parse
    #[test]
    fn a_package_json_changes_only_its_version() {
        let v = &version::parse("0.2.0").unwrap();
        assert_eq!(Kind::parse("npm").unwrap(), Kind::Npm);
        assert_eq!(read(PACKAGE, Kind::Npm).unwrap().to_string(), "0.1.0");
        let out = write(PACKAGE, Kind::Npm, v, &[]).unwrap();
        assert_eq!(out, PACKAGE.replacen("\"0.1.0\",\n", "\"0.2.0\",\n", 1));
        assert_eq!(read(&out, Kind::Npm).unwrap(), *v);
    }

    // covers: manifest::set_json_string
    #[test]
    fn the_string_at_a_path_is_replaced_and_nothing_else() {
        let lock = r#"{"name":"demo","version":"0.1.0","lockfileVersion":3,
"packages":{"":{"name":"demo","version":"0.1.0"},"node_modules/a":{"version":"0.1.0"}}}"#;
        let out = set_json_string(lock, &["packages", "", "version"], "0.2.0")
            .unwrap()
            .unwrap();
        assert_eq!(
            out,
            lock.replace(
                r#""":{"name":"demo","version":"0.1.0"}"#,
                r#""":{"name":"demo","version":"0.2.0"}"#
            )
        );
        // The last of a repeated key counts, as serde_json reads it.
        let twice = r#"{"version": "1.0.0", "a": [1, {"version": "x"}], "version": "2.0.0"}"#;
        let out = set_json_string(twice, &["version"], "3.0.0")
            .unwrap()
            .unwrap();
        assert_eq!(
            out,
            r#"{"version": "1.0.0", "a": [1, {"version": "x"}], "version": "3.0.0"}"#
        );
        assert!(set_json_string(twice, &["missing"], "1").unwrap().is_none());
        assert!(
            set_json_string(r#"{"version": 1}"#, &["version"], "1")
                .unwrap()
                .is_none()
        );
        assert!(set_json_string("{", &["version"], "1").is_err());
    }
}
