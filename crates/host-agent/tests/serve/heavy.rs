//! The heavy turn of the host (openspec/specs/cli/host/spec.md, "serve: Heavy runs take the heavy turn of the host one at a
//! time"): the heavy runs of the host run one at a time before their lends, a run that is not heavy goes beside them,
//! a cancel and a killed holder free the queue, a wait past its time ends with `heavy_timeout`, a later run of the
//! same checkout and command supersedes the one that waits, and the heavy runs of a measure share the cores of the host

use super::{ALL_PHASES, World, events, eventually, gate_and_marks, process_alive, wait_for};
use host_link::proto::{Reply, Request, RunSpec};
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::process::Command;

/// The heavy turn of the host as `Request::Runs` answers it: the holder and the runs of the queue
fn heavy_turn(agent: &super::Agent<'_>) -> (Option<String>, Vec<String>) {
  let Reply::Runs { heavy: Some(heavy), .. } = agent.reply(&Request::Runs) else {
    panic!("no heavy turn");
  };
  (
    heavy["holder"]["run"].as_str().map(str::to_owned),
    heavy["queue"]
      .as_array()
      .map_or_else(Vec::new, |queue| queue.iter().filter_map(|entry| entry["run"].as_str().map(str::to_owned)).collect()),
  )
}

/// A heavy run of `world` whose command marks its start in `marks` and waits for `gate`
fn heavy(world: &World, run: &str, marks: &Path, gate: &Path) -> RunSpec {
  RunSpec {
    heavy: true,
    ..world.spec(run, &format!(": > '{}'; {}", marks.join(run).display(), wait_for(gate)))
  }
}

/// A time left as the line says it: `45 s` up to two minutes, then whole minutes
fn span(seconds: u64) -> String {
  if seconds < 120 { format!("{seconds} s") } else { format!("{} min", seconds / 60) }
}

/// The runs whose command started, by their marks
fn started(marks: &Path) -> Vec<String> {
  let mut runs: Vec<String> = fs::read_dir(marks).map_or_else(|_| Vec::new(), |entries| entries.filter_map(|entry| entry.ok()?.file_name().into_string().ok()).collect());
  runs.sort();
  runs
}

