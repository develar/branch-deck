//! The signals of a run: a cancel sends SIGINT to the command group and SIGKILL after the grace time, the processes
//! that a command leaves in its group get SIGTERM and SIGKILL, and a run whose supervisor is killed ends for a cancel and
//! a follow

use super::{World, eventually, process_alive, wait_for};
use host_link::proto::{Reply, Request};
use pretty_assertions::assert_eq;
use std::fs;
use std::process::Command;

/// A shell loop of up to 60 s that a test interrupts, so a failed test leaves no command behind
const LOOP: &str = "i=0; while [ $i -lt 600 ]; do sleep 0.1; i=$((i + 1)); done";

#[test]
fn a_cancel_interrupts_the_command_group() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  // a trap makes the end certain: without one, sh goes on when its child exits 0 at the moment of the signal
  agent.start(world.spec("1-1", &format!("trap 'echo interrupted; exit 7' INT; echo ready; {LOOP}")));
  agent.follow_until("1-1", "ready");
  assert_eq!(
    agent.reply(&Request::Cancel { run: "1-1".to_owned() }),
    Reply::Done {
      message: "the supervisor of the run 1-1 has the cancel request".to_owned()
    },
    "{}",
    agent.dump("1-1")
  );
  let followed = agent.follow("1-1");
  assert_eq!(followed.end, Reply::Exit { code: 7 }, "{}", agent.dump("1-1"));
  assert!(followed.has_line("interrupted"), "{followed:?}");
  assert_eq!(
    agent.reply(&Request::Cancel { run: "1-1".to_owned() }),
    Reply::Done {
      message: "the run 1-1 has ended already".to_owned()
    }
  );
}

#[test]
fn a_command_that_ignores_sigint_gets_sigkill_after_the_grace_time() {
  let world = World::new(1, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_GRACE_MS", "500")]);
  agent.start(world.spec("2-1", &format!("trap '' INT; echo ready; {LOOP}")));
  agent.follow_until("2-1", "ready");
  assert!(matches!(agent.reply(&Request::Cancel { run: "2-1".to_owned() }), Reply::Done { .. }));
  let followed = agent.follow("2-1");
  assert_eq!(followed.end, Reply::Exit { code: 128 + 9 }, "{}", agent.dump("2-1"));
  assert!(followed.has_line("bd-host: the command did not end 500 ms after SIGINT; SIGKILL"), "{followed:?}");
}

#[test]
fn a_process_that_the_command_leaves_in_its_group_gets_sigterm_and_sigkill_before_the_exit() {
  let world = World::new(1, &[]);
  // the process ignores SIGTERM, so it outlives both windows whatever their length
  let agent = world.serve_with(&[("BD_HOST_TEST_GROUP_SETTLE_MS", "50"), ("BD_HOST_TEST_GROUP_TERM_MS", "100")]);
  let pid_file = world.home.path().join("left.pid");
  // the command ends only once the process ignores SIGTERM, so a short settle never signals it before its trap
  let trapped = world.home.path().join("trapped");
  agent.start(world.spec(
    "3-1",
    &format!(
      "(trap '' TERM; : > '{}'; {LOOP}) & echo $! > '{}'; {}; echo done",
      trapped.display(),
      pid_file.display(),
      wait_for(&trapped)
    ),
  ));
  let followed = agent.follow("3-1");
  assert_eq!(followed.end, Reply::Exit { code: 0 }, "{}", agent.dump("3-1"));
  assert!(followed.has_line("bd-host: the command left processes in its group; SIGTERM"), "{followed:?}");
  assert!(followed.has_line("bd-host: the processes of the group did not end after SIGTERM; SIGKILL"), "{followed:?}");
  // the system reaps the killed process a moment later; its parent, the command, has ended
  let left = fs::read_to_string(&pid_file).expect("the pid");
  eventually("the end of the process of the group", || !process_alive(left.trim()));
}

#[test]
fn a_killed_supervisor_ends_the_run_for_cancel_and_follow() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  let gate = world.home.path().join("gate");
  agent.start(world.spec("4-1", &format!("echo holding; {}", wait_for(&gate))));
  agent.follow_until("4-1", "holding");
  let pid = agent.supervisor_pid("4-1");
  assert!(Command::new("kill").args(["-9", &pid]).status().expect("kill").success());
  // `serve` reaps its supervisor, and the flock of the run goes with the last descriptor of the killed process
  eventually("the end of the supervisor", || !process_alive(&pid));
  let lock = world.run_dir("4-1").join("lock");
  eventually("the release of the run lock", || fs::File::open(&lock).is_ok_and(|file| file.try_lock_shared().is_ok()));
  // the follow of `follow_until` can see the lost run first, while its connection ends
  let done = agent.reply(&Request::Cancel { run: "4-1".to_owned() });
  assert!(
    matches!(&done, Reply::Done { message } if message == "the run 4-1 had lost its supervisor; it has ended" || message == "the run 4-1 has ended already"),
    "{done:?}; {}",
    agent.dump("4-1")
  );
  let followed = agent.follow("4-1");
  assert!(
    matches!(&followed.end, Reply::Refused { code, message } if code == "agent_error" && message == "the supervisor of the run ended without an exit code"),
    "{followed:?}"
  );
  fs::write(&gate, "").expect("the gate opens");
}
