//! Security checks a release must pass, and the signed record of them.
//!
//! `release.json` declares the checks -- commands such as
//! `cargo deny check` or `cargo vet --locked` -- and facts worth keeping
//! with them, such as the revision of the advisory database the check
//! read. `prepare` runs every check before it releases anything, refuses
//! to release if one fails, and records what ran as a `security` claim in
//! the package publet: the command, the tool's version, the outcome and
//! the tool's own summary line, beside each fact. The claim is signed with
//! the release, so what was checked for a version is as verifiable as the
//! version itself.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// A command a release must pass.
#[derive(Debug, Clone)]
pub struct Check {
    /// What the check is called in the record: `advisories`.
    pub name: String,
    /// The shell command that performs it; it passes by exiting zero.
    pub run: String,
    /// A shell command whose first line names the tool and its version.
    pub version: Option<String>,
}

/// A command whose first line of output is recorded beside the checks.
#[derive(Debug, Clone)]
pub struct Fact {
    /// What the fact is called in the record.
    pub name: String,
    /// The shell command whose first non-empty line is the fact's value.
    pub run: String,
}

/// What `release.json`'s `security` section declares.
#[derive(Debug, Clone, Default)]
pub struct Declared {
    /// The checks, run in the order listed.
    pub checks: Vec<Check>,
    /// The facts, read after every check has passed.
    pub facts: Vec<Fact>,
}

