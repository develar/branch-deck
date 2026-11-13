use std::fs::OpenOptions;
use std::os::unix::fs::PermissionsExt;

use pretty_assertions::assert_eq;

use super::*;
use host_link::refusal::{Code, Refusal};

fn spec(run: &str) -> RunSpec {
  RunSpec {
    run: run.to_owned(),
    repo: "~/repo".to_owned(),
    affinity: "checkout-0123456789ab".to_owned(),
    commit: "a".repeat(40),
    command: vec!["true".to_owned()],
    fetch: vec!["out/bt-runs".to_owned()],
    ..RunSpec::default()
  }
}

#[test]
fn an_exit_record_has_the_json_shape_of_the_spec() {
  assert_eq!(serde_json::to_string(&ExitRecord::Code { code: 3 }).expect("json"), r#"{"code":3}"#);
  let refused = ExitRecord::refused(&Refusal::new(Code::LeaseFailed, "no pool"));
  let json = serde_json::to_string(&refused).expect("json");
  assert_eq!(json, r#"{"refused":{"code":"lease_failed","message":"no pool"}}"#);
  assert_eq!(serde_json::from_str::<ExitRecord>(&json).expect("parsed"), refused);
  assert_eq!(refused.code(), REFUSED_EXIT);
}

fn phase(name: &str, ms: u64) -> Phase {
  Phase { name: name.to_owned(), ms }
}

#[test]
fn the_phases_go_to_phases_json_as_they_end_and_to_exit_json_at_the_end() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let runs = Runs::new(dir.path().join("runs"));
  let (run, _lock) = runs.create(&spec("1-1")).expect("created");
  assert_eq!(run.info().expect("the info").phases, Vec::new());
  run.write_phases(&[phase("lock", 3)]).expect("written");
  assert_eq!(run.info().expect("the info").phases, vec![phase("lock", 3)]);
  assert_eq!(fs::read_to_string(run.path().join("phases.json")).expect("the phases"), r#"[{"name":"lock","ms":3}]"#);

  run.write_phases(&[phase("lock", 3), phase("sweep", 0)]).expect("written");
  run.write_exit(&ExitRecord::Code { code: 2 }).expect("written");
  assert_eq!(
    fs::read_to_string(run.path().join("exit.json")).expect("the end"),
    r#"{"code":2,"phases":[{"name":"lock","ms":3},{"name":"sweep","ms":0}]}"#
  );
  let (end, phases) = run.exit_with_phases().expect("read").expect("ended");
  assert_eq!(end, ExitRecord::Code { code: 2 });
  assert_eq!(phases, vec![phase("lock", 3), phase("sweep", 0)]);
  // the end is the source after it: a later write of `phases.json` changes nothing.
  run.write_phases(&[]).expect("written");
  assert_eq!(run.info().expect("the info").phases.len(), 2);

  let (refused_run, _lock) = runs.create(&spec("1-2")).expect("created");
  refused_run.write_exit(&ExitRecord::refused(&Refusal::new(Code::PackMissing, "no pack"))).expect("written");
  assert_eq!(
    refused_run.exit_with_phases().expect("read"),
    Some((ExitRecord::refused(&Refusal::new(Code::PackMissing, "no pack")), Vec::new()))
  );
}

#[test]
fn an_exit_json_without_phases_has_none() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let runs = Runs::new(dir.path().join("runs"));
  let (run, _lock) = runs.create(&spec("1-1")).expect("created");
  fs::write(run.path().join("exit.json"), r#"{"refused":{"code":"agent_error","message":"lost"}}"#).expect("written");
  let (end, phases) = run.exit_with_phases().expect("read").expect("ended");
  assert_eq!(end.code(), REFUSED_EXIT);
  assert_eq!(phases, Vec::new());
}

#[test]
fn a_new_run_has_its_spec_and_an_empty_log_and_refuses_a_second_start() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let runs = Runs::new(dir.path().join("runs"));
  let (run, lock) = runs.create(&spec("1-2")).expect("created");
  assert_eq!(run.spec().expect("the spec"), spec("1-2"));
  assert_eq!(fs::read(run.events_path()).expect("the log"), b"");
  assert_eq!(run.exit().expect("read"), None);
  assert_eq!(runs.create(&spec("1-2")).expect_err("refused").code, Code::RunExists);
  assert_eq!(runs.create(&spec("../x")).expect_err("refused").code, Code::BadRunId);
  assert_eq!(runs.get("1-2").expect("kept").path(), run.path());
  assert_eq!(runs.get("1-3").expect_err("refused").code, Code::UnknownRun);
  assert_eq!(runs.get("a/b").expect_err("refused").code, Code::BadRunId);

  let info = run.info().expect("the info");
  drop(lock);
  assert_eq!(info.exit, None);
  run.write_exit(&ExitRecord::Code { code: 4 }).expect("written");
  assert_eq!(run.info().expect("the info").exit, Some(4));
  assert_eq!(runs.list().len(), 1);
}

#[test]
fn a_run_is_alive_while_its_lock_is_held_and_a_lost_run_ends_once() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let runs = Runs::new(dir.path().join("runs"));
  let (run, lock) = runs.create(&spec("9-1")).expect("created");
  let mode = |path: &Path| fs::metadata(path).expect("the metadata").permissions().mode() & 0o777;
  assert_eq!(mode(&dir.path().join("runs")), 0o700);
  assert_eq!(mode(run.path()), 0o700);
  for file in ["spec.json", "events.log", "lock"] {
    assert_eq!(mode(&run.path().join(file)), 0o600, "{file}");
  }
  assert!(run.is_alive());
  assert!(!run.end_if_lost());
  assert!(!run.cancel_requested());
  run.request_cancel().expect("requested");
  assert!(run.cancel_requested());
  drop(lock);
  // a fork of another test thread shares the open lock file until its exec, so the lock can stay for a moment.
  let dropped = std::time::Instant::now();
  while run.is_alive() {
    assert!(dropped.elapsed() < Duration::from_mins(1), "the run stays alive after the drop of its lock");
    std::thread::sleep(Duration::from_millis(5));
  }
  assert!(run.end_if_lost());
  assert!(matches!(run.exit().expect("read"), Some(ExitRecord::Refused { .. })));
  assert_eq!(mode(&run.path().join("exit.json")), 0o600);
  assert!(!run.end_if_lost(), "an ended run stays as it is");
}

