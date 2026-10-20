//! `pubrel`: version a package by what changed, and record each release as
//! a version of its package publet.
//!
//! A release of a package is a version of a publet -- `pkg.<name>` in the
//! package's own corpus -- whose `identity` claim states the version, tag,
//! and commit, and whose `release` claim lists what changed. The publet's
//! lineage is the release history; `CHANGELOG.md` is generated from it.
//! Changes not yet released accumulate in an `unreleased.json` beside the
//! package publet's source: workshop data, never published.
//!
//! The version follows from the categories of what changed, never from
//! judgement at release time (see [`version`]).

mod api;
mod jsdelivr;
mod manifest;
mod published;
mod record;
mod security;
mod version;
mod withdraw;

use std::fmt::Write as _;
use std::path::Path;
use std::process::{Command, ExitCode};

use manifest::Kind;
use record::{Change, Config};
use version::Version;

const USAGE: &str = "\
usage: pubrel <command> [--package NAME]

  init                    write release.json and an empty unreleased.json
  add CATEGORY CHANGE...  record a change: changed, removed, added, fixed,
                          security, or internal (no bump)
  summary [TEXT...]       set the next release's summary, a paragraph shown
                          above its changes; with no text, print it
  next                    the version the unreleased changes imply
  check BASE              CI: a code change records what it changes; a release
                          is the bump its changes require, and published
  prepare [--no-pr]       cut a release: publish the package publet, commit,
                          and open a release PR
  withdraw PR|BRANCH      after closing a release's pull request unmerged:
                          keep pub from publishing its objects again
  changelog               regenerate each package's changelog from its publet
  tag                     CI on main: tag and release what was published
  --version, version      this pubrel's version
  --help, -h, help        this text