impl Declared {
    /// Read a `security` section: `{"checks": [...], "facts": [...]}`.
    /// `None`, or a list that is absent or not a list, declares nothing.
    ///
    /// # Errors
    ///
    /// Returns a message naming what is missing if a check or a fact lacks
    /// a string `name` or `run`.
    pub fn from_json(json: Option<&Value>) -> Result<Self, String> {
        let Some(json) = json else {
            return Ok(Self::default());
        };
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let list = |k: &str| {
            json.get(k)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let checks = list("checks")
            .iter()
            .map(|c| {
                Ok(Check {
                    name: text(c, "name").ok_or("each security check needs a `name`")?,
                    run: text(c, "run").ok_or("each security check needs a `run` command")?,
                    version: text(c, "version"),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let facts = list("facts")
            .iter()
            .map(|f| {
                Ok(Fact {
                    name: text(f, "name").ok_or("each security fact needs a `name`")?,
                    run: text(f, "run").ok_or("each security fact needs a `run` command")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { checks, facts })
    }

    /// Whether nothing is declared: no checks and no facts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty() && self.facts.is_empty()
    }
}

/// One row of the record: a check that passed, or a fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// `check` or `fact`.
    pub kind: String,
    /// The check's or fact's name, as declared.
    pub name: String,
    /// The command run.
    pub command: String,
    /// The tool and its version, for a check; empty for a fact.
    pub tool: String,
    /// For a check, the tool's summary: the last line it printed. For a
    /// fact, its value: the first line printed.
    pub result: String,
}

/// The release being cut, which every check and fact is told through its
/// environment: `PUBREL_PACKAGE` (the package's name) and
/// `PUBREL_VERSION`. A check can then judge that release, such as `pubkit
/// audit --check` finding its audit report.
#[derive(Debug, Clone, Copy)]
struct Release<'a> {
    /// The package's name, as `release.json` gives it.
    package: &'a str,
    /// The version being released.
    version: &'a str,
}

fn shell(command: &str, dir: &Path, release: Release<'_>) -> Result<Output, String> {
    let out = Command::new("sh")
        .args(["-c", command])
        .current_dir(dir)
        .env("PUBREL_PACKAGE", release.package)
        .env("PUBREL_VERSION", release.version)
        .output()
        .map_err(|e| format!("cannot run `{command}`: {e}"))?;
    Ok(Output {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// What a command printed, with its two streams kept apart: tools print
/// their result to stdout and their warnings to stderr, so joining them
/// would let a warning stand in for the result.
struct Output {
    ok: bool,
    stdout: String,
    stderr: String,
}

impl Output {
    /// The stream a result is read from: stdout, unless it printed nothing.
    fn result(&self) -> &str {
        if self.stdout.trim().is_empty() {
            &self.stderr
        } else {
            &self.stdout
        }
    }

    /// Both streams, for a failure's message.
    fn both(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn last_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or_default();
    line.chars().take(200).collect()
}

/// The pubkit release that added `pubkit audit prepare`.
const AUDIT_PREPARE: (u64, u64, u64) = (0, 15, 0);

/// What to do next when a check fails for want of something a person
/// makes: the quality audit `pubkit audit --check` looks for, which
/// `./pubkit audit prepare` starts. When the repository pins a pubkit
/// older than that command, the pin moves first. Empty for any other
/// failure.
fn next_step(check: &Check, output: &str, package: &str, dir: &Path) -> String {
    if !(check.run.contains("pubkit audit --check") && output.contains("no audit/")) {
        return String::new();
    }
    let prepare = format!(
        "`./pubkit audit prepare --package {package}` writes the report with what a machine \
         can check; complete the rules it marks for review, then prepare again"
    );
    match pinned(dir, "pubkit") {
        Some(pin) if triple(&pin).is_some_and(|v| v < AUDIT_PREPARE) => format!(
            "\nnext: this repository pins pubkit {pin}, older than `pubkit audit prepare` \
             (0.15.0): move the pin to a current pubkit and run `make venv`, then {prepare}"
        ),
        _ => format!("\nnext: {prepare}"),
    }
}

/// `major.minor.patch` of a version, ignoring any pre-release or build.
fn triple(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let v = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(v)
}

/// The version of the Python package `name` that `tests/requirements.lock`
/// pins, as `name==1.2.3` or as `name @ <url>/name-1.2.3-<tags>.whl`.
fn pinned(dir: &Path, name: &str) -> Option<String> {
    let lock = std::fs::read_to_string(dir.join("tests/requirements.lock")).ok()?;
    lock.lines().find_map(|line| {
        let line = line.trim().trim_end_matches('\\').trim();
        if let Some(v) = line.strip_prefix(&format!("{name}==")) {
            return Some(v.trim().to_owned());
        }
        let url = line.strip_prefix(&format!("{name} @ "))?;
        let file = url.trim().rsplit('/').next()?;
        let rest = file.strip_prefix(&format!("{name}-"))?;
        rest.split('-').next().map(str::to_owned)
    })
}

/// The version of the Python package `name` installed in `.venv`: the
/// version its `name-1.2.3.dist-info` directory names.
fn installed(dir: &Path, name: &str) -> Option<String> {
    let lib = dir.join(".venv/lib");
    let prefix = format!("{name}-");
    std::fs::read_dir(lib).ok()?.flatten().find_map(|python| {
        std::fs::read_dir(python.path().join("site-packages"))
            .ok()?
            .flatten()
            .find_map(|entry| {
                let file = entry.file_name().to_string_lossy().into_owned();
                let version = file.strip_prefix(&prefix)?.strip_suffix(".dist-info")?;
                Some(version.to_owned())
            })
    })
}

/// Refuse to run checks with a stale `.venv`: a check that runs
/// `.venv/bin/TOOL` runs whichever TOOL `make venv` last installed, which
/// is not the one `tests/requirements.lock` pins once the pin has moved.
/// It would judge the release with an old tool, and record that tool's
/// version as if it were the pinned one.
fn venv_current(declared: &Declared, dir: &Path) -> Result<(), String> {
    let commands = declared
        .checks
        .iter()
        .flat_map(|c| std::iter::once(&c.run).chain(&c.version));
    let mut tools: Vec<&str> = commands
        .flat_map(|c| c.split(".venv/bin/").skip(1))
        .filter_map(|rest| rest.split(|ch: char| ch.is_whitespace()).next())
        .collect();
    tools.sort_unstable();
    tools.dedup();
    for tool in tools {
        let Some(pin) = pinned(dir, tool) else {
            continue;
        };
        match installed(dir, tool) {
            Some(have) if have == pin => {}
            Some(have) => {
                return Err(format!(
                    ".venv has {tool} {have}, but tests/requirements.lock pins {pin}: run \
                     `make venv`, then prepare again"
                ));
            }
            None => {
                return Err(format!(
                    "the security checks run .venv/bin/{tool}, which is not installed: run \
                     `make venv`, then prepare again"
                ));
            }
        }
    }
    Ok(())
}

/// Run every declared check and fact in `dir`.
///
/// A check's result is the last line it printed to stdout, and a fact's
/// value or a tool's version is the first. Each falls back to stderr only
/// when stdout is empty. Every command sees the release being cut in
/// `PUBREL_PACKAGE` and `PUBREL_VERSION`.
///
/// # Errors
///
/// Returns a message naming `make venv` if a check runs a tool from `.venv`
/// whose installed version is not the one `tests/requirements.lock` pins.
/// Returns a message, with the end of its output, if a check exits
/// non-zero; a message if a fact's command exits non-zero; and a message if
/// `sh` cannot be started for any command. A check's `version` command
/// that exits non-zero is not an error: its first line is recorded as it
/// is. A release is cut only when this succeeds.
pub fn run(
    declared: &Declared,
    dir: &Path,
    package: &str,
    version: &str,
) -> Result<Vec<Row>, String> {
    let release = Release { package, version };
    venv_current(declared, dir)?;
    let mut rows = Vec::new();
    for check in &declared.checks {
        println!("security check {}: {}", check.name, check.run);
        let output = shell(&check.run, dir, release)?;
        if !output.ok {
            let both = output.both();
            let tail: Vec<&str> = both.lines().rev().take(12).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            return Err(format!(
                "security check `{}` failed, so nothing is released:\n{}{}",
                check.name,
                tail.join("\n"),
                next_step(check, &both, package, dir)
            ));
        }
        let tool = match &check.version {
            Some(command) => first_line(shell(command, dir, release)?.result()),
            None => String::new(),
        };
        rows.push(Row {
            kind: "check".to_owned(),
            name: check.name.clone(),
            command: check.run.clone(),
            tool,
            result: last_line(output.result()),
        });
    }
    for fact in &declared.facts {
        let output = shell(&fact.run, dir, release)?;
        if !output.ok {
            return Err(format!(
                "security fact `{}` could not be read: `{}` failed",
                fact.name, fact.run
            ));
        }
        rows.push(Row {
            kind: "fact".to_owned(),
            name: fact.name.clone(),
            command: fact.run.clone(),
            tool: String::new(),
            result: first_line(output.result()),
        });
    }
    Ok(rows)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const RELEASE: Release<'static> = Release {
        package: "demo",
        version: "1.2.3",
    };

    // covers: security::run
    // A check is told which release it is checking.
    #[test]
    fn checks_see_the_release_being_cut() {
        let dir = std::env::temp_dir();
        let out = shell("echo \"$PUBREL_PACKAGE $PUBREL_VERSION\"", &dir, RELEASE).unwrap();
        assert_eq!(out.stdout.trim(), "demo 1.2.3");
    }

    // covers: security::first_line, security::last_line
    #[test]
    fn summaries_are_the_first_and_last_lines_printed() {
        assert_eq!(first_line("\n  tool 1.2\nmore\n"), "tool 1.2");
        assert_eq!(last_line("checking\n\nadvisories ok\n\n"), "advisories ok");
        assert_eq!(last_line(""), "");
    }

    #[test]
    fn a_result_is_read_from_stdout_and_warnings_on_stderr_are_ignored() {
        // As cargo deny prints them: the summary on stdout, warnings after
        // it on stderr. Joined, a warning stood in for the summary.
        let dir = std::env::temp_dir();
        let out = shell(
            "printf 'advisories ok\\n'; printf 'duplicate syn\\n' >&2",
            &dir,
            RELEASE,
        )
        .unwrap();
        assert_eq!(last_line(out.result()), "advisories ok");
        let quiet = shell("printf 'only on stderr\\n' >&2", &dir, RELEASE).unwrap();
        assert_eq!(last_line(quiet.result()), "only on stderr");
        assert!(
            shell("echo a; echo b >&2; exit 1", &dir, RELEASE)
                .unwrap()
                .both()
                .contains('b')
        );
    }

    /// A repository directory removed when dropped.
    struct Repo(std::path::PathBuf);

    impl Repo {
        /// A repository whose lock pins pubkit `pin`, with `have` installed
        /// in its `.venv` when given.
        fn new(pin: &str, have: Option<&str>) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "pubrel-security-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(path.join("tests")).unwrap();
            std::fs::write(
                path.join("tests/requirements.lock"),
                format!(
                    "execnet==2.1.2 \\\n    --hash=sha256:00\n\
                     pubkit @ https://github.com/schryer/pubkit/releases/download/v{pin}/\
                     pubkit-{pin}-py3-none-any.whl \\\n    --hash=sha256:00\n"
                ),
            )
            .unwrap();
            if let Some(have) = have {
                let site = path.join(".venv/lib/python3.14/site-packages");
                std::fs::create_dir_all(site.join(format!("pubkit-{have}.dist-info"))).unwrap();
                std::fs::create_dir_all(site.join("execnet-2.1.2.dist-info")).unwrap();
            }
            Self(path)
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn audit() -> Check {
        Check {
            name: "quality audit current".to_owned(),
            run: ".venv/bin/pubkit audit --check".to_owned(),
            version: Some(".venv/bin/pubkit --version".to_owned()),
        }
    }

    // covers: security::next_step
    #[test]
    fn a_missing_audit_report_names_the_command_that_starts_one() {
        let repo = Repo::new("0.15.2", Some("0.15.2"));
        let missing = "pubkit: no audit/demo-1.2.3.md: run the rust-crate-quality agent";
        let next = next_step(&audit(), missing, "demo", &repo.0);
        assert!(next.contains("`./pubkit audit prepare --package demo`"));
        assert!(!next.contains("move the pin"));
        assert_eq!(
            next_step(
                &audit(),
                "pubkit: 2 finding(s) block a release",
                "demo",
                &repo.0
            ),
            ""
        );
        let other = Check {
            name: "advisories".to_owned(),
            run: "cargo deny check".to_owned(),
            version: None,
        };
        assert_eq!(next_step(&other, missing, "demo", &repo.0), "");
    }

    // covers: security::next_step, security::triple
    // `audit prepare` arrived in pubkit 0.15.0; an older pin cannot run it.
    #[test]
    fn a_pin_older_than_audit_prepare_is_moved_first() {
        let repo = Repo::new("0.14.6", Some("0.14.6"));
        let missing = "pubkit: no audit/demo-1.2.3.md: run the rust-crate-quality agent";
        let next = next_step(&audit(), missing, "demo", &repo.0);
        assert!(next.contains("pins pubkit 0.14.6, older than `pubkit audit prepare`"));
        assert!(next.contains("`./pubkit audit prepare --package demo`"));
        assert_eq!(triple("0.15.0"), Some(AUDIT_PREPARE));
        assert_eq!(triple("1.2.3-rc.1"), Some((1, 2, 3)));
        assert_eq!(triple("1.2"), None);
    }

    // covers: security::pinned, security::installed
    #[test]
    fn the_pin_and_the_installed_version_are_read_from_the_lock_and_the_venv() {
        let repo = Repo::new("0.15.2", Some("0.9.1"));
        assert_eq!(pinned(&repo.0, "pubkit").as_deref(), Some("0.15.2"));
        assert_eq!(pinned(&repo.0, "execnet").as_deref(), Some("2.1.2"));
        assert_eq!(pinned(&repo.0, "absent"), None);
        assert_eq!(installed(&repo.0, "pubkit").as_deref(), Some("0.9.1"));
        assert_eq!(installed(&repo.0, "absent"), None);
    }

    // covers: security::venv_current, security::run
    // A stale .venv ran pubkit 0.9.x for releases while the repositories
    // pinned 0.14.6, and the records named 0.9.x as the tool.
    #[test]
    fn checks_do_not_run_with_a_stale_venv() {
        let declared = Declared {
            checks: vec![audit()],
            facts: Vec::new(),
        };
        let stale = Repo::new("0.15.2", Some("0.9.1"));
        let err = run(&declared, &stale.0, "demo", "1.2.3").unwrap_err();
        assert!(err.contains(".venv has pubkit 0.9.1, but tests/requirements.lock pins 0.15.2"));
        assert!(err.contains("make venv"));
        let missing = Repo::new("0.15.2", None);
        assert!(
            venv_current(&declared, &missing.0)
                .unwrap_err()
                .contains("not installed")
        );
        let current = Repo::new("0.15.2", Some("0.15.2"));
        assert_eq!(venv_current(&declared, &current.0), Ok(()));
        // A tool the lock does not pin is not judged.
        let other = Declared {
            checks: vec![Check {
                name: "other".to_owned(),
                run: ".venv/bin/other --check".to_owned(),
                version: None,
            }],
            facts: Vec::new(),
        };
        assert_eq!(venv_current(&other, &stale.0), Ok(()));
    }

    // covers: security::Declared::from_json, security::Declared::is_empty
    #[test]
    fn a_declaration_needs_names_and_commands() {
        let ok = serde_json::json!({"checks": [{"name": "a", "run": "true"}]});
        assert_eq!(Declared::from_json(Some(&ok)).unwrap().checks.len(), 1);
        let bad = serde_json::json!({"checks": [{"run": "true"}]});
        assert!(Declared::from_json(Some(&bad)).is_err());
        assert!(Declared::from_json(None).unwrap().is_empty());
    }
}
