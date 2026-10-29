//! A package's unreleased changes, which a pull request edits by hand or
//! with `pubrel add`: a file of the `unreleased/` directory, a summary, or a
//! pubrel 0.5 unreleased.json. Reading one must give rows or an error,
//! never a panic; naming one, a file name.
#![no_main]

use libfuzzer_sys::fuzz_target;
use pubrel_fuzz::record::{brief, change, changes, fragment_name, summary};

fuzz_target!(|text: &str| {
    let _ = change(text, "fuzz.json");
    let _ = changes(Some(text));
    let _ = summary(Some(text));
    let _ = brief(text);
    let name = fragment_name("20261029T090622Z", text, |_| false);
    assert!(!name.contains('/') && name.ends_with(".json"));
});