A repository whose release.json lists several `packages` names the one
add, summary and prepare act on with --package NAME (or --package=NAME).
next reports every package unless one is named; check, changelog and tag
act on every package.";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let package = take_package(&mut args);
    let package = package.as_deref();
    let result = match args.first().map(String::as_str) {
        Some("--version" | "version") => {
            println!("pubrel {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some("init") => init(),
        Some("add") => add(&args[1..], package),
        Some("summary") => summary(&args[1..], package),
        Some("next") => next(package),
        Some("check") => args.get(1).map_or_else(
            || Err("usage: pubrel check BASE".to_owned()),
            |base| check(base),
        ),
        Some("prepare") => prepare(!args.iter().any(|a| a == "--no-pr"), package),
        Some("withdraw") => args.get(1).map_or_else(
            || Err("usage: pubrel withdraw PR|BRANCH".to_owned()),
            |which| {
                let cfg = packages()?
                    .into_iter()
                    .next()
                    .ok_or("release.json lists no package")?;
                withdraw::withdraw(&cfg, which).map(|line| println!("{line}"))
            },
        ),
        Some("changelog") => changelog(),
        Some("tag") => tag(),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn root() -> Result<std::path::PathBuf, String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let top = run("git", &["rev-parse", "--show-toplevel"], &here)?;
    Ok(std::path::PathBuf::from(top.trim()))
}

/// `--package NAME` or `--package=NAME`, taken out of the arguments.
fn take_package(args: &mut Vec<String>) -> Option<String> {
    if let Some(i) = args.iter().position(|a| a == "--package") {
        let name = args.get(i + 1).cloned();
        args.drain(i..(i + 2).min(args.len()));
        return name;
    }
    let i = args.iter().position(|a| a.starts_with("--package="))?;
    let arg = args.remove(i);
    arg.strip_prefix("--package=").map(str::to_owned)
}

fn packages() -> Result<Vec<Config>, String> {
    Config::read_all(&root()?)
}

/// The package a command acts on: the one named, or the repository's only one.
fn config(package: Option<&str>) -> Result<Config, String> {
    let mut all = packages()?;
    let names = all
        .iter()
        .map(|c| c.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    match package {
        Some(name) => {
            let i = all.iter().position(|c| c.name == name).ok_or_else(|| {
                format!("release.json lists no package {name} (it lists {names})")
            })?;
            Ok(all.swap_remove(i))
        }
        None if all.len() == 1 => all
            .pop()
            .ok_or_else(|| "release.json lists no package".to_owned()),
        None => Err(format!(
            "this repository releases several packages ({names}); name one with --package NAME"
        )),
    }
}

fn run(program: &str, args: &[&str], dir: &Path) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// A file's content at a git ref, if it exists there.
fn at(cfg: &Config, rev: &str, path: &str) -> Option<String> {
    run("git", &["show", &format!("{rev}:{path}")], &cfg.root).ok()
}

fn unreleased(cfg: &Config) -> Result<Vec<Change>, String> {
    let path = cfg.root.join(cfg.unreleased_rel());
    record::changes(std::fs::read_to_string(path).ok().as_deref())
}

fn unreleased_summary(cfg: &Config) -> Option<String> {
    let path = cfg.root.join(cfg.unreleased_rel());
    record::summary(std::fs::read_to_string(path).ok().as_deref())
}

fn categories(rows: &[Change]) -> Vec<&str> {
    rows.iter().map(|r| r.category.as_str()).collect()
}

fn lock_text(cfg: &Config) -> Option<String> {
    std::fs::read_to_string(cfg.root.join(cfg.lock_rel())).ok()
}

// --- commands -----------------------------------------------------------------

fn init() -> Result<(), String> {
    let root = root()?;
    let path = root.join("release.json");
    if path.exists() {
        return Err("release.json already exists".to_owned());
    }
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("package")
        .to_owned();
    let (kind, manifest) = if root.join("pyproject.toml").exists() {
        ("pyproject", "pyproject.toml")
    } else if root.join("package.json").exists() {
        ("npm", "package.json")
    } else {
        ("cargo-package", "Cargo.toml")
    };
    let mut json = serde_json::json!({
        "name": name,
        "package": format!("pkg.{name}"),
        "tag": "PKG-CHANGE-01-2026",
        "corpus": "corpus",
        "manifest": {"kind": kind, "path": manifest},
        "code": ["src/", manifest],
    });
    // The key releases must be signed by: the corpus's author, if it has one.
    let author = std::fs::read_to_string(root.join("corpus/.publet/config"))
        .ok()
        .and_then(|c| {
            c.lines()
                .find_map(|l| l.strip_prefix("author=").map(str::to_owned))
        });
    if let Some(author) = author {
        json["key"] = serde_json::Value::String(author);
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&json).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    let cfg = config(None)?;
    record::write_changes(&cfg.root.join(cfg.unreleased_rel()), &[], None)?;
    println!("release.json and {} written.", cfg.unreleased_rel());
    println!(
        "Set `tag` to a Section 9.1 tag (PKG-XXXXXX-MM-YYYY) and `code` to the paths users run."
    );
    if !cfg.corpus_dir().join("corpus.json").exists() {
        println!("No corpus yet: in {}, run", cfg.corpus_dir().display());
        println!("  pub corpus init --parent=base=<path to publet-corpus>");
        println!("  pub sign --generate-key --principal=human --label=<you>");
        println!("  pub policy --root=<the key it printed>");
    }
    Ok(())
}

fn add(args: &[String], package: Option<&str>) -> Result<(), String> {
    let (Some(category), text) = (args.first(), args.get(1..).unwrap_or_default().join(" ")) else {
        return Err("usage: pubrel add CATEGORY CHANGE...".to_owned());
    };
    if !version::known(category) {
        return Err(format!(
            "unknown category `{category}` (known: {})",
            version::CATEGORIES.join(", ")
        ));
    }
    if text.trim().is_empty() {
        return Err("say what changed: pubrel add CATEGORY CHANGE...".to_owned());
    }
    let cfg = config(package)?;
    let mut rows = unreleased(&cfg)?;
    rows.push(Change {
        category: category.clone(),
        change: text.trim().to_owned(),
    });
    record::write_changes(
        &cfg.root.join(cfg.unreleased_rel()),
        &rows,
        unreleased_summary(&cfg).as_deref(),
    )?;
    println!("{} change(s) unreleased", rows.len());
    Ok(())
}

fn summary(args: &[String], package: Option<&str>) -> Result<(), String> {
    let cfg = config(package)?;
    let text = args.join(" ");
    if text.trim().is_empty() {
        match unreleased_summary(&cfg) {
            Some(summary) => println!("{summary}"),
            None => println!("no summary yet: pubrel summary TEXT..."),
        }
        return Ok(());
    }
    record::write_changes(
        &cfg.root.join(cfg.unreleased_rel()),
        &unreleased(&cfg)?,
        Some(text.trim()),
    )?;
    println!("summary set for the next release of {}", cfg.name);
    Ok(())
}

fn next(package: Option<&str>) -> Result<(), String> {
    let chosen = match package {
        Some(_) => vec![config(package)?],
        None => packages()?,
    };
    let several = chosen.len() > 1;
    for cfg in &chosen {
        let previous = record::published_version(cfg, lock_text(cfg).as_deref())?;
        let rows = unreleased(cfg)?;
        let next = version::next(previous, cfg.manifest_version()?, &categories(&rows));
        // With several packages, one with nothing to release is reported,
        // not an error that hides the others.
        match (next, several) {
            (Ok(next), true) => println!("{} {next}", cfg.name),
            (Ok(next), false) => println!("{next}"),
            (Err(why), true) => println!("{}: {why}", cfg.name),
            (Err(why), false) => return Err(why),
        }
    }
    Ok(())
}

fn check(base: &str) -> Result<(), String> {
    let all = packages()?;
    let root = all
        .first()
        .map(|c| c.root.clone())
        .ok_or("release.json lists no package")?;
    let changed = run(
        "git",
        &["diff", "--name-only", &format!("{base}...HEAD")],
        &root,
    )?;
    let several = all.len() > 1;
    let who = |cfg: &Config| {
        if several {
            format!("{}: ", cfg.name)
        } else {
            String::new()
        }
    };

    // A release pull request releases one or more packages and changes
    // nothing else, so each release is checked and nothing more.
    let mut released = Vec::new();
    for cfg in &all {
        if let Some(published) = check_release(cfg, base)? {
            released.push(format!(
                "{}release {published}: the bump its changes require, and published",
                who(cfg)
            ));
        }
    }
    if !released.is_empty() {
        for line in released {
            println!("{line}");
        }
        return Ok(());
    }

    // Otherwise every package whose code the change touches records what
    // changed in its own list. A path can belong to more than one: a crate
    // released on its own is also compiled into the workspace's binaries.
    let mut missing = Vec::new();
    for cfg in &all {
        let code: Vec<&str> = changed.lines().filter(|p| cfg.owns(p)).collect();
        let rel = cfg.unreleased_rel();
        let before = record::changes(at(cfg, base, &rel).as_deref())?;
        let after = unreleased(cfg)?;
        let added = after.iter().filter(|r| !before.contains(r)).count();
        if !code.is_empty() && added == 0 {
            let flag = if several {
                format!(" --package {}", cfg.name)
            } else {
                String::new()
            };
            missing.push(format!(
                "{}this change touches {} but records no change in {rel}; \
                 run `pubrel add{flag} CATEGORY \"what changed\"`",
                who(cfg),
                code.join(", ")
            ));
        } else if code.is_empty() {
            println!("{}ok: no code changed", who(cfg));
        } else {
            println!("{}ok: {added} change(s) recorded", who(cfg));
            if let Some(line) = check_api(cfg, &categories(&after))? {
                println!("{}ok: {line}", who(cfg));
            }
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing.join("\n"))
    }
}

/// Check `cfg`'s public API against its last release, if release.json asks
/// for that and there is a release to check against.
fn check_api(cfg: &Config, categories: &[&str]) -> Result<Option<String>, String> {
    if !cfg.semver_checks {
        return Ok(None);
    }
    match record::published_version(cfg, lock_text(cfg).as_deref())? {
        Some(baseline) => api::check(cfg, &baseline, categories).map(Some),
        None => Ok(Some(
            "API: not checked, as there is no earlier release to check it against".to_owned(),
        )),
    }
}

/// The claims a release of `cfg` must carry, signed: its identity, its
/// changes, and -- when release.json declares security checks -- the record
/// of them. Its summary must be signed too, when it has one: the lock keeps
/// the last summary after a release without one, and that summary belongs
/// to an earlier release, signed when that one was.
fn record_parts(cfg: &Config, lock: Option<&str>) -> Vec<&'static str> {
    let mut parts = vec!["identity", "release"];
    if !cfg.security.is_empty() {
        parts.push("security");
    }
    let summary = record::locked(lock, &format!("{}#summary", cfg.package));
    let version = record::published_version(cfg, lock).ok().flatten();
    if let (Some(cid), Some(version)) = (summary, version)
        && published::content_of(cfg, &cid)
            .is_some_and(|content| record::summarizes(cfg, &content, &version))
    {
        parts.push("summary");
    }
    parts
}

fn missing_part(cfg: &Config, part: &str) -> String {
    if part == "security" {
        format!(
            "release.json declares security checks, but the release of {} records none; \
             cut it with `pubrel prepare`, which runs them",
            cfg.name
        )
    } else {
        format!("the lock records no {}#{part}", cfg.package)
    }
}

/// If this branch releases `cfg`, check the release and return its version:
/// signed by the package's key, exactly the bump its changes require, with
/// the manifest saying so and nothing left unreleased.
fn check_release(cfg: &Config, base: &str) -> Result<Option<Version>, String> {
    let rel = cfg.unreleased_rel();
    let base_lock = at(cfg, base, &cfg.lock_rel());
    let base_identity = record::locked(base_lock.as_deref(), &format!("{}#identity", cfg.package));
    let now_lock = lock_text(cfg);
    let now_identity = record::locked(now_lock.as_deref(), &format!("{}#identity", cfg.package));
    if now_identity == base_identity {
        return Ok(None);
    }
    for part in record_parts(cfg, now_lock.as_deref()) {
        let cid = record::locked(now_lock.as_deref(), &format!("{}#{part}", cfg.package))
            .ok_or_else(|| missing_part(cfg, part))?;
        published::require_signed(cfg, &cid)?;
    }
    // The release's own records name the commit it was cut from; any other
    // record it adds belongs to another attempt at a release.
    let identity = now_identity.as_deref().unwrap_or_default();
    if let Some(commit) = published::content_of(cfg, identity)
        .as_deref()
        .and_then(withdraw::commit_named)
    {
        let objects = cfg.corpus.join("objects");
        let added = run(
            "git",
            &[
                "diff",
                "--name-only",
                "--diff-filter=A",
                &format!("{base}...HEAD"),
                "--",
                &objects.to_string_lossy(),
            ],
            &cfg.root,
        )?;
        let cids: Vec<String> = added
            .lines()
            .filter_map(|p| Path::new(p).file_name()?.to_str()?.strip_suffix(".cbor"))
            .map(|stem| stem.replace('_', ":"))
            .collect();
        let strays = withdraw::strays(cids.iter().map(String::as_str), commit, |cid| {
            published::content_of(cfg, cid)
        });
        if !strays.is_empty() {
            return Err(format!(
                "this release also adds records of another attempt at a release:\n  {}\n\
                 Close it, run `pubrel withdraw PR` for that attempt's pull request, and \
                 prepare again",
                strays.join("\n  ")
            ));
        }
    }
    let previous = record::published_version(cfg, base_lock.as_deref())?;
    let published = record::published_version(cfg, now_lock.as_deref())?
        .ok_or("the package identity is recorded but states no version")?;
    let base_manifest = at(cfg, base, &cfg.manifest_rel())
        .ok_or("the manifest does not exist at the base")
        .and_then(|t| {
            manifest::read(&t, cfg.kind).map_err(|_| "the base manifest has no version")
        })?;
    let release = record::releases(cfg)
        .into_iter()
        .find(|r| r.version == published)
        .ok_or_else(|| format!("no published release record states {published}"))?;
    let cats: Vec<&str> = release.rows.iter().map(|(c, _)| c.as_str()).collect();
    let expected = version::next(previous.clone(), base_manifest, &cats)?;
    if published != expected {
        return Err(format!(
            "the release states {published}, but its changes after {} make it {expected}",
            previous.map_or_else(|| "nothing".to_owned(), |p| p.to_string())
        ));
    }
    if cfg.semver_checks
        && let Some(previous) = &previous
    {
        println!("{}: {}", cfg.name, api::check(cfg, previous, &cats)?);
    }
    let manifest_now = cfg.manifest_version()?;
    if manifest_now != published {
        return Err(format!(
            "the release is {published}, but {} states {manifest_now}",
            cfg.manifest_rel()
        ));
    }
    if !unreleased(cfg)?.is_empty() {
        return Err(format!(
            "a release moves every unreleased change into the package publet; {rel} still lists some"
        ));
    }
    Ok(Some(published))
}

fn today() -> Result<String, String> {
    let out = run("date", &["-u", "+%Y-%m-%d"], Path::new("."))?;
    Ok(out.trim().to_owned())
}

fn prepare(open_pr: bool, package: Option<&str>) -> Result<(), String> {
    let all = packages()?;
    let cfg = config(package)?;
    record::require_pub()?;
    if !run("git", &["status", "--porcelain"], &cfg.root)?
        .trim()
        .is_empty()
    {
        return Err("the tree is not clean; a release is cut from a clean main".to_owned());
    }
    run("git", &["fetch", "origin", "main"], &cfg.root)?;
    let head = run("git", &["rev-parse", "HEAD"], &cfg.root)?
        .trim()
        .to_owned();
    let main = run("git", &["rev-parse", "origin/main"], &cfg.root)?
        .trim()
        .to_owned();
    if head != main {
        return Err("HEAD is not origin/main; a release is cut from the current main".to_owned());
    }
    let rows = unreleased(&cfg)?;
    let previous = record::published_version(&cfg, lock_text(&cfg).as_deref())?;
    let release = version::next(
        previous.clone(),
        cfg.manifest_version()?,
        &categories(&rows),
    )?;
    // The checks run on the tree being released, before anything changes:
    // a release is cut only when they pass. A non-release pull request has
    // already checked the API, but main may have moved since.
    if let Some(line) = check_api(&cfg, &categories(&rows))? {
        println!("{line}");
    }
    let security = security::run(&cfg.security, &cfg.root, &cfg.name, &release.to_string())?;
    let date = today()?;
    let tag = cfg.git_tag(&release);
    let branch = format!("release/{tag}");
    println!(
        "releasing {} {} -> {release} from {}",
        cfg.name,
        previous.map_or_else(|| "(first release)".to_owned(), |p| p.to_string()),
        head.chars().take(12).collect::<String>()
    );
    run("git", &["checkout", "-b", &branch], &cfg.root)?;
    println!(
        "on {branch}; if a step below fails: git checkout -- . && git clean -fd {} && \
         git checkout main && git branch -D {branch}",
        cfg.corpus.display()
    );

    let brief = unreleased_summary(&cfg);
    let source = record::package_source(
        &cfg,
        &release,
        &date,
        &head,
        &rows,
        brief.as_deref(),
        &security,
    );
    std::fs::create_dir_all(cfg.package_dir()).map_err(|e| e.to_string())?;
    std::fs::write(
        cfg.package_dir().join("publet.json"),
        serde_json::to_string_pretty(&source).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    record::write_changes(&cfg.root.join(cfg.unreleased_rel()), &[], None)?;
    set_versions(&cfg, &all, &release)?;
    manifest::refresh_lock(&cfg.root, cfg.kind, &cfg.name, &cfg.manifest, &release)?;
    record::run_pub(&cfg.corpus_dir(), &["build"])?;
    print!(
        "{}",
        record::run_pub(&cfg.corpus_dir(), &["publish", &cfg.package])?
    );
    exported_cleanly(&cfg, &head)?;
    write_changelog(&cfg)?;

    run("git", &["add", "-A"], &cfg.root)?;
    let message = format!(
        "Release {tag}\n\nPublishes {} with {} {release}.",
        cfg.package, cfg.name
    );
    run("git", &["commit", "-m", &message], &cfg.root)?;
    if !open_pr {
        println!("committed on {branch}; push it and open a PR to release");
        return Ok(());
    }
    run("git", &["push", "-u", "origin", &branch], &cfg.root)?;
    let notes = notes(&cfg, &release)?;
    let pr = run(
        "gh",
        &[
            "pr",
            "create",
            "--base",
            "main",
            "--title",
            &format!("Release {tag}"),
            "--body",
            &notes,
        ],
        &cfg.root,
    )?;
    print!("{pr}");
    Ok(())
}

/// After `pub publish`: every claim this release records is in `objects/`,
/// and nothing exported records another attempt at a release. `pub`'s
/// store keeps the objects of a release that was cut but never merged,
/// and exports them again unless they are withdrawn.
fn exported_cleanly(cfg: &Config, head: &str) -> Result<(), String> {
    let lock = lock_text(cfg);
    for part in record_parts(cfg, lock.as_deref()) {
        let slug = format!("{}#{part}", cfg.package);
        let cid = record::locked(lock.as_deref(), &slug).ok_or_else(|| missing_part(cfg, part))?;
        if !published::held(cfg, &cid) {
            return Err(format!(
                "{slug} was published as {cid}, but it is not in objects/. It reproduces an \
                 object listed in {}/.draft-discards; remove that line and prepare again",
                cfg.corpus.display()
            ));
        }
    }
    let objects = cfg.corpus.join("objects");
    let new = run(
        "git",
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "--",
            &objects.to_string_lossy(),
        ],
        &cfg.root,
    )?;
    let cids: Vec<String> = new
        .lines()
        .filter_map(|p| Path::new(p).file_name()?.to_str()?.strip_suffix(".cbor"))
        .map(|stem| stem.replace('_', ":"))
        .collect();
    let strays = withdraw::strays(cids.iter().map(String::as_str), head, |cid| {
        published::content_of(cfg, cid)
    });
    if strays.is_empty() {
        return Ok(());
    }
    Err(format!(
        "pub exported records of another attempt at a release, which its store still holds:\n  \
         {}\nUndo this attempt as shown above, then run `pubrel withdraw PR` for the release's \
         closed pull request (or its branch), and prepare again",
        strays.join("\n  ")
    ))
}

/// Set `cfg`'s manifest to `release`. Crates released with versions of
/// their own keep their pins through a workspace bump; releasing one of
/// them pins its new version in the workspace that depends on it.
fn set_versions(cfg: &Config, all: &[Config], release: &Version) -> Result<(), String> {
    let own: Vec<String> = all
        .iter()
        .filter(|o| o.name != cfg.name && o.kind == Kind::CargoPackage)
        .map(Config::manifest_dir)
        .collect();
    let manifest_path = cfg.root.join(&cfg.manifest);
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    std::fs::write(
        &manifest_path,
        manifest::write(&text, cfg.kind, release, &own)?,
    )
    .map_err(|e| e.to_string())?;
    if cfg.kind == Kind::CargoPackage {
        for workspace in all.iter().filter(|o| o.kind == Kind::CargoWorkspace) {
            let path = workspace.root.join(&workspace.manifest);
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            if let Some(pinned) = manifest::pin(&text, &cfg.manifest_dir(), release)? {
                std::fs::write(&path, pinned).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn notes(cfg: &Config, release: &Version) -> Result<String, String> {
    let found = record::releases(cfg)
        .into_iter()
        .find(|r| r.version == *release)
        .ok_or_else(|| format!("no published release record states {release}"))?;
    let mut text = record::section(&found);
    if let Some(install) = &cfg.install {
        let _ = write!(
            text,
            "Install:\n\n```\n{}\n```\n",
            install
                .replace("{tag}", &cfg.git_tag(release))
                .replace("{version}", &release.to_string())
        );
    }
    Ok(text)
}

fn changelog() -> Result<(), String> {
    for cfg in packages()? {
        write_changelog(&cfg)?;
    }
    Ok(())
}

fn write_changelog(cfg: &Config) -> Result<(), String> {
    let body: String = record::releases(cfg).iter().map(record::section).collect();
    let text = format!(
        "# Changelog\n\nGenerated by `pubrel changelog` from the package publet `{}` [{}] in \
         `{}/`. Do not edit: each release is a published version of that publet, and this \
         file is a view of it.\n\n{body}",
        cfg.package,
        cfg.tag,
        cfg.corpus.display()
    );
    let path = cfg.root.join(&cfg.changelog);
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{} written", cfg.changelog.display());
    Ok(())
}

fn tag() -> Result<(), String> {
    for cfg in packages()? {
        tag_one(&cfg)?;
    }
    Ok(())
}

/// Tag and release `cfg`'s manifest version, if it is published and untagged.
fn tag_one(cfg: &Config) -> Result<(), String> {
    let version = cfg.manifest_version()?;
    let tag = cfg.git_tag(&version);
    if !run("git", &["tag", "-l", &tag], &cfg.root)?
        .trim()
        .is_empty()
    {
        println!("{tag} already exists; nothing to do");
        return Ok(());
    }
    let lock = lock_text(cfg);
    if record::published_version(cfg, lock.as_deref())?.as_ref() != Some(&version) {
        println!(
            "{version} is not a published release of {}; nothing to tag",
            cfg.package
        );
        return Ok(());
    }
    let identity =
        record::locked(lock.as_deref(), &format!("{}#identity", cfg.package)).unwrap_or_default();
    let release =
        record::locked(lock.as_deref(), &format!("{}#release", cfg.package)).unwrap_or_default();
    for part in record_parts(cfg, lock.as_deref()) {
        let cid = record::locked(lock.as_deref(), &format!("{}#{part}", cfg.package))
            .ok_or_else(|| missing_part(cfg, part))?;
        published::require_signed(cfg, &cid)?;
    }
    let annotation = format!(
        "{} {version}\n\npackage {} [{}]\nidentity {identity}\nrelease  {release}",
        cfg.name, cfg.package, cfg.tag
    );
    run("git", &["tag", "-a", &tag, "-m", &annotation], &cfg.root)?;
    run("git", &["push", "origin", &tag], &cfg.root)?;
    let notes = notes(cfg, &version)?;
    run(
        "gh",
        &[
            "release",
            "create",
            &tag,
            "--title",
            &format!("{} {version}", cfg.name),
            "--notes",
            &notes,
        ],
        &cfg.root,
    )?;
    println!("tagged and released {tag}");
    jsdelivr::refresh(&cfg.root, &tag, &version, &cfg.jsdelivr);
    Ok(())
}
