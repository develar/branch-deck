use super::{CERT_FILE, Fingerprint, Identity, Imported, KEY_FILE};
use pretty_assertions::assert_eq;
use std::fs;
use std::thread;

#[test]
fn a_fingerprint_round_trips_through_its_hex_text() {
  let fingerprint = Fingerprint::of(b"a certificate");
  let text = fingerprint.to_string();
  assert_eq!(text.len(), 64);
  assert_eq!(text.parse::<Fingerprint>(), Ok(fingerprint));
  assert_eq!(fingerprint.short(), text[..12]);
  assert_eq!(serde_json::to_string(&fingerprint).expect("json"), format!("\"{text}\""));
}

#[test]
fn a_text_that_is_not_64_hex_digits_is_refused() {
  for text in ["", "abc", &"g".repeat(64), &"a".repeat(63), &"é".repeat(32)] {
    assert!(text.parse::<Fingerprint>().is_err(), "{text:?}");
  }
}

/// The first calls of many threads at once answer one identity, and its key and its certificate belong together
#[test]
#[expect(clippy::needless_collect, reason = "every call starts before the first join, so they run at the same time")]
fn first_calls_at_the_same_time_answer_one_identity() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let created: Vec<Identity> = thread::scope(|scope| {
    let calls: Vec<_> = (0..8).map(|_| scope.spawn(|| Identity::load_or_create(dir.path()))).collect();
    calls.into_iter().map(|call| call.join().expect("a thread").expect("an identity")).collect()
  });
  let loaded = Identity::load(dir.path()).expect("loaded");
  for identity in &created {
    assert_eq!(identity, &loaded, "each call answers the pair on disk");
  }
  let names: Vec<String> = fs::read_dir(dir.path())
    .expect("the directory")
    .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
    .filter(|name| name.contains(".tmp-"))
    .collect();
  assert!(names.is_empty(), "temporary files stay: {names:?}");
}

#[test]
fn an_identity_is_created_once_and_loaded_after() {
  let home = tempfile::tempdir().expect("a temporary directory");
  let dir = home.path().join("bd-host/agent");
  let created = Identity::load_or_create(&dir).expect("created");
  let loaded = Identity::load_or_create(&dir).expect("loaded");
  assert_eq!(loaded.fingerprint(), created.fingerprint());
  assert_ne!(Identity::generate().expect("another").fingerprint(), created.fingerprint());
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = |path: &std::path::Path| fs::metadata(path).expect("metadata").permissions().mode() & 0o777;
    for name in [KEY_FILE, CERT_FILE] {
      assert_eq!(mode(&dir.join(name)), 0o600, "{name}");
    }
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(dir.parent().expect("a parent")), 0o700);
  }
}

/// `setup --import-yonder`: the identity of yonder comes as it is, and an identity of bd-host stays
#[test]
fn an_import_copies_the_identity_of_yonder_once_and_keeps_another_one() {
  let home = tempfile::tempdir().expect("a temporary directory");
  let yonder = home.path().join(".config/yonder");
  let bd_host = home.path().join(".config/bd-host");
  assert_eq!(Identity::import(&yonder, &bd_host).expect("no yonder"), Imported::Missing);
  assert!(!bd_host.exists(), "an import without a source writes nothing");

  let source = Identity::load_or_create(&yonder).expect("the identity of yonder");
  assert_eq!(Identity::import(&yonder, &bd_host).expect("copied"), Imported::Copied);
  assert_eq!(Identity::load(&bd_host).expect("the copy"), source);
  assert_eq!(Identity::import(&yonder, &bd_host).expect("again"), Imported::Same);

  let other = home.path().join("other");
  let own = Identity::load_or_create(&other).expect("an identity of bd-host");
  assert_eq!(Identity::import(&yonder, &other).expect("kept"), Imported::Kept);
  assert_eq!(Identity::load(&other).expect("the own identity"), own);
}