#[test]
fn heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them() {
  let world = World::new(3, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_CORES", "20")]);
  let (gate, marks) = gate_and_marks(&world);
  agent.start(heavy(&world, "7-1", &marks, &gate));
  eventually("the first heavy run holds the turn", || started(&marks) == ["7-1"]);
  agent.start(heavy(&world, "7-2", &marks, &gate));
  eventually("the second heavy run says its wait at once", || {
    events(&world, "7-2").contains("for the heavy turn of the host")
  });
  agent.start(RunSpec {
    heavy: false,
    ..heavy(&world, "7-3", &marks, &world.home.path().join("never"))
  });
  eventually("the run that is not heavy goes beside the heavy one", || started(&marks) == ["7-1", "7-3"]);
  assert_eq!(heavy_turn(&agent), (Some("7-1".to_owned()), vec!["7-2".to_owned()]));
  assert!(
    events(&world, "7-2").contains("brd-host: run 7-2 waits 0 s for the heavy turn of the host, all its 20 cores, as the command has no measure yet, 120 min left: run 7-1 of "),
    "{}",
    agent.dump("7-2")
  );
  assert_eq!(started(&marks), ["7-1", "7-3"], "the second heavy run waits for the first");

  fs::write(&gate, "").expect("the gate opens");
  let first = agent.follow("7-1");
  assert_eq!(first.end, Reply::Exit { code: 0 }, "{}", agent.dump("7-1"));
  eventually("the first heavy run gives the turn back after its give-back", || {
    let log = fs::read_to_string(world.run_dir("7-1").join("supervisor.log")).unwrap_or_default();
    log
      .find("gave back the slot")
      .is_some_and(|gave_back| log.find("gave the heavy turn of the host back").is_some_and(|turn| gave_back < turn))
  });
  let second = agent.follow("7-2");
  assert_eq!(second.end, Reply::Exit { code: 0 }, "{}", agent.dump("7-2"));
  let heavy_phases: Vec<&str> = std::iter::once("heavy").chain(ALL_PHASES).collect();
  assert_eq!(first.phase_names(), heavy_phases);
  assert_eq!(second.phase_names(), heavy_phases);
  if second.phase_ms("heavy") >= 1000 {
    assert!(
      second.lines.iter().any(|(_, line)| line.starts_with("brd-host: took the heavy turn of the host after ")),
      "{second:?}"
    );
  }
  fs::write(world.home.path().join("never"), "").expect("the gate of the run that is not heavy opens");
  let beside = agent.follow("7-3");
  assert_eq!(beside.end, Reply::Exit { code: 0 }, "{}", agent.dump("7-3"));
  assert_eq!(beside.phase_names(), ALL_PHASES, "a run that is not heavy takes no turn");
  eventually("the second heavy run gives the turn back", || heavy_turn(&agent) == (None, vec![]));
}

#[test]
fn a_cancel_in_the_heavy_queue_ends_the_run_before_its_lend_and_its_ticket_leaves() {
  let world = World::new(2, &[]);
  let agent = world.serve();
  let (gate, marks) = gate_and_marks(&world);
  agent.start(heavy(&world, "8-1", &marks, &gate));
  eventually("the first heavy run holds the turn", || started(&marks) == ["8-1"]);
  agent.start(heavy(&world, "8-2", &marks, &gate));
  eventually("the second heavy run waits for the turn", || heavy_turn(&agent).1 == ["8-2"]);
  assert!(matches!(agent.reply(&Request::Cancel { run: "8-2".to_owned() }), Reply::Done { .. }));
  let cancelled = agent.follow("8-2");
  assert_eq!(cancelled.end, Reply::Exit { code: 130 }, "{}", agent.dump("8-2"));
  assert_eq!(cancelled.phase_names(), ["heavy"], "no lend after the cancel");
  eventually("the cancel left the heavy queue", || heavy_turn(&agent) == (Some("8-1".to_owned()), vec![]));
  assert_eq!(started(&marks), ["8-1"], "the cancelled run never ran");
  fs::write(&gate, "").expect("the gate opens");
  assert_eq!(agent.follow("8-1").end, Reply::Exit { code: 0 });
}

#[test]
fn a_killed_heavy_run_frees_the_turn_for_the_next_one() {
  let world = World::new(2, &[]);
  let agent = world.serve();
  let (gate, marks) = gate_and_marks(&world);
  agent.start(heavy(&world, "9-1", &marks, &gate));
  eventually("the first heavy run holds the turn", || started(&marks) == ["9-1"]);
  agent.start(heavy(&world, "9-2", &marks, &world.home.path().join("second")));
  eventually("the second heavy run waits for the turn", || heavy_turn(&agent).1 == ["9-2"]);
  let pid = agent.supervisor_pid("9-1");
  assert!(Command::new("kill").args(["-9", &pid]).status().expect("kill").success());
  eventually("the end of the supervisor", || !process_alive(&pid));
  // the end of a process makes no file event: the waiting run sees it at its next look, within 5 s
  eventually("the second heavy run takes the turn of the killed one", || started(&marks) == ["9-1", "9-2"]);
  assert_eq!(heavy_turn(&agent), (Some("9-2".to_owned()), vec![]));
  fs::write(world.home.path().join("second"), "").expect("the gate of the second run opens");
  assert_eq!(agent.follow("9-2").end, Reply::Exit { code: 0 }, "{}", agent.dump("9-2"));
  fs::write(&gate, "").expect("the gate of the orphaned command opens");
}

#[test]
fn a_heavy_run_that_waits_past_its_time_ends_with_heavy_timeout() {
  let world = World::new(2, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_HEAVY_WAIT_MS", "1500"), ("BD_HOST_TEST_CORES", "20")]);
  let (gate, marks) = gate_and_marks(&world);
  agent.start(heavy(&world, "10-1", &marks, &gate));
  eventually("the first heavy run holds the turn", || started(&marks) == ["10-1"]);
  agent.start(heavy(&world, "10-2", &marks, &gate));
  let timed_out = agent.follow("10-2");
  assert!(
    matches!(&timed_out.end, Reply::Refused { code, message } if code == "heavy_timeout" && message.starts_with("no heavy turn for 1 s; giving up: run 10-2 waits ") && message.contains(" for the heavy turn of the host, all its 20 cores, as the command has no measure yet, 0 s left: run 10-1 of ")),
    "{timed_out:?}"
  );
  assert_eq!(timed_out.phase_names(), ["heavy"]);
  eventually("the run that gave up left the heavy queue", || heavy_turn(&agent) == (Some("10-1".to_owned()), vec![]));
  assert_eq!(started(&marks), ["10-1"]);
  fs::write(&gate, "").expect("the gate opens");
  assert_eq!(agent.follow("10-1").end, Reply::Exit { code: 0 });
}

#[test]
fn the_line_of_a_heavy_wait_comes_again_with_the_time_waited_and_the_time_left() {
  let world = World::new(2, &[]);
  let agent = world.serve_with(&[
    ("BD_HOST_TEST_WAIT_PROGRESS_MS", "300"),
    ("BD_HOST_TEST_HEAVY_WAIT_MS", "600000"),
    ("BD_HOST_TEST_CORES", "20"),
  ]);
  let (gate, marks) = gate_and_marks(&world);
  agent.start(heavy(&world, "11-1", &marks, &gate));
  eventually("the first heavy run holds the turn", || started(&marks) == ["11-1"]);
  agent.start(heavy(&world, "11-2", &marks, &gate));
  // the line at once, then one each 300 ms after a look of up to 2 s, the time left in whole minutes of what is left
  // of the 600 s
  let lines = || -> Vec<String> {
    events(&world, "11-2")
      .lines()
      .filter(|line| line.contains("for the heavy turn of the host"))
      .map(str::to_owned)
      .collect()
  };
  eventually("a second line of the wait", || lines().len() >= 2);
  let lines = lines();
  assert!(
    lines[0].contains("run 11-2 waits 0 s for the heavy turn of the host, all its 20 cores, as the command has no measure yet, 10 min left: run 11-1 of "),
    "{lines:?}"
  );
  let waited: u64 = lines[1]
    .split("waits ")
    .nth(1)
    .and_then(|rest| rest.split(' ').next())
    .and_then(|n| n.parse().ok())
    .expect("the time waited");
  // the line says the whole seconds waited, and the time left is of the exact wait: one of two whole minutes
  let left = |seconds: u64| {
    format!(
      "waits {waited} s for the heavy turn of the host, all its 20 cores, as the command has no measure yet, {} left: run 11-1 of ",
      span(seconds)
    )
  };
  assert!(
    waited >= 1 && (lines[1].contains(&left(600 - waited)) || lines[1].contains(&left(599 - waited))),
    "{lines:?}"
  );
  fs::write(&gate, "").expect("the gate opens");
  assert_eq!(agent.follow("11-1").end, Reply::Exit { code: 0 });
  assert_eq!(agent.follow("11-2").end, Reply::Exit { code: 0 }, "{}", agent.dump("11-2"));
}

#[test]
fn a_later_heavy_run_of_the_same_checkout_and_command_supersedes_the_waiting_one() {
  let world = World::new(2, &[]);
  let agent = world.serve();
  let (gate, marks) = gate_and_marks(&world);
  agent.start(heavy(&world, "12-1", &marks, &gate));
  eventually("the first heavy run holds the turn", || started(&marks) == ["12-1"]);
  // one command for the runs of the checkout: it marks `same`
  let same = |run: &str| RunSpec {
    heavy: true,
    ..world.spec(run, &format!(": > '{}'; {}", marks.join("same").display(), wait_for(&gate)))
  };
  agent.start(same("12-2"));
  eventually("the second heavy run waits for the turn", || heavy_turn(&agent).1 == ["12-2"]);
  agent.start(same("12-3"));
  let superseded = agent.follow("12-2");
  assert_eq!(
    superseded.end,
    Reply::Refused {
      code: "superseded".to_owned(),
      message: "run 12-2 was superseded by run 12-3 of the same checkout and command before it started: nothing ran; do not run it again, here or on the client; follow 12-3: `brd-host follow <host> 12-3`".to_owned(),
    },
    "{}",
    agent.dump("12-2")
  );
  assert_eq!(superseded.phase_names(), ["heavy"], "no lend, no command");
  assert!(events(&world, "12-3").contains("brd-host: replaces run 12-2, which waited "), "{}", agent.dump("12-3"));
  // the same command of another checkout is another run
  agent.start(RunSpec {
    affinity: "checkout-ba9876543210".to_owned(),
    ..same("12-4")
  });
  eventually("the later run and the run of another checkout wait", || {
    heavy_turn(&agent) == (Some("12-1".to_owned()), vec!["12-3".to_owned(), "12-4".to_owned()])
  });
  assert!(!events(&world, "12-4").contains("replaces run"), "{}", agent.dump("12-4"));
  let Reply::Runs { runs, .. } = agent.reply(&Request::Runs) else {
    panic!("no runs");
  };
  let refused = runs
    .iter()
    .find(|run| run.run == "12-2")
    .and_then(|run| run.refused.as_ref())
    .map(|refused| refused.code.as_str());
  assert_eq!(refused, Some("superseded"), "`brd-host runs` says how it ended");

  fs::write(&gate, "").expect("the gate opens");
  assert_eq!(agent.follow("12-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("12-1"));
  assert_eq!(agent.follow("12-3").end, Reply::Exit { code: 0 }, "{}", agent.dump("12-3"));
  assert_eq!(agent.follow("12-4").end, Reply::Exit { code: 0 }, "{}", agent.dump("12-4"));
  assert_eq!(started(&marks), ["12-1", "same"]);
}

/// The holders of the heavy turn as `Request::Runs` answers it
fn turn_holders(agent: &super::Agent<'_>) -> Vec<String> {
  let Reply::Runs { heavy: Some(heavy), .. } = agent.reply(&Request::Runs) else {
    panic!("no heavy turn");
  };
  heavy["holders"]
    .as_array()
    .map_or_else(Vec::new, |holders| holders.iter().filter_map(|holder| holder["run"].as_str().map(str::to_owned)).collect())
}

#[test]
fn heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host() {
  let world = World::new(3, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_CORES", "20"), ("BD_HOST_TEST_SAMPLE_MS", "100"), ("BD_HOST_TEST_MIN_WALL_MS", "0")]);
  let (gate, marks) = gate_and_marks(&world);
  // one command for the runs that share: each process marks its own pid, then waits for the gate
  let script = format!(": > '{}/shared-'$$; {}", marks.display(), wait_for(&gate));
  let shared = |run: &str| RunSpec {
    heavy: true,
    ..world.spec(run, &script)
  };
  fs::write(&gate, "").expect("the gate opens");
  agent.start(shared("13-1"));
  assert_eq!(agent.follow("13-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("13-1"));
  assert!(events(&world, "13-1").contains("brd-host: the command used "), "{}", agent.dump("13-1"));
  let Reply::Runs { runs, .. } = agent.reply(&Request::Runs) else {
    panic!("no runs");
  };
  assert!(runs.iter().find(|run| run.run == "13-1").is_some_and(|run| run.cores.is_some()), "{runs:?}");
  fs::remove_file(&gate).expect("the gate closes");
  fs::remove_file(marks.join(fs::read_dir(&marks).unwrap().next().unwrap().unwrap().file_name())).expect("the mark of the first run");
  // the first run, without a measure, holds the whole host until its give-back, after its end
  eventually("the first run left the turn", || turn_holders(&agent).is_empty());

  // the command measured a fraction of a core: two of its runs hold cores at once; the second of another checkout, so
  // it supersedes nothing
  agent.start(shared("13-2"));
  agent.start(RunSpec {
    affinity: "checkout-ba9876543210".to_owned(),
    ..shared("13-3")
  });
  eventually("two heavy runs of a measure run at once", || fs::read_dir(&marks).map_or(0, Iterator::count) == 2);
  let mut holders = turn_holders(&agent);
  holders.sort();
  assert_eq!(holders, ["13-2", "13-3"]);
  // a command without a measure waits for the whole host
  agent.start(heavy(&world, "13-4", &marks, &gate));
  eventually("the run without a measure waits", || heavy_turn(&agent).1 == ["13-4"]);
  assert!(
    events(&world, "13-4").contains("brd-host: run 13-4 waits 0 s for the heavy turn of the host, all its 20 cores, as the command has no measure yet, "),
    "{}",
    agent.dump("13-4")
  );
  fs::write(&gate, "").expect("the gate opens");
  for run in ["13-2", "13-3", "13-4"] {
    assert_eq!(agent.follow(run).end, Reply::Exit { code: 0 }, "{}", agent.dump(run));
  }
  assert!(started(&marks).contains(&"13-4".to_owned()));
}

/// `serve` samples the busy cores of the whole host into `host.json` of the heavy turn every 2 s, for the admissions:
/// the samples of the runs miss what no run counts
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn serve_samples_the_busy_cores_of_the_host_for_the_heavy_turn() {
  let world = World::new(1, &[]);
  let _agent = world.serve_with(&[("BD_HOST_TEST_HOST_SAMPLE_MS", "200")]);
  let file = world.home.path().join(".cache/brd-host/heavy/host.json");
  eventually("a sample of the host", || {
    fs::read_to_string(&file).is_ok_and(|text| serde_json::from_str::<serde_json::Value>(&text).is_ok_and(|sample| sample["busy"].is_number() && sample["at_ms"].is_u64()))
  });
}

/// Starts the exclusive run `run` of `world`, whose command marks its start in `marks` and waits for `gate`, and waits
/// until it holds the host
fn hold_the_host(agent: &super::Agent<'_>, world: &World, run: &str, marks: &Path, gate: &Path) {
  agent.start(RunSpec {
    exclusive: true,
    ..heavy(world, run, marks, gate)
  });
  eventually("the exclusive run holds the host", || started(marks) == [run]);
}

/// A run that is not heavy waits before its lend while an exclusive run holds the host, says what it waits for, starts
/// once the exclusive run ends, and books the wait as its step `exclusive`
#[test]
fn a_run_that_is_not_heavy_waits_for_an_exclusive_run_and_starts_after_it() {
  let world = World::new(2, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_CORES", "20")]);
  let (gate, marks) = gate_and_marks(&world);
  hold_the_host(&agent, &world, "14-1", &marks, &gate);
  let open = world.home.path().join("open");
  agent.start(RunSpec {
    heavy: false,
    ..heavy(&world, "14-2", &marks, &open)
  });
  eventually("the run that is not heavy says its wait", || {
    events(&world, "14-2").contains("brd-host: run 14-2 waits 0 s for a measurement: run 14-1 of ")
  });
  assert_eq!(started(&marks), ["14-1"], "the run that is not heavy waits for the measurement");
  fs::write(&gate, "").expect("the gate opens");
  assert_eq!(agent.follow("14-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("14-1"));
  fs::write(&open, "").expect("the gate of the run that is not heavy opens");
  let after = agent.follow("14-2");
  assert_eq!(after.end, Reply::Exit { code: 0 }, "{}", agent.dump("14-2"));
  let phases: Vec<&str> = std::iter::once("exclusive").chain(ALL_PHASES).collect();
  assert_eq!(after.phase_names(), phases);
}

/// A run of another repository that is not heavy waits for an exclusive run too: the measurement holds the whole host,
/// whatever the repository of a run
#[test]
fn a_run_of_another_repository_waits_for_an_exclusive_run() {
  let world = World::new(1, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_CORES", "20")]);
  let (gate, marks) = gate_and_marks(&world);
  hold_the_host(&agent, &world, "15-1", &marks, &gate);
  let other = test_utils::git_test_utils::TestRepo::new();
  other.create_commit_with_files("first", &[(".gitignore", "out/\n")]);
  test_utils::git_test_utils::append_config(other.path(), &[("branchdeck.hostSlots", "1")]);
  let open = world.home.path().join("open");
  agent.start(RunSpec {
    heavy: false,
    repo: other.path_str().to_owned(),
    commit: other.head(),
    ..heavy(&world, "15-2", &marks, &open)
  });
  eventually("the run of the other repository says its wait", || {
    events(&world, "15-2").contains("brd-host: run 15-2 waits 0 s for a measurement: run 15-1 of ")
  });
  assert_eq!(started(&marks), ["15-1"], "the run of the other repository waits for the measurement");
  fs::write(&gate, "").expect("the gate opens");
  assert_eq!(agent.follow("15-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("15-1"));
  fs::write(&open, "").expect("the gate of the other run opens");
  let after = agent.follow("15-2");
  assert_eq!(after.end, Reply::Exit { code: 0 }, "{}", agent.dump("15-2"));
  assert_eq!(after.phase_names().first(), Some(&"exclusive"));
}
