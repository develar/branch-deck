use std::fs::OpenOptions;
use std::os::unix::fs::PermissionsExt;

use pretty_assertions::assert_eq;

use super::*;
use host_link::refusal::{Code, Refusal};

/// The home of an agent for `RunDir::info`, which only expands `~/` by it
fn home() -> Home {
  Home::new(PathBuf::from("/Users/agent"))
}

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
  assert_eq!(run.info(&home()).expect("the info").phases, Vec::new());
  run.write_phases(&[phase("lock", 3)]).expect("written");
  assert_eq!(run.info(&home()).expect("the info").phases, vec![phase("lock", 3)]);
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
  assert_eq!(run.info(&home()).expect("the info").phases.len(), 2);

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

  let info = run.info(&home()).expect("the info");
  drop(lock);
  assert_eq!(info.exit, None);
  run.write_exit(&ExitRecord::Code { code: 4 }).expect("written");
  assert_eq!(run.info(&home()).expect("the info").exit, Some(4));
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
  let history = History::new(&Home::new(dir.path().to_path_buf()));
  assert_eq!(runs.remove_expired(now, &history), Vec::<String>::new());
  assert_eq!(runs.remove_expired(now + KEEP + Duration::from_secs(1), &history), vec!["ended".to_owned()]);
  assert_eq!(runs.list().len(), 1, "a run that goes on stays");
}

/// A forget keeps the run: its stage, its outputs and its changes go, its spec, its end, its slot and its log stay, and
/// the runs list it as forgotten with its end, its repository and its slot
#[test]
fn a_forgotten_run_keeps_its_end_and_its_log_and_its_files_go() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let runs = Runs::new(dir.path().join("runs"));
  let (run, lock) = runs.create(&spec("1-1")).expect("created");
  assert_eq!((run.info(&home()).expect("the info").slot, run.slot()), (None, None), "no slot before the lend");
  run
    .write_slot(lease_pool::host_history::SlotRecord {
      n: 2,
      choice: lease_pool::host_slots::SlotChoice::Idle,
      waited: false,
      cleaned: false,
    })
    .expect("the slot");
  let log = EventLog::open(&run.events_path()).expect("opened");
  log.line(Stream::Stdout, "out");
  log.close();
  fs::create_dir_all(run.stage_path().join("out/bt-runs")).expect("the stage");
  fs::write(run.stage_path().join("out/bt-runs/report.txt"), "r").expect("a staged file");
  fs::create_dir_all(run.outputs_path()).expect("the outputs");
  run.write_output_roots(&["target/nextest".to_owned()]).expect("the output roots");
  fs::write(run.changes_path(), "diff").expect("the changes");
  run.write_exit(&ExitRecord::Code { code: 3 }).expect("written");
  drop(lock);
  assert!(!run.info(&home()).expect("the info").forgotten);

  run.forget().expect("forgotten");
  for gone in [run.stage_path(), run.outputs_path(), run.path().join("outputs.json"), run.changes_path()] {
    assert!(!gone.exists(), "{} is gone", gone.display());
  }
  let info = run.info(&home()).expect("the info");
  assert_eq!((info.exit, info.forgotten), (Some(3), true));
  assert!(info.ended_ms.is_some_and(|ended| ended >= info.started_ms), "{info:?}");
  assert_eq!(
    (info.repo.as_deref(), info.slot.clone()),
    (
      Some("/Users/agent/repo"),
      Some(RunSlot {
        n: 2,
        choice: "idle".to_owned(),
        waited: false,
        cleaned: false
      })
    )
  );
  let events: Vec<String> = read_events(&run.events_path(), 0, 1024)
    .expect("the log")
    .into_iter()
    .map(|(event, _)| event.piece.text)
    .collect();
  assert_eq!(events, ["out"], "a short log stays whole");
}

