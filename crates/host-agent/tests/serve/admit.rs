//! The start of a member of a set of hosts that takes runs only at once (openspec/specs/cli/host/spec.md, "serve: A start
//! that admits at once or refuses with busy"): its supervisor takes the heavy turn in one look under the lock of the
//! table, or the start answers `busy` and nothing ran, so two clients that race for one free host never both get it

use super::{World, eventually, gate_and_marks, wait_for};
use host_link::proto::{Reply, Request, RunSpec};
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;

/// A heavy run of `admit_now` of `world` whose command marks its start in `marks` and waits for `gate`
fn at_once(world: &World, run: &str, marks: &Path, gate: &Path) -> RunSpec {
  RunSpec {
    heavy: true,
    admit_now: true,
    ..world.spec(run, &format!(": > '{}'; {}", marks.join(run).display(), wait_for(gate)))
  }
}

#[test]
fn of_two_starts_that_admit_at_once_on_a_free_host_one_runs_and_the_other_is_busy() {
  let world = World::new(2, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_CORES", "20")]);
  let (gate, marks) = gate_and_marks(&world);
  // two clients at once, each a run of the whole host, as neither command has a measure
  let starts = [at_once(&world, "21-1", &marks, &gate), at_once(&world, "21-2", &marks, &gate)];
  // both threads start before either joins: an array maps at once
  let replies: Vec<(String, Reply)> = std::thread::scope(|scope| {
    let asked = starts.map(|spec| scope.spawn(|| (spec.run.clone(), agent.reply(&Request::Start(spec)))));
    Vec::from(asked.map(|asked| asked.join().expect("a start")))
  });
  let admitted: Vec<&str> = replies
    .iter()
    .filter(|(_, reply)| matches!(reply, Reply::Started { .. }))
    .map(|(run, _)| run.as_str())
    .collect();
  let busy: Vec<(&str, &str)> = replies
    .iter()
    .filter_map(|(run, reply)| match reply {
      Reply::Refused { code, message } if code == "busy" => Some((run.as_str(), message.as_str())),
      _ => None,
    })
    .collect();
  assert_eq!((admitted.len(), busy.len()), (1, 1), "{replies:?}");
  let (refused, message) = busy[0];
  assert!(
    message.starts_with("<host> takes runs only at once (`idle_only` of the set), and its heavy turn has no room for it now: "),
    "{message}"
  );
  let ended = agent.follow(refused);
  assert!(matches!(&ended.end, Reply::Refused { code, .. } if code == "busy"), "{}", agent.dump(refused));
  assert_eq!(ended.phase_names(), ["heavy"], "the refused run got no slot");

  eventually("the admitted run runs its command", || marks.join(admitted[0]).exists());
  assert!(!marks.join(refused).exists(), "the refused run ran nothing");
  fs::write(&gate, "").expect("the gate opens");
  assert_eq!(agent.follow(admitted[0]).end, Reply::Exit { code: 0 }, "{}", agent.dump(admitted[0]));

  // the turn is free again after the give-back of the admitted run: the next start that admits at once runs
  eventually(
    "the admitted run gives the turn back",
    || matches!(agent.reply(&Request::Runs { run: None }), Reply::Runs { heavy: Some(heavy), .. } if heavy["holders"].as_array().is_some_and(Vec::is_empty)),
  );
  let free = Path::new("/");
  agent.start(at_once(&world, "21-3", &marks, free));
  assert_eq!(agent.follow("21-3").end, Reply::Exit { code: 0 }, "{}", agent.dump("21-3"));
}
