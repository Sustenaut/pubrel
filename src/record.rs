//! The release record: a package's `release.json`, its unreleased changes,
//! and the package publet its releases are versions of.
//!
//! Building and publishing the package publet -- which signs it -- go
//! through the installed `pub`, a release from 0.1 or 0.2 (`>=0.1.0, <0.3.0`). Reading it back is
//! done here, with `publet-core` (see `published`).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::manifest::Kind;
use crate::version::{self, Version};

/// One package a repository releases, as `release.json` describes it.
///
/// A repository releases one package -- `release.json` is then that
/// package's description -- or several, listed under `packages`, each with
/// its own manifest, code paths, package publet and tag prefix. The corpus
/// and the signing key are the repository's, shared unless a package names
/// its own.
pub struct Config {
    /// The repository's root: the directory holding `release.json`.
    pub root: PathBuf,
    /// The package's name, as releases are titled: `publet-cli`.
    pub name: String,
    /// The package publet's slug: `pkg.publet-cli`.
    pub package: String,
    /// Its Section 9.1 citation tag.
    pub tag: String,
    /// What a release's git tag starts with: `v`, or `publet-core-v`.
    pub tag_prefix: String,
    /// The corpus directory, relative to the root.
    pub corpus: PathBuf,
    /// The kind of manifest the version is kept in.
    pub kind: Kind,
    /// The manifest, relative to the root: `Cargo.toml`.
    pub manifest: PathBuf,
    /// Path prefixes whose change is a change to what users run.
    pub code: Vec<String>,
    /// The command the package provides, and how to ask its version.
    pub command: Option<(String, String)>,
    /// How to install a release; `{tag}` is substituted.
    pub install: Option<String>,
    /// The key object a release must be signed by: the corpus's author.
    pub key: Option<String>,
    /// The generated changelog, relative to the root.
    pub changelog: PathBuf,
    /// The security checks a release must pass, recorded in it.
    pub security: crate::security::Declared,
    /// Whether the crate's public API is checked against its last release.
    pub semver_checks: bool,
}

impl Config {
    /// Every package `release.json` at `root` describes.
    ///
    /// # Errors
    ///
    /// Returns a message if `release.json` cannot be read or is not JSON;
    /// if `packages` is present but is not a non-empty list; if an entry
    /// lacks `name`, `package`, `tag`, `manifest.kind` or `manifest.path`,
    /// or names an unknown manifest kind; if an entry sets `semver-checks`
    /// without a `cargo-package` manifest; if a `security` section is
    /// malformed (see [`crate::security::Declared::from_json`]); or if two
    /// packages share a name or a tag prefix.
    pub fn read_all(root: &Path) -> Result<Vec<Self>, String> {
        let path = root.join("release.json");
        let text = std::fs::read_to_string(&path).map_err(|_| {
            format!(
                "no release.json in {}; `pubrel init` writes one",
                root.display()
            )
        })?;
        Self::parse_all(root, &text, &path.display().to_string())
    }

    /// Every package a `release.json`'s text describes, for a repository
    /// at `root`; `source` names the text in a syntax error. [`Config::read_all`]
    /// reads the file and calls this, and the fuzz targets call it directly.
    ///
    /// # Errors
    ///
    /// Returns a message in the cases [`Config::read_all`] lists, other than
    /// a file that cannot be read.
    pub fn parse_all(root: &Path, text: &str, source: &str) -> Result<Vec<Self>, String> {
        let json: Value = serde_json::from_str(text).map_err(|e| format!("{source}: {e}"))?;
        let Some(packages) = json.get("packages") else {
            return Ok(vec![Self::from_json(root, &json, &json, false)?]);
        };
        let packages = packages
            .as_array()
            .filter(|p| !p.is_empty())
            .ok_or("release.json's `packages` must list at least one package")?;
        let all = packages
            .iter()
            .map(|p| Self::from_json(root, p, &json, true))
            .collect::<Result<Vec<_>, _>>()?;
        for (i, a) in all.iter().enumerate() {
            if all
                .iter()
                .skip(i + 1)
                .any(|b| b.name == a.name || b.tag_prefix == a.tag_prefix)
            {
                return Err(format!(
                    "release.json lists two packages with the name or tag prefix of {}",
                    a.name
                ));
            }
        }
        Ok(all)
    }