/// The log of a forgotten run keeps its tail from the start of an event, after a line of the agent that says how much
/// of the log went, and the cut maps the offsets of the log before it; one within the limit stays whole
#[test]
fn the_log_of_a_forgotten_run_keeps_its_tail_from_the_start_of_an_event() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let path = dir.path().join("events.log");
  let cut_path = dir.path().join("events.log.cut");
  let log = EventLog::open(&path).expect("opened");
  for line in ["first", "second", "third"] {
    log.line(Stream::Stdout, line);
  }
  log.close();
  let before = read_events(&path, 0, 4096).expect("the log");
  let (second_end, third_end) = (before[1].1, before[2].1);
  let third = third_end - second_end;
  // the tail starts inside the second event, so only the third stays
  let KeptLog::Cut(cut) = cut_log(&path, &cut_path, third + 5).expect("cut") else {
    panic!("a log longer than the limit is cut");
  };
  assert_eq!(cut.gone, second_end);
  assert_eq!(fs::read(&path).expect("the log").len() as u64, third_end, "the log stays until the rename");
  let events: Vec<(Stream, String, u64)> = read_events(&cut_path, 0, 4096)
    .expect("the cut log")
    .into_iter()
    .map(|(event, end)| (event.stream, event.piece.text, cut.out_of_cut(end)))
    .collect();
  assert_eq!(
    events,
    [
      (
        Stream::Stderr,
        format!("brd-host: the run was forgotten, and its log keeps its last 0 KiB: the first {second_end} bytes of its log are gone"),
        second_end
      ),
      (Stream::Stdout, "third".to_owned(), third_end),
    ],
    "each end is the one of the log before the cut"
  );
  assert_eq!(cut.ino, fs::metadata(&cut_path).expect("the cut log").ino());
  // an offset of the log before the cut: 0 and the part that went at the note, the start of the tail right after it
  assert_eq!([cut.into_cut(0), cut.into_cut(before[0].1), cut.into_cut(second_end)], [0, 0, cut.note]);
  assert_eq!(cut.into_cut(third_end), cut.note + third);
  assert_eq!(cut_log(&path, &cut_path, third_end).expect("a log within the limit"), KeptLog::Whole);
}

/// The run `1-1` in `dir` with a log of 60 000 events, more than [`KEPT_LOG`], and its flock
fn run_with_a_long_log(dir: &Path) -> (RunDir, File) {
  let (run, lock) = Runs::new(dir.join("runs")).create(&spec("1-1")).expect("created");
  let log = EventLog::open(&run.events_path()).expect("opened");
  for line in 0..60_000 {
    log.line(Stream::Stdout, &line.to_string());
  }
  log.close();
  (run, lock)
}

/// A forget cut short (its marker written, its files and its log as they were, or its cut log recorded and not renamed
/// yet) is a forgotten run, and the next forget finishes it; a third changes nothing
#[test]
fn a_forget_cut_short_is_finished_by_the_next_one() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let (run, _lock) = run_with_a_long_log(dir.path());
  fs::create_dir_all(run.stage_path()).expect("the stage");
  fs::write(run.stage_path().join("a.txt"), "a").expect("a staged file");
  run.write_exit(&ExitRecord::Code { code: 0 }).expect("written");
  // the intent of a forget that stopped right after it
  fs::write(run.path().join(FORGOTTEN), "{}").expect("the marker");
  assert!(run.is_forgotten() && run.info(&home()).expect("the info").forgotten);
  assert_eq!(run.kept_log().expect("the marker"), None);

  // a forget that stopped after the record of its cut, before the rename
  let kept = cut_log(&run.events_path(), &run.path().join(CUT_LOG), KEPT_LOG).expect("cut");
  run.write_forgotten(Forgotten { log: Some(kept) }).expect("recorded");
  run.forget().expect("finished");
  assert!(!run.stage_path().exists() && !run.path().join(CUT_LOG).exists());
  assert_eq!(run.kept_log().expect("the marker"), Some(kept));
  let KeptLog::Cut(cut) = kept else { panic!("a log of 60 000 events is cut") };
  assert_eq!(fs::metadata(run.events_path()).expect("the log").ino(), cut.ino, "the cut log is the log");
  let size = fs::metadata(run.events_path()).expect("the log").len();
  assert!(size <= KEPT_LOG + cut.note, "{size}");

  run.forget().expect("forgotten again");
  assert_eq!(fs::metadata(run.events_path()).expect("the log").len(), size, "a log is cut once");
  assert_eq!(run.kept_log().expect("the marker"), Some(kept));

  // a record of another inode, as two forgets at once of a build before the flock left it, is repaired
  run
    .write_forgotten(Forgotten {
      log: Some(KeptLog::Cut(LogCut { ino: cut.ino + 1, ..cut })),
    })
    .expect("a record of another inode");
  run.forget().expect("repaired");
  assert_eq!(run.kept_log().expect("the marker"), Some(kept));
}

