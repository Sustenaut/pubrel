//! `pubrel`: version a package by what changed, and record each release as
//! a version of its package publet.
//!
//! A release of a package is a version of a publet -- `pkg.<name>` in the
//! package's own corpus -- whose `identity` claim states the version, tag,
//! and commit, and whose `release` claim lists what changed. The publet's
//! lineage is the release history; `CHANGELOG.md` is generated from it.
//! Changes not yet released accumulate in an `unreleased/` directory beside
//! the package publet's source, one file per change, so pull requests that
//! each record a change never conflict: workshop data, never published.
//!
//! The version follows from the categories of what changed, never from
//! judgement at release time (see [`version`]).

mod api;
mod jsdelivr;
mod manifest;
mod order;
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

  init                    write release.json
  add CATEGORY CHANGE...  record a change, in a file of its own: changed,
                          removed, added, fixed, security, or internal
                          (no bump)
  summary [TEXT...]       set the next release's summary, a paragraph shown
                          above its changes; with no text, print it
  next                    the version the unreleased changes imply
  check BASE              CI: a code change records what it changes; a release
                          is the bump its changes require, and published
  prepare [--no-pr]       cut a release: publish the package publet, commit,
                          and open a release PR
  prepare --all [--no-pr] cut one release PR for every package with
                          unreleased changes, in dependency order
  withdraw PR|BRANCH      after closing a release's pull request unmerged:
                          keep pub from publishing its objects again
  changelog               regenerate each package's changelog from its publet
  tag                     CI on main: tag and release what was published
  --version, version      this pubrel's version
  --help, -h, help        this text

A repository whose release.json lists several `packages` names the one
add, summary and prepare act on with --package NAME (or --package=NAME),
or releases every package that has changes with prepare --all. next
reports every package unless one is named; check, changelog and tag act
on every package.";

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
        Some("prepare") => prepare(
            !args.iter().any(|a| a == "--no-pr"),
            package,
            args.iter().any(|a| a == "--all"),
        ),
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