    /// One package, from its entry; `top` supplies what packages share.
    fn from_json(root: &Path, json: &Value, top: &Value, listed: bool) -> Result<Self, String> {
        let text = |k: &str| json.get(k).and_then(Value::as_str).map(str::to_owned);
        let shared =
            |k: &str| text(k).or_else(|| top.get(k).and_then(Value::as_str).map(str::to_owned));
        let need = |k: &str| text(k).ok_or_else(|| format!("release.json needs `{k}`"));
        let manifest = json
            .get("manifest")
            .ok_or("release.json needs `manifest`: {kind, path}")?;
        let kind = Kind::parse(
            manifest
                .get("kind")
                .and_then(Value::as_str)
                .ok_or("manifest.kind is required")?,
        )?;
        let command = json.get("command").and_then(|c| {
            Some((
                c.get("name")?.as_str()?.to_owned(),
                c.get("check")?.as_str()?.to_owned(),
            ))
        });
        let manifest = PathBuf::from(
            manifest
                .get("path")
                .and_then(Value::as_str)
                .ok_or("manifest.path is required")?,
        );
        let name = need("name")?;
        // A listed package's tags and changelog sit beside its own manifest
        // unless it says otherwise; a repository's only package keeps `v`
        // and the root CHANGELOG.md, as it always has.
        let tag_prefix = text("tag-prefix").unwrap_or_else(|| {
            if listed && manifest.parent().is_some_and(|d| !d.as_os_str().is_empty()) {
                format!("{name}-v")
            } else {
                "v".to_owned()
            }
        });
        let changelog = text("changelog").map_or_else(
            || {
                manifest
                    .parent()
                    .map_or_else(PathBuf::new, Path::to_path_buf)
                    .join("CHANGELOG.md")
            },
            PathBuf::from,
        );
        let semver_checks = json
            .get("semver-checks")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if semver_checks && kind != Kind::CargoPackage {
            return Err(format!(
                "{name}: `semver-checks` checks a crate's public API, so it needs a \
                 cargo-package manifest"
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            name,
            package: need("package")?,
            tag: need("tag")?,
            tag_prefix,
            corpus: PathBuf::from(shared("corpus").unwrap_or_else(|| "corpus".to_owned())),
            kind,
            manifest,
            code: json
                .get("code")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            command,
            install: text("install"),
            key: shared("key"),
            changelog,
            security: crate::security::Declared::from_json(
                json.get("security").or_else(|| top.get("security")),
            )?,
            semver_checks,
        })
    }

    /// The git tag a release of this package is: `v0.2.0`, `publet-core-v0.2.0`.
    #[must_use]
    pub fn git_tag(&self, version: &Version) -> String {
        format!("{}{version}", self.tag_prefix)
    }

    /// The directory holding the manifest, relative to the root: `crates/core`.
    #[must_use]
    pub fn manifest_dir(&self) -> String {
        self.manifest
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Whether a changed path is part of what this package ships.
    #[must_use]
    pub fn owns(&self, path: &str) -> bool {
        self.code.iter().any(|c| path.starts_with(c.as_str()))
    }

    /// The corpus directory, as an absolute path under the root.
    pub fn corpus_dir(&self) -> PathBuf {
        self.root.join(&self.corpus)
    }

    /// The package publet's source directory: `<corpus>/publets/<package>`.
    pub fn package_dir(&self) -> PathBuf {
        self.corpus_dir().join("publets").join(&self.package)
    }

    /// The unreleased list, relative to the root, as git names it.
    pub fn unreleased_rel(&self) -> String {
        self.corpus
            .join("publets")
            .join(&self.package)
            .join("unreleased.json")
            .to_string_lossy()
            .into_owned()
    }

    /// The corpus's lock, relative to the root, as git names it.
    pub fn lock_rel(&self) -> String {
        self.corpus
            .join("corpus.lock")
            .to_string_lossy()
            .into_owned()
    }

    /// The manifest, relative to the root, as git names it.
    pub fn manifest_rel(&self) -> String {
        self.manifest.to_string_lossy().into_owned()
    }

    /// The manifest's current version.
    ///
    /// # Errors
    ///
    /// Returns a message if the manifest cannot be read, or if
    /// [`crate::manifest::read`] fails on its text.
    pub fn manifest_version(&self) -> Result<Version, String> {
        let text = std::fs::read_to_string(self.root.join(&self.manifest))
            .map_err(|e| format!("{}: {e}", self.manifest.display()))?;
        crate::manifest::read(&text, self.kind)
    }

    /// The crate's name, as its manifest states it: `[package].name`.
    ///
    /// # Errors
    ///
    /// Returns a message if the manifest cannot be read or names no package.
    pub fn crate_name(&self) -> Result<String, String> {
        let text = std::fs::read_to_string(self.root.join(&self.manifest))
            .map_err(|e| format!("{}: {e}", self.manifest.display()))?;
        let doc: toml_edit::DocumentMut = text
            .parse()
            .map_err(|e| format!("{}: not TOML: {e}", self.manifest.display()))?;
        doc.get("package")
            .and_then(|p| p.get("name"))
            .and_then(toml_edit::Item::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{} names no [package]", self.manifest.display()))
    }
}

/// One recorded change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// One of [`version::CATEGORIES`].
    pub category: String,
    /// What changed, in a sentence.
    pub change: String,
}

/// The changes an `unreleased.json` lists. `None`, for a file that does
/// not exist, lists none.
///
/// # Errors
///
/// Returns a message if the text is not JSON, or if a row of `changes` has
/// a category not in [`version::CATEGORIES`] or a blank `change`.
pub fn changes(text: Option<&str>) -> Result<Vec<Change>, String> {
    let Some(text) = text else {
        return Ok(Vec::new());
    };
    let json: Value = serde_json::from_str(text).map_err(|e| format!("unreleased.json: {e}"))?;
    let mut out = Vec::new();
    for row in json
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = |k: &str| {
            row.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let (category, change) = (field("category"), field("change"));
        if !version::known(&category) || change.trim().is_empty() {
            return Err(format!(
                "each change needs a known category ({}) and a description: {row}",
                version::CATEGORIES.join(", ")
            ));
        }
        out.push(Change { category, change });
    }
    Ok(out)
}

/// The summary an `unreleased.json` gives the next release, if any.
#[must_use]
pub fn summary(text: Option<&str>) -> Option<String> {
    let json: Value = serde_json::from_str(text?).ok()?;
    json.get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Write an `unreleased.json`, creating its directory if need be.
///
/// # Errors
///
/// Returns a message if the directory cannot be created or the file cannot
/// be written.
pub fn write_changes(path: &Path, rows: &[Change], summary: Option<&str>) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut json = json!({
        "comment": "Changes not yet released. One row per change: category changed or \
                    removed (a published interface changed: major), added (a feature: \
                    minor), or fixed or security (patch). `pubrel add` appends one; \
                    `pubrel summary` sets the release's summary; `pubrel prepare` moves \
                    them into the package publet.",
        "changes": rows.iter().map(|r| json!({"category": r.category, "change": r.change}))
            .collect::<Vec<_>>(),
    });
    if let Some(summary) = summary {
        json["summary"] = Value::String(summary.to_owned());
    }
    let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// What a summary claim's content puts between the version and the summary:
/// "pubrel 0.5.0 in brief: ...".
const BRIEF: &str = " in brief: ";

/// Whether the summary claim `content` is the summary of `version` of
/// `cfg`: its content reads "`<name> <version> in brief: ...`".
#[must_use]
pub fn summarizes(cfg: &Config, content: &str, version: &Version) -> bool {
    content.starts_with(&format!("{}{version}{BRIEF}", prefix(cfg)))
}

/// What every claim's content about a release of `cfg` starts with.
fn prefix(cfg: &Config) -> String {
    format!("{} ", cfg.name)
}

/// The package publet's source for a release.
#[must_use]
pub fn package_source(
    cfg: &Config,
    version: &Version,
    date: &str,
    commit: &str,
    rows: &[Change],
    summary: Option<&str>,
    security: &[crate::security::Row],
) -> Value {
    let tag = cfg.git_tag(version);
    let (name, check, expect) = match &cfg.command {
        Some((name, check)) => (name.clone(), check.clone(), format!("{name} {version}")),
        None => (cfg.name.clone(), String::new(), String::new()),
    };
    let scope = format!("the published release {tag} of {}", cfg.name);
    let columns =
        |names: &[&str]| -> Vec<Value> { names.iter().map(|n| json!({"name": n})).collect() };
    let mut source = json!({
        "slug": cfg.package, "tag": cfg.tag, "created": format!("{date}T00:00:00Z"),
        "title": cfg.name,
        "claims": {
            "identity": {
                "class": "archival", "scope": scope,
                "content": format!("{} {version} is released as {tag} from commit {commit}.", cfg.name),
                "data": {"columns": columns(&["name", "version", "check", "expect", "tag", "commit"]),
                         "rows": [[name, version.to_string(), check, expect, tag, commit]]},
                "sources": [{"ref": cfg.name, "revision": commit}],
            },
            "release": {
                "class": "archival", "scope": scope,
                "content": format!("{} {version} was released on {date} from commit {commit} with these changes.", cfg.name),
                "depends": ["#identity"],
                "data": {"columns": columns(&["category", "change"]),
                         "rows": rows.iter().map(|r| json!([r.category, r.change])).collect::<Vec<_>>()},
                "sources": [{"ref": cfg.name, "revision": commit}],
            },
        },
        "sections": [
            {"heading": "Identity", "items": [{"ref": "#identity"}]},
            {"heading": "Release", "items": [{"ref": "#release"}]},
        ],
    });
    if let Some(summary) = summary {
        source["claims"]["summary"] = json!({
            "class": "archival", "scope": scope,
            "content": format!("{}{version}{BRIEF}{summary}", prefix(cfg)),
            "depends": ["#release"],
            "sources": [{"ref": cfg.name, "revision": commit}],
        });
        if let Some(sections) = source["sections"].as_array_mut() {
            sections.insert(
                0,
                json!({"heading": "Summary", "items": [{"ref": "#summary"}]}),
            );
        }
    }
    if !security.is_empty() {
        source["claims"]["security"] = json!({
            "class": "archival", "scope": scope,
            "content": format!(
                "{} {version} passed these security checks before release on {date}.",
                cfg.name
            ),
            "depends": ["#identity"],
            "data": {"columns": columns(&["kind", "name", "command", "tool", "result"]),
                     "rows": security.iter()
                        .map(|r| json!([r.kind, r.name, r.command, r.tool, r.result]))
                        .collect::<Vec<_>>()},
            "sources": [{"ref": cfg.name, "revision": commit}],
        });
        if let Some(sections) = source["sections"].as_array_mut() {
            sections.push(json!({"heading": "Security", "items": [{"ref": "#security"}]}));
        }
    }
    source
}

// --- pub --------------------------------------------------------------------

/// The `pub` releases this pubrel works with.
static PUB_REQUIRED: std::sync::LazyLock<semver::VersionReq> = std::sync::LazyLock::new(|| {
    semver::VersionReq::parse(">=0.1.0, <0.3.0").unwrap_or(semver::VersionReq::STAR)
});

/// The `pub` to drive: `$PUB`, or `pub` on the PATH.
pub fn pub_program() -> String {
    std::env::var("PUB").unwrap_or_else(|_| "pub".to_owned())
}

/// Confirm the `pub` this will drive is one pubrel is tested with: at
/// least 0.1.0 and below 0.3.0. pub 0.2.0 changed only render pipelines'
/// tool requirements, which pubrel does not use; a later minor version is
/// admitted once the suite passes against it.
///
/// # Errors
///
/// Returns a message if it cannot run or is not such a release.
pub fn require_pub() -> Result<(), String> {
    let program = pub_program();
    let out = Command::new(&program)
        .arg("--version")
        .output()
        .map_err(|e| format!("cannot run `{program} --version` ({e}); install a released pub"))?;
    let first = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let version = first
        .strip_prefix("pub ")
        .and_then(|v| version::parse(v).ok())
        .filter(|v| PUB_REQUIRED.matches(v));
    if version.is_none() {
        return Err(format!(
            "pubrel drives a released pub from 0.1 or 0.2, and `{program} --version` says \
             {first:?}; install one: cargo install --locked --git \
             https://github.com/schryer/publet --tag v0.2.0 publet-cli"
        ));
    }
    Ok(())
}

/// Run `pub` in `dir`, and return what it printed to stdout.
///
/// # Errors
///
/// Returns a message if `pub` cannot be started, or, with its stdout and
/// stderr, if it exits non-zero.
pub fn run_pub(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(pub_program())
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run pub: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        Ok(text)
    } else {
        Err(format!(
            "pub {} failed:\n{text}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

pub use crate::published::read_object;

/// The identifier a lock records for `slug`.
#[must_use]
pub fn locked(lock_text: Option<&str>, slug: &str) -> Option<String> {
    let json: Value = serde_json::from_str(lock_text?).ok()?;
    json.get("publets")?
        .as_array()?
        .iter()
        .find(|r| r.get("slug").and_then(Value::as_str) == Some(slug))?
        .get("cid")?
        .as_str()
        .map(str::to_owned)
}

/// The version a lock's published package identity states. `None` if the
/// lock records no identity for the package.
///
/// # Errors
///
/// Returns a message if the identity is recorded but
/// [`read_object`] fails on it, or it states no version, or the version is
/// not plain `MAJOR.MINOR.PATCH`.
pub fn published_version(cfg: &Config, lock_text: Option<&str>) -> Result<Option<Version>, String> {
    let Some(cid) = locked(lock_text, &format!("{}#identity", cfg.package)) else {
        return Ok(None);
    };
    let object = read_object(cfg, &cid)?;
    let stated = object
        .pointer("/body/data/rows/0/1")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{cid} states no version"))?;
    version::parse(stated).map(Some)
}

/// A published release, read back from its `release` claim.
pub struct Release {
    /// The version released.
    pub version: Version,
    /// The release date, `YYYY-MM-DD`, as the claim states it.
    pub date: String,
    /// The commit released from, as the claim states it.
    pub commit: String,
    /// The `release` claim's identifier.
    pub cid: String,
    /// The changes released, as (category, change) pairs.
    pub rows: Vec<(String, String)>,
    /// The release's summary, if it has one.
    pub summary: Option<String>,
    /// The security record, if the release has one.
    pub security: Vec<crate::security::Row>,
}

/// Every published release of the package, newest first: the `release`
/// claim's lineage, read from the corpus's objects.
#[must_use]
pub fn releases(cfg: &Config) -> Vec<Release> {
    let prefix = prefix(cfg);
    let mut out = Vec::new();
    let mut security: Vec<(Version, Vec<crate::security::Row>)> = Vec::new();
    let mut summaries: Vec<(Version, String)> = Vec::new();
    let dir = cfg.corpus_dir().join("objects");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".cbor") else {
            continue;
        };
        let cid = stem.replace('_', ":");
        let Ok(object) = read_object(cfg, &cid) else {
            continue;
        };
        if object.get("type").and_then(Value::as_str) != Some("claim.prose") {
            continue;
        }
        let content = object
            .pointer("/body/content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(rest) = content.strip_prefix(&prefix) else {
            continue;
        };
        // "<version> in brief: <summary>"
        if let Some((v, summary)) = rest.split_once(BRIEF) {
            if let Ok(version) = version::parse(v) {
                summaries.push((version, summary.to_owned()));
            }
            continue;
        }
        let words: Vec<&str> = rest.split_whitespace().collect();
        // "<version> passed these security checks before release on <date>."
        if let (Some(v), Some(&"passed")) = (words.first(), words.get(1)) {
            if let Ok(version) = version::parse(v) {
                security.push((version, security_rows(&object)));
            }
            continue;
        }
        // "<version> was released on <date> from commit <sha> with these changes."
        let (Some(v), Some(&"was"), Some(date), Some(commit)) =
            (words.first(), words.get(1), words.get(4), words.get(7))
        else {
            continue;
        };
        let Ok(version) = version::parse(v) else {
            continue;
        };
        let rows = object
            .pointer("/body/data/rows")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some((
                    r.get(0)?.as_str()?.to_owned(),
                    r.get(1)?.as_str()?.to_owned(),
                ))
            })
            .collect();
        out.push(Release {
            version,
            date: (*date).to_owned(),
            commit: (*commit).to_owned(),
            cid,
            rows,
            summary: None,
            security: Vec::new(),
        });
    }
    for release in &mut out {
        if let Some((_, rows)) = security.iter().find(|(v, _)| *v == release.version) {
            release.security.clone_from(rows);
        }
        if let Some((_, text)) = summaries.iter().find(|(v, _)| *v == release.version) {
            release.summary = Some(text.clone());
        }
    }
    out.sort_by(|a, b| b.version.cmp(&a.version));
    out
}

/// A security claim's rows: kind, name, command, tool, result.
fn security_rows(object: &Value) -> Vec<crate::security::Row> {
    object
        .pointer("/body/data/rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let cell = |i: usize| r.get(i).and_then(Value::as_str).map(str::to_owned);
            Some(crate::security::Row {
                kind: cell(0)?,
                name: cell(1)?,
                command: cell(2)?,
                tool: cell(3)?,
                result: cell(4)?,
            })
        })
        .collect()
}

/// One release as a changelog section.
#[must_use]
pub fn section(r: &Release) -> String {
    use std::fmt::Write as _;
    let mut out = format!(
        "## {} - {}\n\nCommit `{}`; release record `{}`.\n\n",
        r.version,
        r.date,
        r.commit.chars().take(12).collect::<String>(),
        r.cid
    );
    if let Some(summary) = &r.summary {
        let _ = write!(out, "{summary}\n\n");
    }
    for category in version::CATEGORIES {
        let entries: Vec<&String> = r
            .rows
            .iter()
            .filter(|(c, _)| c == category)
            .map(|(_, e)| e)
            .collect();
        if entries.is_empty() {
            continue;
        }
        let mut title = category.to_owned();
        if let Some(first) = title.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        let _ = writeln!(out, "### {title}");
        for e in entries {
            let _ = writeln!(out, "- {e}");
        }
        out.push('\n');
    }
    if !r.security.is_empty() {
        let _ = writeln!(out, "### Security checks");
        for row in &r.security {
            if row.kind == "fact" {
                let _ = writeln!(out, "- {}: `{}`", row.name, row.result);
            } else if row.tool.is_empty() {
                let _ = writeln!(out, "- {}: `{}` -- {}", row.name, row.command, row.result);
            } else {
                let _ = writeln!(
                    out,
                    "- {}: `{}` ({}) -- {}",
                    row.name, row.command, row.tool, row.result
                );
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // covers: record::changes
    #[test]
    fn unreleased_rows_are_validated() {
        let ok = changes(Some(
            r#"{"changes": [{"category": "added", "change": "x"}]}"#,
        ))
        .unwrap();
        assert_eq!(ok.len(), 1);
        assert!(
            changes(Some(
                r#"{"changes": [{"category": "nope", "change": "x"}]}"#
            ))
            .is_err()
        );
        assert!(
            changes(Some(
                r#"{"changes": [{"category": "fixed", "change": " "}]}"#
            ))
            .is_err()
        );
        assert!(changes(None).unwrap().is_empty());
    }

    // covers: record::summarizes
    // The lock keeps the last summary after a release without one; it is
    // that earlier release's, and the new release is not held to it.
    #[test]
    fn a_summary_belongs_to_the_version_it_names() {
        let cfg = Config::parse_all(
            std::path::Path::new("/repo"),
            r#"{"name": "demo", "package": "pkg.demo", "tag": "PKG-DEMODE-10-2026",
                "corpus": "corpus", "manifest": {"kind": "cargo-package", "path": "Cargo.toml"},
                "code": ["src/"]}"#,
            "release.json",
        )
        .unwrap()
        .remove(0);
        let v010 = Version::new(0, 1, 0);
        let v011 = Version::new(0, 1, 1);
        let content = "demo 0.1.0 in brief: the first release.";
        assert!(summarizes(&cfg, content, &v010));
        assert!(!summarizes(&cfg, content, &v011));
        assert!(!summarizes(&cfg, "other 0.1.0 in brief: x", &v010));
    }

    // covers: record::summary
    #[test]
    fn a_summary_is_optional_and_never_blank() {
        assert_eq!(
            summary(Some(r#"{"changes": [], "summary": " Faster. "}"#)).as_deref(),
            Some("Faster.")
        );
        assert_eq!(summary(Some(r#"{"changes": [], "summary": "  "}"#)), None);
        assert_eq!(summary(Some(r#"{"changes": []}"#)), None);
        assert_eq!(summary(None), None);
    }

    // covers: record::locked
    #[test]
    fn a_lock_row_is_found_by_slug() {
        let lock = r#"{"publets": [{"slug": "pkg.x#identity", "cid": "pub:sha2-256:abc"}]}"#;
        assert_eq!(
            locked(Some(lock), "pkg.x#identity").as_deref(),
            Some("pub:sha2-256:abc")
        );
        assert_eq!(locked(Some(lock), "pkg.y#identity"), None);
        assert_eq!(locked(None, "pkg.x#identity"), None);
    }
}
