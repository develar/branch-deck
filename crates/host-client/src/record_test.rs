use super::*;
use pretty_assertions::assert_eq;
use std::time::SystemTime;

/// A record names what a fetch of its run takes and reads back as it was written; a record of another shape refuses
/// with `config`; the records and the patches of a week ago go, and the other files stay
#[test]
fn a_run_record_names_what_a_fetch_takes_and_old_records_go() {
  let base = tempfile::tempdir().unwrap();
  let files = RunFiles::new(&base.path().join(".git/branch-deck/host/runs")).unwrap();
  assert!(files.dir().is_dir(), "{}", files.dir().display());
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(files.dir()).unwrap().permissions().mode() & 0o777, 0o700);
  }
  assert_eq!(files.read("1-1").unwrap(), None);
  let record = RunRecord {
    commit: "c".to_owned(),
    fetch: vec!["out/bt-runs".to_owned()],
    outputs: vec!["results".to_owned()],
    changes: true,
  };
  assert!(record.asks_for_files());
  assert!(!RunRecord::default().asks_for_files());
  files.write("1-1", &record).unwrap();
  assert_eq!(files.read("1-1").unwrap(), Some(record));
  assert_eq!(files.patch("1-1"), files.dir().join("1-1.patch"));
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(files.dir().join("1-1.json")).unwrap().permissions().mode() & 0o777, 0o600);
  }

  std::fs::write(files.dir().join("2-2.json"), "{\"commit\": \"c\", \"host\": \"h\"}").unwrap();
  let refusal = files.read("2-2").unwrap_err();
  assert_eq!(refusal.code, Code::Config);
  assert!(refusal.message.contains("is not a run record"), "{}", refusal.message);

  let week_ago = SystemTime::now() - KEEP - Duration::from_secs(60);
  for name in ["3-3.json", "3-3.patch", "note.txt"] {
    std::fs::File::create(files.dir().join(name)).unwrap().set_modified(week_ago).unwrap();
  }
  RunFiles::new(files.dir()).unwrap();
  let mut left: Vec<String> = std::fs::read_dir(files.dir())
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  left.sort();
  assert_eq!(left, ["1-1.json", "2-2.json", "note.txt"]);
}
