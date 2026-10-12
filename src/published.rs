//! Published objects, read and checked in-process with `publet-core`.
//!
//! Reading the package publet back -- its version, its release history --
//! and checking a release is signed need only canonical CBOR, content
//! identifiers, and Ed25519, which is what `publet-core` is. So `check`,
//! `tag`, `next` and `changelog` run without `pub`; only `prepare`, which
//! signs, needs it.

use std::fmt::Write as _;
use std::path::PathBuf;

use publet_core::cbor::Value as Cbor;
use publet_core::{Cid, Object, SigAlg};
use serde_json::{Map, Value, json};

use crate::record::Config;

/// The purpose `pub` signs what it publishes for.
const PURPOSE: &str = "authored";

/// The purpose both keys sign a rotating key object for (publet spec
/// Section 10.5, and `publet_graph::ROTATION_PURPOSE`).
const ROTATION: &str = "rotation";

/// The most rotations followed back from the key `release.json` names.
const MAX_ROTATIONS: usize = 64;

fn path_of(cfg: &Config, cid: &str) -> PathBuf {
    cfg.corpus_dir()
        .join("objects")
        .join(format!("{}.cbor", cid.replace(':', "_")))
}

/// The object `cid` names, from the corpus's objects, after checking the
/// bytes there are the bytes that identifier names.
fn load(cfg: &Config, cid: &str) -> Result<Object, String> {
    let expected: Cid = cid
        .parse()
        .map_err(|_| format!("{cid} is not an identifier"))?;
    let path = path_of(cfg, cid);
    let bytes = std::fs::read(&path).map_err(|_| {
        format!(
            "{cid} is not in {}",
            cfg.corpus_dir().join("objects").display()
        )
    })?;
    let object = Object::parse(&bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .verify(&expected)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(object.into_inner())
}

/// A claim's prose content, from an object's bytes: `None` if the bytes
/// are not an object, or the object is not a `claim.prose`. The bytes are
/// not checked against an identifier; callers only read what a record says.
#[must_use]
pub fn claim_content(bytes: &[u8]) -> Option<String> {
    let unverified = Object::parse(bytes).ok()?;
    let object = unverified.peek();
    if object.kind() != "claim.prose" {
        return None;
    }
    object.body().iter().find_map(|(k, v)| match v {
        Cbor::Text(text) if k == "content" => Some(text.clone()),
        _ => None,
    })
}

/// An object's kind, and its `purpose` if it states one, from its bytes:
/// `None` if they are not an object. Not checked against an identifier.
#[must_use]
pub fn kind_and_purpose(bytes: &[u8]) -> Option<(String, Option<String>)> {
    let unverified = Object::parse(bytes).ok()?;
    let object = unverified.peek();
    let purpose = object.body().iter().find_map(|(k, v)| match v {
        Cbor::Text(text) if k == "purpose" => Some(text.clone()),
        _ => None,
    });
    Some((object.kind().to_owned(), purpose))
}

/// [`claim_content`] of the object `cid` names in the corpus's objects.
#[must_use]
pub fn content_of(cfg: &Config, cid: &str) -> Option<String> {
    claim_content(&std::fs::read(path_of(cfg, cid)).ok()?)
}

/// Whether the corpus's objects hold a file for `cid`.
#[must_use]
pub fn held(cfg: &Config, cid: &str) -> bool {
    path_of(cfg, cid).exists()
}

fn to_json(value: &Cbor) -> Value {
    match value {
        Cbor::Uint(n) => json!(n),
        Cbor::Nint(n) => json!(-1 - i128::from(*n)),
        Cbor::Bytes(b) => {
            let hex = b.iter().fold(String::new(), |mut out, x| {
                let _ = write!(out, "{x:02x}");
                out
            });
            json!({ "$bytes": hex })
        }
        Cbor::Text(t) => json!(t),
        Cbor::Array(items) => Value::Array(items.iter().map(to_json).collect()),
        Cbor::Map(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), to_json(v)))
                .collect::<Map<_, _>>(),
        ),
        Cbor::Bool(b) => json!(b),
        // Null, and any kind of value a later protocol adds.
        _ => Value::Null,
    }
}