/// Two forgets of one run at once cut its log once, and the record names the inode of the log: a forget holds the flock
/// of the run for its whole time, so the cleanup skips a run whose forget goes on, and a second request waits for it
#[test]
fn two_forgets_of_one_run_at_once_cut_its_log_once_and_the_record_names_the_log() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let (run, _lock) = run_with_a_long_log(dir.path());
  run.write_exit(&ExitRecord::Code { code: 0 }).expect("written");
  let (reached, at_cut) = std::sync::mpsc::channel();
  let (go, held) = std::sync::mpsc::channel();
  seam::hold_after_cut(reached, held);
  let first = std::thread::spawn({
    let run = run.clone();
    move || run.forget()
  });
  at_cut.recv().expect("the first forget wrote its cut log");
  assert!(!run.try_forget().expect("the cleanup"), "the cleanup skips a run whose forget goes on");
  let second = std::thread::spawn({
    let run = run.clone();
    move || run.forget()
  });
  go.send(()).expect("the first forget goes on");
  first.join().expect("the first forget").expect("forgotten");
  second.join().expect("the second forget").expect("forgotten again");
  let Some(KeptLog::Cut(cut)) = run.kept_log().expect("the marker") else {
    panic!("a log of 60 000 events is cut");
  };
  assert_eq!(cut.ino, fs::metadata(run.events_path()).expect("the log").ino(), "the record names the log");
  assert!(!run.path().join(CUT_LOG).exists());
}

/// A run that lost its supervisor has no time of its end: the agent wrote it when it saw the loss
#[test]
fn a_lost_run_has_no_time_of_its_end() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let runs = Runs::new(dir.path().join("runs"));
  let (run, lock) = runs.create(&spec("1-1")).expect("created");
  drop(lock);
  assert!(run.end_if_lost());
  let info = run.info(&home()).expect("the info");
  assert_eq!((info.exit, info.ended_ms), (Some(REFUSED_EXIT), None));
}

/// The agent keeps the last forgotten runs by their end; a run that ended and that nobody forgot stays
#[test]
fn the_forgotten_runs_past_the_latest_by_their_end_go() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let runs = Runs::new(dir.path().join("runs"));
  let now = SystemTime::now();
  // the ids sort against the order of their ends
  for (id, ended_ago, forgotten) in [("1-1", 1, true), ("1-2", 3, true), ("1-3", 2, true), ("1-4", 9, false)] {
    let (run, _lock) = runs.create(&spec(id)).expect("created");
    run.write_exit(&ExitRecord::Code { code: 0 }).expect("written");
    if forgotten {
      run.forget().expect("forgotten");
    }
    File::options()
      .write(true)
      .open(run.path().join(EXIT))
      .and_then(|file| file.set_modified(now - Duration::from_secs(ended_ago * 60)))
      .expect("the end");
  }
  let history = History::new(&Home::new(dir.path().to_path_buf()));
  assert_eq!(runs.remove_forgotten_past(2, &history), ["1-2"]);
  let left: Vec<String> = runs.list().iter().map(run_id).collect();
  assert_eq!(left, ["1-1", "1-3", "1-4"]);
  assert_eq!(runs.remove_forgotten_past(2, &history), Vec::<String>::new());
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
  let mut file = File::open(&path).expect("the log");
  assert!(is_line_start(&mut file, first_end).expect("checked"));
  assert!(!is_line_start(&mut file, first_end - 1).expect("checked"));
  assert!(!is_line_start(&mut file, events[1].1 + 1).expect("checked"));
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
