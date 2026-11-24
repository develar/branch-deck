use crate::background_kill::{BackgroundKill, MEMORY, claim_notice, read, record, starts_app};

const BUILD: &str = "1.2.0+abc";

fn kill(at_ms: u64, build: &str) -> BackgroundKill {
  BackgroundKill { at_ms, build: build.to_owned() }
}

#[test]
fn a_bridge_starts_the_app_but_within_a_day_of_a_kill_of_its_build() {
  let day = u64::try_from(MEMORY.as_millis()).unwrap();
  let now = 10 * day;
  // (the record, whether the bridge starts the app)
  let cases = [
    (None, true),
    (Some(kill(now - 30_000, BUILD)), false),
    (Some(kill(now - day + 1, BUILD)), false),
    (Some(kill(now - day, BUILD)), true),
    // another build may be allowed in the background
    (Some(kill(now - 30_000, "1.1.0+def")), true),
    // a clock set back: the kill is recent
    (Some(kill(now + 5_000, BUILD)), false),
  ];
  for (record, expected) in cases {
    assert_eq!(starts_app(record.as_ref(), now, BUILD), expected, "{record:?}");
  }
}

#[test]
fn the_record_is_read_as_the_app_wrote_it_and_a_missing_or_cut_one_is_none() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(read(dir.path()), None);
  record(dir.path(), &kill(1_000, BUILD)).unwrap();
  assert_eq!(read(dir.path()), Some(kill(1_000, BUILD)));
  std::fs::write(dir.path().join("backend-killed.json"), "{\"atMs\":").unwrap();
  assert_eq!(read(dir.path()), None);
}

#[test]
fn only_the_first_claim_of_a_record_tells_it_and_a_new_record_is_told_again() {
  let dir = tempfile::tempdir().unwrap();
  let first = kill(1_000, BUILD);
  assert!(claim_notice(dir.path(), &first));
  assert!(!claim_notice(dir.path(), &first));
  let next = kill(2_000, BUILD);
  assert!(claim_notice(dir.path(), &next));
  assert!(!claim_notice(dir.path(), &next));
}
