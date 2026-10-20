//! A manifest (Cargo.toml, pyproject.toml or package.json), its version, and the next
//! version a set of changes makes from it. Any manifest text and any
//! version must give a result or an error, never a panic: a panic is
//! where an overflow would surface, since pubrel's release builds check
//! for it.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use pubrel_fuzz::manifest::{self, Kind};
use pubrel_fuzz::version::{self, CATEGORIES, Version};

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    kind: u8,
    text: &'a str,
    previous: Option<(u64, u64, u64)>,
    categories: Vec<u8>,
}

fuzz_target!(|input: Input| {
    let kind = match input.kind % 4 {
        0 => Kind::CargoWorkspace,
        1 => Kind::CargoPackage,
        2 => Kind::Pyproject,
        _ => Kind::Npm,
    };
    let release = Version::new(1, 2, 3);
    let read = manifest::read(input.text, kind);
    let _ = manifest::write(input.text, kind, &release, &["crates/a".to_owned()]);
    let _ = manifest::pin(input.text, "crates/a", &release);
    let _ = version::parse(input.text);

    let categories: Vec<&str> = input
        .categories
        .iter()
        .map(|c| CATEGORIES[usize::from(*c) % CATEGORIES.len()])
        .collect();
    let previous = input.previous.map(|(a, b, c)| Version::new(a, b, c));
    // Whether or not the text held a version: the arithmetic is reached
    // with any previous version, which comes from a release record.
    let current = read.unwrap_or(Version::new(0, 1, 0));
    let _ = version::next(previous, current, &categories);
});
