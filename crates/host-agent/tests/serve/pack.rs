//! The wait of a run for its pack: the deadline, the stall time without a chunk, and a pack that comes after the end of
//! its run

use super::{World, client_commit, events, eventually, process_alive, send_pack};
use host_link::proto::{PackTarget, Reply, Request, RunSpec, Stream};
use pretty_assertions::assert_eq;
use std::thread;
use std::time::{Duration, Instant};

/// A commit that no repository has
fn missing_commit() -> String {
  "1".repeat(40)
}

#[test]
fn a_start_whose_pack_never_comes_ends_with_pack_missing() {
  let world = World::new(1, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_PACK_WAIT_MS", "300")]);
  let ran = world.home.path().join("ran");
  agent.start(RunSpec {
    commit: missing_commit(),
    ..world.spec("1-1", &format!(": > '{}'", ran.display()))
  });
  let followed = agent.follow("1-1");
  assert!(
    matches!(&followed.end, Reply::Refused { code, message } if code == "pack_missing" && message.starts_with("the pack of the run 1-1 did not come in 0.3 s, and ")),
    "{followed:?}; {}",
    agent.dump("1-1")
  );
  assert_eq!(followed.phase_names(), ["slot", "pack_wait"]);
  assert!(
    !followed.lines.iter().any(|(_, line)| line.contains("waiting for the pack")),
    "a wait under a second says nothing: {followed:?}"
  );
  assert!(!ran.exists(), "the command never ran");
  match agent.reply(&Request::Runs) {
    Reply::Runs { runs } => {
      assert_eq!(runs.len(), 1, "{runs:?}");
      assert_eq!(runs[0].exit, Some(255), "a refused run exits with 255");
      assert_eq!(runs[0].phases, followed.phases);
    }
    other => panic!("{other:?}"),
  }
}

/// A client that builds its pack sends empty chunks, and they keep the wait of its run alive past the stall time. A
/// start whose pack connection never comes ends with `pack_missing` at the stall time, long before the deadline
#[test]
fn the_chunks_of_a_pack_keep_its_run_waiting_and_a_start_without_them_ends_at_the_stall() {
  // the agent notes a chunk at most once a second, so the stall time is a few times that
  let stall = Duration::from_secs(3);
  let world = World::new(1, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_PACK_STALL_MS", "3000")]);
  let (_client, commit, pack) = client_commit(&world, "b.txt", "from the client\n");
  agent.start(RunSpec {
    commit: commit.clone(),
    ..world.spec("2-1", "cat b.txt")
  });
  let mut channel = agent.ask(&Request::Pack {
    repo: world.repo.path_str().to_owned(),
    commit,
    target: PackTarget::Run("2-1".to_owned()),
  });
  let building = Instant::now();
  while building.elapsed() < stall + Duration::from_millis(1000) {
    channel.send_keepalive().expect("an empty chunk");
    thread::sleep(Duration::from_millis(100));
  }
  assert!(events(&world, "2-1").contains("waiting for the pack"), "the run still waits: {}", agent.dump("2-1"));
  channel.send_stream(&mut &pack[..], &mut |_| {}).expect("the pack");
  let packed = loop {
    match channel.recv::<Reply>().expect("the reply") {
      Reply::Heartbeat { .. } => {}
      reply => break reply,
    }
  };
  assert_eq!(packed, Reply::Packed);
  let followed = agent.follow("2-1");
  assert_eq!(followed.end, Reply::Exit { code: 0 }, "{}", agent.dump("2-1"));
  assert_eq!(followed.of(Stream::Stdout), ["from the client"]);
  let waited = followed.phases.iter().find(|phase| phase.name == "pack_wait").map_or(0, |phase| phase.ms);
  assert!(u128::from(waited) >= stall.as_millis(), "the wait outlived the stall time: {followed:?}");

  let started = Instant::now();
  agent.start(RunSpec {
    commit: missing_commit(),
    ..world.spec("2-2", "true")
  });
  let stalled = agent.follow("2-2");
  assert!(
    matches!(&stalled.end, Reply::Refused { code, message } if code == "pack_missing" && message.contains("sent no chunk for 3 s, so its client is gone")),
    "{stalled:?}"
  );
  assert!(started.elapsed() < Duration::from_secs(20), "{:?}", started.elapsed());
}

/// A pack that comes after the end of its run lands its objects and its ref, and marks no run: the keeper deletes a
/// run ref that no lend holds
#[test]
fn a_pack_after_the_end_of_its_run_lands_and_marks_no_run() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  agent.start(world.spec("3-1", "true"));
  assert_eq!(agent.follow("3-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("3-1"));
  // the give-back of the supervisor follows `exit.json` and deletes the ref of the run, so the pack comes after it
  let supervisor = agent.supervisor_pid("3-1");
  eventually("the end of the supervisor", || !process_alive(&supervisor));
  let (_client, commit, pack) = client_commit(&world, "b.txt", "late\n");
  assert_eq!(send_pack(&agent, &commit, PackTarget::Run("3-1".to_owned()), &pack), Reply::Packed);
  assert_eq!(world.repo.git(&["cat-file", "-t", &commit]).trim(), "commit", "the objects land");
  assert_eq!(world.repo.git(&["rev-parse", "refs/branch-deck/host/run/3-1"]).trim(), commit, "the ref lands");
  assert!(!world.run_dir("3-1").join("packed").exists(), "the ended run gets no mark");
}
