use super::*;
use git_ops::lease_slot::STATE_DIR;
use pretty_assertions::assert_eq;
use std::time::SystemTime;

/// A record names what a fetch of its run takes and reads back as it was written; a record of another shape refuses
/// with `config`; the records, the patches, the logs, the ends and the locks of a week ago go, and the other files stay
#[test]
fn a_run_record_names_what_a_fetch_takes_and_old_records_go() {
  let base = tempfile::tempdir().unwrap();
  let files = RunFiles::new(&base.path().join(".git").join(STATE_DIR).join("host/runs")).unwrap();
  assert!(files.dir().is_dir(), "{}", files.dir().display());
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
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(files.dir().join("1-1.json")).unwrap().permissions().mode() & 0o777, 0o600);
  }

  std::fs::write(files.dir().join("2-2.json"), "{\"commit\": \"c\", \"host\": \"h\"}").unwrap();
  let refusal = files.read("2-2").unwrap_err();
  assert_eq!(refusal.code, Code::Config);
  assert!(refusal.message.contains("is not a run record"), "{}", refusal.message);

  let week_ago = SystemTime::now() - KEEP - Duration::from_secs(60);
  for name in ["3-3.json", "3-3.patch", "3-3.log", "3-3.end.json", "3-3.lock", "note.txt"] {
    File::create(files.dir().join(name)).unwrap().set_modified(week_ago).unwrap();
  }
  RunFiles::new(files.dir()).unwrap();
  let mut left: Vec<String> = std::fs::read_dir(files.dir())
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  left.sort();
  assert_eq!(left, ["1-1.json", "2-2.json", "note.txt"]);
}

/// The end of a run reads back as it was written, and one process at a time holds the lock of the end of a run: a second
/// one waits up to its deadline, or until its stop, and gets none; the lock goes with the drop. On paused time, so the
/// deadline of the wait is the one of the test
#[tokio::test(start_paused = true)]
async fn the_end_of_a_run_reads_back_and_one_process_at_a_time_holds_its_lock() {
  let base = tempfile::tempdir().unwrap();
  let files = RunFiles::new(&base.path().join("runs")).unwrap();
  assert_eq!(files.read_end("1-1").unwrap(), None);
  let end = RunEnd {
    exit: 1,
    command_exit: Some(1),
    refusal: None,
    phases: vec![Phase {
      name: "command".to_owned(),
      ms: 10,
    }],
    fetched: vec!["out/a.txt".to_owned()],
    applied_files: None,
    forgotten: true,
  };
  files.write_end("1-1", &end).unwrap();
  assert_eq!(files.read_end("1-1").unwrap(), Some(end));
  assert_eq!(files.log("1-1"), files.dir().join("1-1.log"));

  let poll = Duration::from_millis(1);
  let held = files.lock("1-1", None, poll, &|| false).await.unwrap().expect("a free lock");
  let started = Instant::now();
  let deadline = Some(started + Duration::from_millis(20));
  // a second open file of the same process stands for another process: the lock is of the open file
  assert!(files.lock("1-1", deadline, poll, &|| false).await.unwrap().is_none(), "held until the deadline");
  assert_eq!(started.elapsed(), Duration::from_millis(20));
  assert!(files.lock("1-1", None, poll, &|| true).await.unwrap().is_none(), "the stop ends the wait");
  assert!(files.lock("2-2", None, poll, &|| false).await.unwrap().is_some(), "each run has a lock of its own");
  drop(held);
  assert!(files.lock("1-1", deadline, poll, &|| false).await.unwrap().is_some());
}