#[test]
fn a_run_that_ended_a_day_ago_is_removed() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let runs = Runs::new(dir.path().join("runs"));
  let (ended, _ended_lock) = runs.create(&spec("ended")).expect("created");
  ended.write_exit(&ExitRecord::Code { code: 0 }).expect("written");
  let _going = runs.create(&spec("going")).expect("created");
  let now = SystemTime::now();
  assert_eq!(runs.remove_expired(now), Vec::<String>::new());
  assert_eq!(runs.remove_expired(now + KEEP + Duration::from_secs(1)), vec!["ended".to_owned()]);
  assert_eq!(runs.list().len(), 1, "a run that goes on stays");
}

#[test]
fn the_log_gives_complete_events_with_their_end_offsets() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let path = dir.path().join("events.log");
  let log = EventLog::open(&path).expect("opened");
  log.line(Stream::Stdout, "one");
  log.line(Stream::Stderr, "two \"quoted\"");
  let events = read_events(&path, 0, 1 << 20).expect("read");
  assert_eq!(events.len(), 2);
  assert_eq!(
    events[0].0,
    Event {
      stream: Stream::Stdout,
      piece: Piece::of(b"one", false),
    }
  );
  let first_end = events[0].1;
  assert!(is_line_start(&path, first_end).expect("checked"));
  assert!(!is_line_start(&path, first_end - 1).expect("checked"));
  assert!(!is_line_start(&path, events[1].1 + 1).expect("checked"));
  let rest = read_events(&path, first_end, 1 << 20).expect("read");
  assert_eq!(rest.len(), 1);
  assert_eq!(rest[0].0.piece.text, "two \"quoted\"");

  // a line without its end waits for a later read.
  OpenOptions::new()
    .append(true)
    .open(&path)
    .and_then(|mut file| file.write_all(br#"{"stream":"stdout","#))
    .expect("appended");
  assert_eq!(read_events(&path, events[1].1, 1 << 20).expect("read"), Vec::new());

  log.close();
  log.line(Stream::Stdout, "dropped");
  assert_eq!(read_events(&path, 0, 1 << 20).expect("read").len(), 2);

  // an event of an older supervisor has no `partial`: a whole line
  let older = dir.path().join("older.log");
  fs::write(&older, b"{\"stream\":\"stdout\",\"text\":\"older\"}\n").expect("written");
  assert_eq!(read_events(&older, 0, 1 << 20).expect("read")[0].0.piece, Piece::of(b"older", false));
}

/// The bytes of each stream of the log: the bytes of each piece, each followed by its line end unless it is partial
fn streams(path: &Path) -> (Vec<u8>, Vec<u8>) {
  let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
  for (event, _) in read_events(path, 0, 1 << 20).expect("read") {
    let bytes = event.piece.into_stream_bytes().expect("the bytes");
    match event.stream {
      Stream::Stdout => stdout.extend(bytes),
      Stream::Stderr => stderr.extend(bytes),
    }
  }
  (stdout, stderr)
}

/// A piece longer than an event becomes several events, cut between two characters, and only the last one has the line
/// end. A source goes on with its open line, and the line of another source of the stream ends that line first
#[test]
fn a_long_piece_becomes_several_events_and_another_source_ends_an_open_line() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let path = dir.path().join("events.log");
  let log = EventLog::open(&path).expect("opened");
  let text = format!("a{}", "é".repeat(MAX_EVENT_BYTES));
  log.line(Stream::Stdout, &text);
  let pieces: Vec<(usize, bool)> = read_events(&path, 0, 1 << 20)
    .expect("read")
    .iter()
    .map(|(event, _)| (event.piece.text.len(), event.piece.partial))
    .collect();
  assert_eq!(pieces, [(MAX_EVENT_BYTES - 1, true), (MAX_EVENT_BYTES, true), (2, false)], "each é stays whole");
  assert_eq!(streams(&path).0, format!("{text}\n").into_bytes());

  let dir = tempfile::tempdir().expect("a temporary directory");
  let path = dir.path().join("events.log");
  let log = EventLog::open(&path).expect("opened");
  let (out, err) = (log.source(), log.source());
  log.write(out, Stream::Stdout, b"one ", true);
  log.write(err, Stream::Stderr, b"half", true);
  log.line(Stream::Stderr, "brd-host: a line of the agent");
  log.write(out, Stream::Stdout, b"line", false);
  log.write(err, Stream::Stderr, b"\xff rest", false);
  log.write(err, Stream::Stderr, b"no end", true);
  assert_eq!(streams(&path), (b"one line\n".to_vec(), b"half\nbrd-host: a line of the agent\n\xff rest\nno end".to_vec()));
}

/// A cut moves the start of a character to the next piece, and leaves bytes that are not UTF-8 where they are
#[test]
fn a_cut_keeps_each_character_whole() {
  let smile = "😀".as_bytes();
  for kept in 1..smile.len() {
    assert_eq!(char_cut(&[b"ab", &smile[..kept]].concat()), 2, "{kept} bytes of 😀");
  }
  assert_eq!(char_cut(&[b"ab", smile].concat()), 6);
  assert_eq!(char_cut("aé".as_bytes()), 3);
  assert_eq!(char_cut(b"a\xc3"), 1);
  assert_eq!(char_cut(b"\xff\xff\xff\xff"), 4);
  assert_eq!(char_cut(b"\x80\x80\x80\x80"), 4);
  assert_eq!(char_cut(b""), 0);
}