/// A published object as JSON, in the shape `pub read --json` gives its
/// `object`: the header fields and the body, bytes as `{"$bytes": hex}`.
///
/// # Errors
///
/// Returns a message if `cid` is not an identifier, the corpus's
/// `objects/` holds no file for it, or the file does not parse as an
/// object whose bytes are the ones `cid` names.
pub fn read_object(cfg: &Config, cid: &str) -> Result<Value, String> {
    let object = load(cfg, cid)?;
    let mut out = Map::new();
    out.insert("pub".into(), json!(object.protocol()));
    out.insert("type".into(), json!(object.kind()));
    out.insert("created".into(), json!(object.created()));
    out.insert("author".into(), json!(object.author().to_string()));
    out.insert(
        "body".into(),
        Value::Object(
            object
                .body()
                .iter()
                .map(|(k, v)| (k.clone(), to_json(v)))
                .collect(),
        ),
    );
    if let Some(prev) = object.prev() {
        out.insert("prev".into(), json!(prev.to_string()));
    }
    if let Some(basis) = object.basis() {
        out.insert("basis".into(), json!(basis.to_string()));
    }
    Ok(Value::Object(out))
}

fn text<'a>(object: &'a Object, field: &str) -> Option<&'a str> {
    match object.body().get(field) {
        Some(Cbor::Text(t)) => Some(t),
        _ => None,
    }
}

fn bytes<'a>(object: &'a Object, field: &str) -> Option<&'a [u8]> {
    match object.body().get(field) {
        Some(Cbor::Bytes(b)) => Some(b),
        _ => None,
    }
}

/// An ed25519 key's public key, from its `key` object.
fn public_key(cfg: &Config, key_cid: &str) -> Result<Vec<u8>, String> {
    let key = load(cfg, key_cid)?;
    if key.kind() != "key" || text(&key, "alg") != Some("ed25519") {
        return Err(format!("{key_cid} is not an ed25519 key"));
    }
    bytes(&key, "pubkey")
        .map(<[u8]>::to_vec)
        .ok_or_else(|| format!("{key_cid} states no public key"))
}

/// Whether a `sig` object in the corpus's objects, by `key_cid`, signs the
/// object `cid` for `purpose` with a signature `public` verifies.
fn signed_by(
    cfg: &Config,
    key_cid: &str,
    public: &[u8],
    cid: &str,
    purpose: &str,
) -> Result<bool, String> {
    let target = load(cfg, cid)?;
    let dir = cfg.corpus_dir().join("objects");
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".cbor") else {
            continue;
        };
        let Ok(sig) = load(cfg, &stem.replace('_', ":")) else {
            continue;
        };
        if sig.kind() != "sig"
            || sig.author().to_string() != key_cid
            || text(&sig, "target") != Some(cid)
        {
            continue;
        }
        let (Some(found), Some(value)) = (text(&sig, "purpose"), bytes(&sig, "value")) else {
            continue;
        };
        if publet_core::verify(
            SigAlg::Ed25519,
            public,
            value,
            found,
            purpose,
            target.bytes(),
        )
        .is_ok()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The key `release.json` names, then each key it rotates from, newest
/// first: (identifier, public key). A rotation is followed only when the
/// newer key's object names the older in `rotates` and is signed, for the
/// rotation purpose, by both keys (publet spec Section 10.5). The chain
/// stops at the first link that is not, or after [`MAX_ROTATIONS`].
fn key_chain(cfg: &Config, key_cid: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut chain = vec![(key_cid.to_owned(), public_key(cfg, key_cid)?)];
    while chain.len() <= MAX_ROTATIONS {
        let (newest, newest_public) = chain.last().cloned().unwrap_or_default();
        let Ok(object) = load(cfg, &newest) else {
            break;
        };
        let Some(previous) = text(&object, "rotates").map(str::to_owned) else {
            break;
        };
        if chain.iter().any(|(k, _)| *k == previous) {
            break;
        }
        let Ok(previous_public) = public_key(cfg, &previous) else {
            break;
        };
        if !(signed_by(cfg, &newest, &newest_public, &newest, ROTATION)?
            && signed_by(cfg, &previous, &previous_public, &newest, ROTATION)?)
        {
            break;
        }
        chain.push((previous, previous_public));
    }
    Ok(chain)
}

