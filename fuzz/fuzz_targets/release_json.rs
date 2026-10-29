//! release.json, as `pubrel check` reads it from a pull request's branch.
//! Reading any text must give packages or an error, never a panic.
#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use pubrel_fuzz::record::Config;
use pubrel_fuzz::version::Version;

fuzz_target!(|text: &str| {
    let Ok(all) = Config::parse_all(Path::new("/repo"), text, "release.json") else {
        return;
    };
    for cfg in &all {
        let _ = cfg.git_tag(&Version::new(1, 2, 3));
        let _ = cfg.manifest_dir();
        let _ = cfg.owns("src/main.rs");
        let _ = cfg.unreleased_rel();
        let _ = cfg.summary_rel();
        let _ = cfg.legacy_rel();
        let _ = cfg.lock_rel();
        let _ = cfg.manifest_rel();
    }
});