/// The files of `cfg`'s unreleased directory that each record a change,
/// in name order: the order they were recorded in.
fn fragment_names(cfg: &Config) -> Result<Vec<String>, String> {
    let dir = cfg.root.join(cfg.unreleased_rel());
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if Path::new(&name).extension().is_some_and(|e| e == "json") {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Every change not yet released: those the pubrel 0.5 `unreleased.json`
/// still lists, then one per file of the unreleased directory, in name
/// order.
fn unreleased(cfg: &Config) -> Result<Vec<Change>, String> {
    let legacy = cfg.root.join(cfg.legacy_rel());
    let mut rows = record::changes(std::fs::read_to_string(legacy).ok().as_deref())?;
    let dir = cfg.root.join(cfg.unreleased_rel());
    for name in fragment_names(cfg)? {
        let text = std::fs::read_to_string(dir.join(&name)).map_err(|e| format!("{name}: {e}"))?;
        rows.push(record::change(
            &text,
            &format!("{}/{name}", cfg.unreleased_rel()),
        )?);
    }
    Ok(rows)
}

/// The next release's summary: its own file's, or what the pubrel 0.5
/// `unreleased.json` gives.
fn unreleased_summary(cfg: &Config) -> Option<String> {
    std::fs::read_to_string(cfg.root.join(cfg.summary_rel()))
        .ok()
        .as_deref()
        .and_then(record::brief)
        .or_else(|| {
            let legacy = std::fs::read_to_string(cfg.root.join(cfg.legacy_rel())).ok();
            record::summary(legacy.as_deref())
        })
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
    println!(
        "release.json written; `pubrel add` records each change in {}/.",
        cfg.unreleased_rel()
    );
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
    let row = Change {
        category: category.clone(),
        change: text.trim().to_owned(),
    };
    let stamp = run("date", &["-u", "+%Y%m%dT%H%M%SZ"], Path::new("."))?;
    let dir = cfg.root.join(cfg.unreleased_rel());
    let name = record::fragment_name(stamp.trim(), &row.change, |n| dir.join(n).exists());
    record::write_change(&dir.join(&name), &row)?;
    println!("{}/{name} written", cfg.unreleased_rel());
    println!("{} change(s) unreleased", unreleased(&cfg)?.len());
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
    record::write_text(
        &cfg.root.join(cfg.summary_rel()),
        &format!("{}\n", text.trim()),
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
        let after = unreleased(cfg)?;
        let added = recorded_since(cfg, base)?;
        if !code.is_empty() && added == 0 {
            let flag = if several {
                format!(" --package {}", cfg.name)
            } else {
                String::new()
            };
            missing.push(format!(
                "{}this change touches {} but records no change in {rel}/; \
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

/// How many changes this branch records that `base` does not: a file of
/// the unreleased directory `base` lacks, or a row of the pubrel 0.5
/// `unreleased.json` it lacks.
fn recorded_since(cfg: &Config, base: &str) -> Result<usize, String> {
    let dir = cfg.unreleased_rel();
    let listed = run(
        "git",
        &["ls-tree", "--name-only", base, "--", &format!("{dir}/")],
        &cfg.root,
    )?;
    let before: Vec<&str> = listed
        .lines()
        .filter_map(|p| Path::new(p).file_name()?.to_str())
        .collect();
    let files = fragment_names(cfg)?
        .iter()
        .filter(|n| !before.contains(&n.as_str()))
        .count();
    let rel = cfg.legacy_rel();
    let old = record::changes(at(cfg, base, &rel).as_deref())?;
    let legacy = cfg.root.join(&rel);
    let rows = record::changes(std::fs::read_to_string(legacy).ok().as_deref())?
        .iter()
        .filter(|r| !old.contains(r))
        .count();
    Ok(files + rows)
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
            "a release moves every unreleased change into the package publet, but {rel}/ \
             still records some: main gained changes after the release was cut. Close the \
             release, run `pubrel withdraw PR`, and prepare again from the current main"
        ));
    }
    Ok(Some(published))
}

fn today() -> Result<String, String> {
    let out = run("date", &["-u", "+%Y-%m-%d"], Path::new("."))?;
    Ok(out.trim().to_owned())
}

/// A package `prepare` is releasing: its unreleased changes, the version
/// it was last released at, and the version they make.
struct Cut {
    cfg: Config,
    rows: Vec<Change>,
    previous: Option<Version>,
    release: Version,
}

/// The packages `prepare` releases: the one named (or the only one), or
/// with `every`, each package that has changes to release, in the order
/// that releases each after the packages it depends on.
fn to_release(all: &[Config], package: Option<&str>, every: bool) -> Result<Vec<Cut>, String> {
    let chosen = if every {
        if package.is_some() {
            return Err(
                "prepare --all releases every package with changes; it takes no --package"
                    .to_owned(),
            );
        }
        packages()?
    } else {
        vec![config(package)?]
    };
    let mut cuts = Vec::new();
    let mut idle = Vec::new();
    for cfg in chosen {
        let rows = unreleased(&cfg)?;
        let previous = record::published_version(&cfg, lock_text(&cfg).as_deref())?;
        match version::next(
            previous.clone(),
            cfg.manifest_version()?,
            &categories(&rows),
        ) {
            Ok(release) => cuts.push(Cut {
                cfg,
                rows,
                previous,
                release,
            }),
            Err(why) if every => idle.push(format!("{}: {why}", cfg.name)),
            Err(why) => return Err(why),
        }
    }
    if cuts.is_empty() {
        return Err(format!(
            "no package has changes to release:\n  {}",
            idle.join("\n  ")
        ));
    }
    for line in &idle {
        println!("{line}");
    }
    if cuts.len() < 2 {
        return Ok(cuts);
    }
    let root = cuts.first().map(|c| c.cfg.root.clone()).unwrap_or_default();
    let members = if all
        .iter()
        .any(|c| c.kind == Kind::CargoWorkspace || c.kind == Kind::CargoPackage)
    {
        order::members(&run(
            "cargo",
            &[
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--offline",
            ],
            &root,
        )?)?
    } else {
        Vec::new()
    };
    let names: Vec<String> = order::by_dependency(
        &cuts.iter().map(|c| &c.cfg).collect::<Vec<_>>(),
        all,
        &members,
    )?
    .into_iter()
    .map(|c| c.name.clone())
    .collect();
    let mut ordered = Vec::with_capacity(cuts.len());
    for name in names {
        if let Some(i) = cuts.iter().position(|c| c.cfg.name == name) {
            ordered.push(cuts.swap_remove(i));
        }
    }
    Ok(ordered)
}

fn prepare(open_pr: bool, package: Option<&str>, every: bool) -> Result<(), String> {
    let all = packages()?;
    let cuts = to_release(&all, package, every)?;
    let root = cuts
        .first()
        .map(|c| c.cfg.root.clone())
        .ok_or("no package to release")?;
    record::require_pub()?;
    if !run("git", &["status", "--porcelain"], &root)?
        .trim()
        .is_empty()
    {
        return Err("the tree is not clean; a release is cut from a clean main".to_owned());
    }
    run("git", &["fetch", "origin", "main"], &root)?;
    let head = run("git", &["rev-parse", "HEAD"], &root)?.trim().to_owned();
    let main = run("git", &["rev-parse", "origin/main"], &root)?
        .trim()
        .to_owned();
    if head != main {
        return Err("HEAD is not origin/main; a release is cut from the current main".to_owned());
    }
    // The checks run on the tree being released, before anything changes:
    // a release is cut only when every package's pass. A non-release pull
    // request has already checked the API, but main may have moved since.
    let mut security = Vec::with_capacity(cuts.len());
    for cut in &cuts {
        if let Some(line) = check_api(&cut.cfg, &categories(&cut.rows))? {
            println!("{line}");
        }
        security.push(security::run(
            &cut.cfg.security,
            &root,
            &cut.cfg.name,
            &cut.release.to_string(),
        )?);
    }
    let date = today()?;
    let tags: Vec<String> = cuts.iter().map(|c| c.cfg.git_tag(&c.release)).collect();
    let branch = format!("release/{}", tags.join("+"));
    for cut in &cuts {
        println!(
            "releasing {} {} -> {} from {}",
            cut.cfg.name,
            cut.previous
                .as_ref()
                .map_or_else(|| "(first release)".to_owned(), ToString::to_string),
            cut.release,
            head.chars().take(12).collect::<String>()
        );
    }
    run("git", &["checkout", "-b", &branch], &root)?;
    let corpora: Vec<String> = {
        let mut dirs: Vec<String> = cuts
            .iter()
            .map(|c| c.cfg.corpus.display().to_string())
            .collect();
        dirs.dedup();
        dirs
    };
    println!(
        "on {branch}; if a step below fails: git checkout -- . && git clean -fd {} && \
         git checkout main && git branch -D {branch}",
        corpora.join(" ")
    );

    for (cut, security) in cuts.iter().zip(&security) {
        cut_one(cut, &all, &date, &head, security)?;
    }

    run("git", &["add", "-A"], &root)?;
    let what: Vec<String> = cuts
        .iter()
        .map(|c| format!("{} with {} {}", c.cfg.package, c.cfg.name, c.release))
        .collect();
    let message = format!(
        "Release {}\n\nPublishes {}.",
        tags.join(", "),
        what.join("; ")
    );
    run("git", &["commit", "-m", &message], &root)?;
    if !open_pr {
        println!("committed on {branch}; push it and open a PR to release");
        return Ok(());
    }
    run("git", &["push", "-u", "origin", &branch], &root)?;
    open_release_pr(&cuts, &tags, &root)
}

/// Open the pull request whose merge is the release: one package's notes,
/// or each package's under its name.
fn open_release_pr(cuts: &[Cut], tags: &[String], root: &Path) -> Result<(), String> {
    let body = match cuts {
        [cut] => notes(&cut.cfg, &cut.release)?,
        several => {
            let mut text = String::new();
            for cut in several {
                let _ = write!(
                    text,
                    "# {} {}\n\n{}\n",
                    cut.cfg.name,
                    cut.release,
                    notes(&cut.cfg, &cut.release)?
                );
            }
            text
        }
    };
    let pr = run(
        "gh",
        &[
            "pr",
            "create",
            "--base",
            "main",
            "--title",
            &format!("Release {}", tags.join(", ")),
            "--body",
            &body,
        ],
        root,
    )?;
    print!("{pr}");
    Ok(())
}

/// Publish one package's release on the release branch: its package
/// publet's new version, its manifest and lockfile, and its changelog.
fn cut_one(
    cut: &Cut,
    all: &[Config],
    date: &str,
    head: &str,
    security: &[security::Row],
) -> Result<(), String> {
    let cfg = &cut.cfg;
    let brief = unreleased_summary(cfg);
    let source = record::package_source(
        cfg,
        &cut.release,
        date,
        head,
        &cut.rows,
        brief.as_deref(),
        security,
    );
    std::fs::create_dir_all(cfg.package_dir()).map_err(|e| e.to_string())?;
    std::fs::write(
        cfg.package_dir().join("publet.json"),
        serde_json::to_string_pretty(&source).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    clear_unreleased(cfg)?;
    set_versions(cfg, all, &cut.release)?;
    manifest::refresh_lock(&cfg.root, cfg.kind, &cfg.name, &cfg.manifest, &cut.release)?;
    record::run_pub(&cfg.corpus_dir(), &["build"])?;
    print!(
        "{}",
        record::run_pub(&cfg.corpus_dir(), &["publish", &cfg.package])?
    );
    exported_cleanly(cfg, head)?;
    write_changelog(cfg)
}

/// Delete what `cut` has moved into the package publet: each change's
/// file, the summary, and a pubrel 0.5 `unreleased.json`. A release pull
/// request only deletes them, so it conflicts with no pull request that
/// records a change.
fn clear_unreleased(cfg: &Config) -> Result<(), String> {
    let dir = cfg.root.join(cfg.unreleased_rel());
    let mut gone: Vec<std::path::PathBuf> = fragment_names(cfg)?
        .into_iter()
        .map(|n| dir.join(n))
        .collect();
    gone.push(cfg.root.join(cfg.summary_rel()));
    gone.push(cfg.root.join(cfg.legacy_rel()));
    for path in gone.iter().filter(|p| p.exists()) {
        std::fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    // Empty once its changes are released, unless it holds files of its own.
    let _ = std::fs::remove_dir(&dir);
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

/// Tag every package whose release main now holds. One release pull
/// request may release several packages, so one failing does not stop the
/// others: each is tried, and every failure is reported.
fn tag() -> Result<(), String> {
    let all = packages()?;
    let several = all.len() > 1;
    let failed: Vec<String> = all
        .iter()
        .filter_map(|cfg| {
            tag_one(cfg).err().map(|e| {
                if several {
                    format!("{}: {e}", cfg.name)
                } else {
                    e
                }
            })
        })
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("\n"))
    }
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