/// Check `cid` carries a valid signature by the key `release.json` names,
/// or by a key it rotates from.
///
/// The key is trusted because the repository names it: a change to it is a
/// change to `release.json`, reviewed like any other. Its public key is
/// read from its own `key` object, whose identifier is checked like every
/// other object's. A rotated key carries the trust back along its chain
/// (see `key_chain`), so releases signed before a rotation still verify
/// once `release.json` names the new key.
///
/// If `release.json` names no key, nothing is checked: a warning goes to
/// stderr, and this returns `Ok`.
///
/// # Errors
///
/// Returns a message if the key object cannot be loaded, is not an
/// `ed25519` key, or states no public key; if `cid` cannot be loaded; if
/// the corpus's `objects/` cannot be listed; or if no `sig` object there,
/// by that key, targets `cid` with a signature that verifies.
pub fn require_signed(cfg: &Config, cid: &str) -> Result<(), String> {
    let Some(key_cid) = cfg.key.as_deref() else {
        eprintln!(
            "warning: release.json names no `key`, so {cid}'s signature is not checked; \
             add \"key\": the corpus's author (`author=` in {}/.publet/config)",
            cfg.corpus.display()
        );
        return Ok(());
    };
    let chain = key_chain(cfg, key_cid).map_err(|e| format!("the release key: {e}"))?;
    for (key, public) in &chain {
        if signed_by(cfg, key, public, cid, PURPOSE)? {
            return Ok(());
        }
    }
    let rotated = if chain.len() > 1 {
        format!(
            ", or by a key it rotates from ({} checked)",
            chain.len() - 1
        )
    } else {
        String::new()
    };
    Err(format!(
        "{cid} has no valid signature by {key_cid}, the key release.json names{rotated}"
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use publet_core::{HashAlg, cbor::Value as V};

    /// A corpus in a fresh directory, with `release.json` naming `key`.
    fn corpus(key: &Cid) -> (tempdir::Dir, Config) {
        let dir = tempdir::Dir::new();
        std::fs::create_dir_all(dir.0.join("corpus/objects")).unwrap();
        let json = format!(
            r#"{{"name": "demo", "package": "pkg.demo", "tag": "PKG-DEMODE-10-2026",
                "corpus": "corpus", "manifest": {{"kind": "cargo-package", "path": "Cargo.toml"}},
                "code": ["src/"], "key": "{key}"}}"#
        );
        let cfg = Config::parse_all(&dir.0, &json, "release.json")
            .unwrap()
            .remove(0);
        (dir, cfg)
    }

    fn put(cfg: &Config, bytes: &[u8]) -> Cid {
        let cid = Cid::of(bytes, HashAlg::Sha2_256);
        std::fs::write(path_of(cfg, &cid.to_string()), bytes).unwrap();
        cid
    }

    fn key_bytes(seed: u8, rotates: Option<&Cid>) -> (Vec<u8>, [u8; 32]) {
        let secret = [seed; 32];
        let public = publet_core::verifying_key(SigAlg::Ed25519, &secret).unwrap();
        let author = Cid::of(&public, HashAlg::Sha2_256).to_string();
        let mut b = Object::builder("key", &author)
            .created("2026-10-12T00:00:00Z")
            .field("alg", V::Text("ed25519".into()))
            .field("pubkey", V::Bytes(public))
            .field("principal", V::Text("human".into()));
        if let Some(r) = rotates {
            b = b.field("rotates", V::Text(r.to_string()));
        }
        (b.build().unwrap(), secret)
    }

    fn sig_bytes(
        key: &Cid,
        secret: &[u8; 32],
        purpose: &str,
        target: &Cid,
        bytes: &[u8],
    ) -> Vec<u8> {
        let value = publet_core::sign(SigAlg::Ed25519, secret, purpose, bytes).unwrap();
        Object::builder("sig", &key.to_string())
            .created("2026-10-12T00:00:00Z")
            .field("target", V::Text(target.to_string()))
            .field("alg", V::Text("ed25519".into()))
            .field("value", V::Bytes(value))
            .field("purpose", V::Text(purpose.into()))
            .build()
            .unwrap()
    }

    mod tempdir {
        /// A directory removed when dropped.
        pub(super) struct Dir(pub(super) std::path::PathBuf);
        impl Dir {
            pub(super) fn new() -> Self {
                use std::sync::atomic::{AtomicU64, Ordering};
                static N: AtomicU64 = AtomicU64::new(0);
                let path = std::env::temp_dir().join(format!(
                    "pubrel-test-{}-{}",
                    std::process::id(),
                    N.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir_all(&path).unwrap();
                Self(path)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    // covers: published::require_signed
    // A release signed before a rotation still verifies once release.json
    // names the new key, but only when both keys signed the rotation.
    #[test]
    fn a_release_signed_by_a_rotated_from_key_verifies() {
        let (old_bytes, old_secret) = key_bytes(1, None);
        let old = Cid::of(&old_bytes, HashAlg::Sha2_256);
        let (new_bytes, new_secret) = key_bytes(2, Some(&old));
        let new = Cid::of(&new_bytes, HashAlg::Sha2_256);
        let (_dir, cfg) = corpus(&new);
        put(&cfg, &old_bytes);
        put(&cfg, &new_bytes);
        let record = Object::builder("claim.prose", &old.to_string())
            .created("2026-10-12T00:00:00Z")
            .field("content", V::Text("demo 0.1.0 is released.".into()))
            .build()
            .unwrap();
        let record_cid = put(&cfg, &record);
        put(
            &cfg,
            &sig_bytes(&old, &old_secret, "authored", &record_cid, &record),
        );
        let cid = record_cid.to_string();

        // The new key alone signed the rotation: the chain stops.
        put(
            &cfg,
            &sig_bytes(&new, &new_secret, "rotation", &new, &new_bytes),
        );
        let refused = require_signed(&cfg, &cid).unwrap_err();
        assert!(refused.contains("no valid signature"), "{refused}");

        // A signature by the old key for another purpose is not consent.
        put(
            &cfg,
            &sig_bytes(&old, &old_secret, "authored", &new, &new_bytes),
        );
        assert!(require_signed(&cfg, &cid).is_err());

        put(
            &cfg,
            &sig_bytes(&old, &old_secret, "rotation", &new, &new_bytes),
        );
        assert_eq!(require_signed(&cfg, &cid), Ok(()));
    }

    // covers: published::require_signed
    // A record signed by a key the release key does not rotate from is
    // refused, and says how many rotations it followed.
    #[test]
    fn a_key_outside_the_chain_is_refused() {
        let (stranger_bytes, stranger_secret) = key_bytes(3, None);
        let stranger = Cid::of(&stranger_bytes, HashAlg::Sha2_256);
        let (key, _) = key_bytes(4, None);
        let key_cid = Cid::of(&key, HashAlg::Sha2_256);
        let (_dir, cfg) = corpus(&key_cid);
        put(&cfg, &key);
        put(&cfg, &stranger_bytes);
        let record = Object::builder("claim.prose", &stranger.to_string())
            .created("2026-10-12T00:00:00Z")
            .field("content", V::Text("demo 0.1.0 is released.".into()))
            .build()
            .unwrap();
        let record_cid = put(&cfg, &record);
        put(
            &cfg,
            &sig_bytes(
                &stranger,
                &stranger_secret,
                "authored",
                &record_cid,
                &record,
            ),
        );
        let refused = require_signed(&cfg, &record_cid.to_string()).unwrap_err();
        assert!(refused.ends_with("the key release.json names"), "{refused}");
    }

    // covers: published::kind_and_purpose
    #[test]
    fn kind_and_purpose_read_from_bytes() {
        let (key, secret) = key_bytes(5, None);
        let cid = Cid::of(&key, HashAlg::Sha2_256);
        assert_eq!(kind_and_purpose(&key), Some(("key".to_owned(), None)));
        let sig = sig_bytes(&cid, &secret, "rotation", &cid, &key);
        assert_eq!(
            kind_and_purpose(&sig),
            Some(("sig".to_owned(), Some("rotation".to_owned())))
        );
        assert_eq!(kind_and_purpose(b"not an object"), None);
    }
}
