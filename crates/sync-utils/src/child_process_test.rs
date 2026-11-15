use crate::child_process::{
  ChildPriority, ChildProcessError, ToolDir, ToolLookup, find_executable, find_tool_in, run_child, run_child_lines, run_child_until_superseded, windowless_command,
  windowless_command_at,
};
#[cfg(unix)]
use crate::process_scan::is_running;
use crate::process_scan::{is_running as process_is_running, process_started};
use std::process::{Child, Command, Stdio};
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use test_utils::fake_executable::{fake_executable, shell_path};

const TIMEOUT: Duration = Duration::from_secs(10);

/// A child that copies its stdin to stdout, or one that hangs: `cat` and `sleep` of the fake's sh, which Windows has
/// only in Git's `usr\bin`, not always on `PATH`
fn fake(dir: &tempfile::TempDir, body: &str) -> std::path::PathBuf {
  fake_executable(dir.path(), "child", body)
}

#[test]
fn writes_stdin_while_reading_stdout() {
  // more than a pipe buffer in both directions: writing all input before reading would deadlock
  let input = "0123456789abcdef\n".repeat(64 * 1024).into_bytes();
  let dir = tempfile::tempdir().unwrap();
  let output = run_child(&mut windowless_command(fake(&dir, "exec cat")), Some(input.clone()), TIMEOUT, &|| false).unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, input);
}

#[test]
fn reports_a_missing_binary() {
  let dir = tempfile::tempdir().unwrap();
  let result = run_child(&mut windowless_command(dir.path().join("missing")), None, TIMEOUT, &|| false);
  assert!(matches!(result, Err(ChildProcessError::NotFound)), "{result:?}");
}

/// A child that ends by itself only after two minutes: an end within a minute, which no run of a kill reaches, is the
/// kill
const HANG: &str = "exec sleep 120";

#[test]
fn kills_the_child_after_the_timeout() {
  let dir = tempfile::tempdir().unwrap();
  let mut hang = windowless_command(fake(&dir, HANG));
  let started = Instant::now();
  let result = run_child(&mut hang, None, Duration::from_millis(300), &|| false);
  assert!(matches!(result, Err(ChildProcessError::TimedOut)), "{result:?}");
  assert!(started.elapsed() < Duration::from_mins(1), "{:?}", started.elapsed());
}

/// What the last kill of this thread found, for what a test says of a process that outlived it: the rounds of its tree
/// on Unix, nothing on Windows, whose job takes every process at once
#[cfg(unix)]
type KillTrace = Option<crate::process_tree::KillReport>;
/// Nothing, as a value of its own: a `()` is passed by value and bound by no `let` (clippy)
#[cfg(not(unix))]
#[derive(Debug)]
struct KillTrace;

fn last_kill() -> KillTrace {
  #[cfg(unix)]
  return crate::process_tree::seam::last_kill();
  #[cfg(not(unix))]
  KillTrace
}

/// Waits until each of `processes` has exited, as long as it takes: the slow timeout of nextest ends the test of one that
/// outlives the kill. Meanwhile, once in a while, it says what such a process is: its parent now (1 when the system
/// took it as an orphan), whether it is stopped, and whether the kill found it (`kill`)
fn wait_for_the_exits(processes: &[(u32, Option<u64>)], what: &str, kill: &KillTrace) {
  for &(pid, started) in processes {
    let mut polls: u32 = 0;
    while process_is_running(pid, started) {
      polls += 1;
      if polls % 2000 == 0 {
        #[cfg(unix)]
        let (halted, found) = (
          crate::process_scan::halted(pid),
          kill
            .as_ref()
            .map(|kill| (kill.settled(), kill.rounds.iter().position(|round| round.iter().any(|process| process.pid == pid)))),
        );
        #[cfg(not(unix))]
        let (halted, found) = ((), kill);
        eprintln!(
          "{what} {pid} runs on after the kill: its parent is {:?}, halted {halted:?}, (settled, the round that found it) {found:?}",
          crate::process_scan::parent_of(pid)
        );
      }
      std::thread::sleep(Duration::from_millis(5));
    }
  }
}

/// The processes of the tree of `child_with_a_tree`, each a file of its directory that names it
const TREE: [&str; 3] = ["grandchild", "great-grandchild", "sleep"];

/// The processes of `TREE` that are known, with their starts
type Tree = std::collections::BTreeMap<&'static str, (u32, Option<u64>)>;

/// The child of `fake` with a tree below it: a subshell in the background (the grandchild) that starts a `sleep` of its
/// own (the great-grandchild) and waits for it, and a `sleep` of the child in the background; each writes its pid, as
/// the system names it, into its file of `TREE`, and the child waits
fn child_with_a_tree(dir: &tempfile::TempDir) -> std::path::PathBuf {
  // on Windows the sh of Git keeps a pid of its own
  let pid_of_last = if cfg!(windows) { "cat /proc/$!/winpid" } else { "echo $!" };
  let file = |name: &str| shell_path(&dir.path().join(name));
  fake(
    dir,
    &format!(
      "(sleep 120 &\n{pid_of_last} > '{}'\nwait) &\n{pid_of_last} > '{}'\nsleep 120 &\n{pid_of_last} > '{}'\nwait",
      file("great-grandchild"),
      file("grandchild"),
      file("sleep")
    ),
  )
}

/// Runs the child of `child_with_a_tree` until each process of its tree is known (into `tree`, with its start), then
/// kills it: what the kill found
fn kill_once_the_tree_runs(dir: &tempfile::TempDir, tree: &std::cell::RefCell<Tree>) -> KillTrace {
  let child = child_with_a_tree(dir);
  let known = || {
    let mut tree = tree.borrow_mut();
    for name in TREE {
      if let (false, Ok(text)) = (tree.contains_key(name), std::fs::read_to_string(dir.path().join(name)))
        && let Ok(pid) = text.trim().parse::<u32>()
      {
        tree.insert(name, (pid, process_started(pid)));
      }
    }
    tree.len() == TREE.len()
  };
  let result = run_child_until_superseded(&mut windowless_command(child), None, &known);
  assert!(matches!(result, Err(ChildProcessError::Superseded)), "{result:?}");
  last_kill()
}

/// Waits for the exit of every process of `tree`, each started when it was known
fn wait_for_the_tree(tree: &Tree, kill: &KillTrace) {
  for (name, &(pid, started)) in tree {
    assert!(started.is_some(), "the {name} runs when it is known");
    wait_for_the_exits(&[(pid, started)], name, kill);
  }
}

/// A kill ends the processes the child started too: each process of its tree, which would sleep two minutes, exits.
/// The child is killed once its tree is known, with the starts; no deadline but nextest's
#[test]
fn a_killed_child_leaves_no_process_it_started() {
  let dir = tempfile::tempdir().unwrap();
  let tree = std::cell::RefCell::new(Tree::new());
  let kill = kill_once_the_tree_runs(&dir, &tree);
  wait_for_the_tree(&tree.into_inner(), &kill);
}

/// The start of a tree that goes on only once the whole tree of the child of `child_with_a_tree` in `dir` runs, as a
/// loaded machine may let the child run before the step after its spawn
fn wait_after_spawns_for_the_tree(dir: &tempfile::TempDir) {
  let files: Vec<std::path::PathBuf> = TREE.iter().map(|name| dir.path().join(name)).collect();
  crate::process_tree::seam::wait_after_spawns(move || {
    while !files.iter().all(|file| std::fs::read_to_string(file).is_ok_and(|text| text.trim().parse::<u32>().is_ok())) {
      std::thread::sleep(Duration::from_millis(5));
    }
  });
}

/// A process that the child starts at once, before the step after its spawn (`wait_after_spawns_for_the_tree`), ends
/// with the kill: on Windows it is in the job the child was created in. No deadline but nextest's
#[test]
fn a_process_the_child_starts_at_once_ends_with_the_kill() {
  let dir = tempfile::tempdir().unwrap();
  wait_after_spawns_for_the_tree(&dir);
  let tree = std::cell::RefCell::new(Tree::new());
  let kill = kill_once_the_tree_runs(&dir, &tree);
  wait_for_the_tree(&tree.into_inner(), &kill);
}

/// On Windows the child is created suspended and resumed only once it is in its job: each process of its tree, all of
/// them started before the step after the spawn (`wait_after_spawns_for_the_tree`), is in the job; a job joined after
/// the child ran would hold none of them
#[cfg(windows)]
#[test]
fn a_process_the_child_starts_at_once_is_in_its_job() {
  let dir = tempfile::tempdir().unwrap();
  wait_after_spawns_for_the_tree(&dir);
  let mut command = windowless_command(child_with_a_tree(&dir));
  command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
  let (mut child, tree) = crate::process_tree::ProcessTree::start(&mut command).unwrap();
  let held: Vec<(&str, bool)> = TREE
    .iter()
    .map(|name| {
      let pid: u32 = std::fs::read_to_string(dir.path().join(name)).unwrap().trim().parse().unwrap();
      (*name, tree.holds(pid))
    })
    .collect();
  tree.kill(&mut child);
  let _ = child.wait();
  assert!(held.iter().all(|(_, held)| *held), "{held:?}");
}

/// Where Windows makes no job for a child (the seam of the tests), the child runs without one and its kill ends the
/// child alone: the start does not fail
#[cfg(windows)]
#[test]
fn a_child_that_gets_no_job_runs_without_one() {
  crate::process_tree::seam::make_no_jobs();
  let dir = tempfile::tempdir().unwrap();
  let mut command = windowless_command(fake(&dir, "echo ran"));
  let output = run_child(&mut command, None, TIMEOUT, &|| false).unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ran");
  let mut hang = windowless_command(fake(&dir, HANG));
  let result = run_child(&mut hang, None, Duration::from_millis(300), &|| false);
  assert!(matches!(result, Err(ChildProcessError::TimedOut)), "{result:?}");
}

/// The kill of the tree of `child_with_a_tree` whose scans list what `change` makes of their lists, by the round and the
/// tree known so far: each process exits, the kill settles, and a round after the first found `found_later`
#[cfg(unix)]
fn kill_with_scans_that_miss(change: impl Fn(usize, &Tree, &mut Option<Vec<crate::process_scan::Descendant>>) + 'static, found_later: &str) {
  let dir = tempfile::tempdir().unwrap();
  let tree = std::rc::Rc::new(std::cell::RefCell::new(Tree::new()));
  let known = std::rc::Rc::clone(&tree);
  crate::process_tree::seam::change_scans(move |round, listed| change(round, &known.borrow(), listed));
  let kill = kill_once_the_tree_runs(&dir, &tree).expect("the kill of the tree");
  let tree = tree.borrow().clone();
  let (pid, _) = tree[found_later];
  let round = kill.rounds.iter().position(|round| round.iter().any(|process| process.pid == pid));
  assert!(round.is_some_and(|round| round > 0), "a later round found the {found_later}: {kill:?}");
  assert!(kill.settled(), "{kill:?}");
  wait_for_the_tree(&tree, &Some(kill));
}

/// The scans of `round` 0 without `name` of `tree` (with its children too when `below`)
#[cfg(unix)]
fn without_in_round_0(round: usize, tree: &Tree, listed: &mut Option<Vec<crate::process_scan::Descendant>>, name: &str, below: bool) {
  if let (0, Some(&(pid, _)), Some(listed)) = (round, tree.get(name), listed.as_mut()) {
    listed.retain(|process| process.pid != pid && !(below && process.parent == pid));
  }
}

/// A process that the scans of the first round miss (the seam of the tests hides the grandchild and its sleep) is
/// found by a later round: no process is killed before a round finds nothing new, and the grandchild is below the
/// stopped child still. Without the rounds the grandchild would outlive the kill, never stopped
#[cfg(unix)]
#[test]
fn a_process_the_first_round_misses_is_killed_after_a_later_round_finds_it() {
  kill_with_scans_that_miss(|round, tree, listed| without_in_round_0(round, tree, listed, "grandchild", true), "grandchild");
}

/// The miss of the first round at a depth of two (the great-grandchild alone hidden: a hook's sh and its node, a
/// fetch's `git-remote-https`): its parent, the grandchild, was found, and is killed only after a later round found the
/// great-grandchild below it, so neither goes to launchd alive
#[cfg(unix)]
#[test]
fn a_great_grandchild_the_first_round_misses_outlives_none_of_its_parents() {
  kill_with_scans_that_miss(|round, tree, listed| without_in_round_0(round, tree, listed, "great-grandchild", false), "great-grandchild");
}

/// A first round whose scans find nothing at all confirms nothing: the kill goes on until a later round finds the tree,
/// and one after it nothing new
#[cfg(unix)]
#[test]
fn a_first_round_that_finds_nothing_confirms_nothing() {
  kill_with_scans_that_miss(
    |round, _, listed| {
      if round == 0 {
        *listed = Some(Vec::new());
      }
    },
    "grandchild",
  );
}

/// A round that finds something new confirms nothing either: the first round misses the grandchild and its sleep, the
/// second finds the grandchild and misses its sleep, and only a third finds it, below the grandchild still stopped
#[cfg(unix)]
#[test]
fn a_round_that_finds_something_new_confirms_nothing() {
  kill_with_scans_that_miss(
    |round, tree, listed| {
      without_in_round_0(round, tree, listed, "grandchild", true);
      if let (1, Some(&(pid, _)), Some(listed)) = (round, tree.get("great-grandchild"), listed.as_mut()) {
        listed.retain(|process| process.pid != pid);
      }
    },
    "great-grandchild",
  );
}

/// A scan that reads no list (a churn of processes no read kept up with) is no empty scan: after a first round that
/// missed the great-grandchild, a second one that reads no list confirms nothing, and a third finds it
#[cfg(unix)]
#[test]
fn a_scan_that_reads_no_list_confirms_nothing() {
  kill_with_scans_that_miss(
    |round, tree, listed| {
      without_in_round_0(round, tree, listed, "great-grandchild", false);
      if round == 1 {
        *listed = None;
      }
    },
    "great-grandchild",
  );
}

/// A round in which a process did not halt confirms nothing (the seam of the tests says so of the grandchild in the
/// second round): the kill takes a third round, which confirms the tree
#[cfg(unix)]
#[test]
fn a_round_in_which_a_process_did_not_halt_confirms_nothing() {
  let dir = tempfile::tempdir().unwrap();
  let tree = std::rc::Rc::new(std::cell::RefCell::new(Tree::new()));
  let known = std::rc::Rc::clone(&tree);
  crate::process_tree::seam::processes_run_on(move |round, pid| round == 1 && known.borrow().get("grandchild").is_some_and(|&(grandchild, _)| grandchild == pid));
  let kill = kill_once_the_tree_runs(&dir, &tree).expect("the kill of the tree");
  assert_eq!(kill.rounds.len(), 3, "{kill:?}");
  assert!(kill.settled(), "{kill:?}");
  wait_for_the_tree(&tree.borrow(), &Some(kill));
}

/// The pids a child wrote into `pids` that `started` lacks, each with its start, read as soon as its pid is written: a
/// pid the system gives again is no survivor; how many `started` holds
fn record_forks(pids: &std::path::Path, started: &std::cell::RefCell<Vec<(u32, Option<u64>)>>) -> usize {
  let written: Vec<u32> = std::fs::read_to_string(pids)
    .unwrap_or_default()
    .lines()
    .filter_map(|pid| pid.trim().parse().ok())
    .collect();
  let mut started = started.borrow_mut();
  for &pid in &written[started.len().min(written.len())..] {
    started.push((pid, process_started(pid)));
  }
  started.len()
}

/// The child of `fake` that starts a `sleep 120` every 10 ms and writes its pid, as the system names it, into `pids`
fn child_forking_into(dir: &tempfile::TempDir, pids: &std::path::Path) -> std::path::PathBuf {
  let pid_of_last = if cfg!(windows) { "cat /proc/$!/winpid" } else { "echo $!" };
  fake(dir, &format!("while :; do\n  sleep 120 &\n  {pid_of_last} >> '{}'\n  sleep 0.01\ndone", shell_path(pids)))
}

/// The kill of a child that a process outside the kill continues after its last round, with what the kill stopped below
/// it (an external `SIGCONT`, as the `fg` of a terminal sends its group: the child may wait for a stopped `sleep 0.01`),
/// and that forks before it is stopped again; the seams of `seams` set first. The scan after the kill finds what the
/// child started (`left` holds each such fork, which the test asserts) and kills it: what the kill reports. No deadline
/// but nextest's
#[cfg(unix)]
fn kill_after_an_external_sigcont(seams: impl FnOnce()) -> crate::process_tree::KillReport {
  seams();
  let dir = tempfile::tempdir().unwrap();
  let pids = dir.path().join("pids");
  let child = child_forking_into(&dir, &pids);
  let started = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
  let (outside, forks) = (pids.clone(), std::rc::Rc::clone(&started));
  // the forks after the confirm, as the hook saw them
  let after = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
  let after_confirm = std::rc::Rc::clone(&after);
  crate::process_tree::seam::when_confirmed(move |root| {
    let root_started = process_started(root).unwrap();
    let below = crate::process_scan::descendants(root, root_started).unwrap_or_default();
    let tree: Vec<String> = std::iter::once(root).chain(below.iter().map(|process| process.pid)).map(|pid| pid.to_string()).collect();
    Command::new("sh").args(["-c", &format!("kill -CONT {}", tree.join(" "))]).status().unwrap();
    // a fork after the confirm: the pid of one the rounds found may be written only now, its `echo` stopped by the kill
    let is_new = |pid: &u32| !below.iter().any(|process| process.pid == *pid);
    while {
      record_forks(&outside, &forks);
      !forks.borrow().iter().any(|(pid, _)| is_new(pid))
    } {
      std::thread::sleep(Duration::from_millis(1));
    }
    crate::process_scan::stop(root, root_started);
    while crate::process_scan::halted(root) == Some(false) {
      std::thread::sleep(Duration::from_millis(1));
    }
    // each fork of the child since the confirm, once the child stopped again
    record_forks(&outside, &forks);
    *after_confirm.borrow_mut() = forks.borrow().iter().copied().filter(|(pid, _)| is_new(pid)).collect::<Vec<_>>();
  });
  let result = run_child_until_superseded(&mut windowless_command(child), None, &|| record_forks(&pids, &started) >= 3);
  assert!(matches!(result, Err(ChildProcessError::Superseded)), "{result:?}");
  let kill = last_kill().expect("the kill of the tree");
  let missed: Vec<u32> = after
    .borrow()
    .iter()
    .filter(|(pid, started)| started.is_some() && !kill.left.iter().any(|process| process.pid == *pid))
    .map(|(pid, _)| *pid)
    .collect();
  assert!(missed.is_empty(), "the forks after the confirm {missed:?} are not left: {kill:?}");
  record_forks(&pids, &started);
  let started: Vec<(u32, Option<u64>)> = started.borrow().iter().copied().filter(|(_, started)| started.is_some()).collect();
  wait_for_the_exits(&started, "a process the child started", &Some(kill.clone()));
  kill
}

/// A fork after an external `SIGCONT` after the last round (`kill_after_an_external_sigcont`) is killed, and the kill
/// says it is not settled because of it (`Unsettled::Left`)
#[cfg(unix)]
#[test]
fn a_fork_after_an_external_sigcont_after_the_last_round_is_killed_and_unsettles_the_kill() {
  let kill = kill_after_an_external_sigcont(|| {});
  assert_eq!(kill.unsettled, [crate::process_tree::Unsettled::Left], "{kill:?}");
}

/// A kill whose wait for the exits ran out (the seam of the tests) still scans after it: the fork is killed too, and the
/// kill names both reasons
#[cfg(unix)]
#[test]
fn a_kill_whose_exit_wait_ran_out_still_kills_the_forks_after_its_last_round() {
  let kill = kill_after_an_external_sigcont(crate::process_tree::seam::run_exit_waits_out);
  assert_eq!(kill.unsettled, [crate::process_tree::Unsettled::ExitWait, crate::process_tree::Unsettled::Left], "{kill:?}");
}

/// A scan after the kill that reads no list is tried again: the next one finds the fork
#[cfg(unix)]
#[test]
fn a_scan_after_the_kill_that_reads_no_list_is_tried_again() {
  let kill = kill_after_an_external_sigcont(|| {
    let mut first = true;
    crate::process_tree::seam::change_scans(move |round, listed| {
      if round == usize::MAX && std::mem::take(&mut first) {
        *listed = None;
      }
    });
  });
  assert_eq!(kill.unsettled, [crate::process_tree::Unsettled::Left], "{kill:?}");
}

/// A child that starts a process every 10 ms while it is killed leaves none of them: what it forks while the kill scans
/// is a child of a process that the scan stopped or the next pass finds. The kill comes after 20 starts, however long
/// they take; no deadline but nextest's
#[test]
fn a_killed_child_that_keeps_forking_leaves_no_process_it_started() {
  let dir = tempfile::tempdir().unwrap();
  let pids = dir.path().join("pids");
  let child = child_forking_into(&dir, &pids);
  let started: std::cell::RefCell<Vec<(u32, Option<u64>)>> = std::cell::RefCell::new(Vec::new());
  let forked = || record_forks(&pids, &started);
  let result = run_child_until_superseded(&mut windowless_command(child), None, &|| forked() >= 20);
  assert!(matches!(result, Err(ChildProcessError::Superseded)), "{result:?}");
  let kill = last_kill();
  forked();
  let started = started.into_inner();
  assert!(started.len() >= 20, "{started:?}");
  // one that exited before its start was read is gone
  let started: Vec<(u32, Option<u64>)> = started.into_iter().filter(|(_, started)| started.is_some()).collect();
  wait_for_the_exits(&started, "a process the child started", &kill);
}

#[test]
fn kills_the_child_when_superseded() {
  let dir = tempfile::tempdir().unwrap();
  let mut hang = windowless_command(fake(&dir, HANG));
  let started = Instant::now();
  // a newer request came before the child ended
  let result = run_child(&mut hang, None, TIMEOUT, &|| true);
  assert!(matches!(result, Err(ChildProcessError::Superseded)), "{result:?}");
  assert!(started.elapsed() < Duration::from_mins(1), "{:?}", started.elapsed());
}

#[test]
fn the_failure_message_is_the_first_stderr_line_or_the_exit_code() {
  let dir = tempfile::tempdir().unwrap();
  let tool = fake_executable(dir.path(), "tool", "echo >&2\necho '  boom  ' >&2\necho second >&2\nexit 3");
  let output = run_child(&mut windowless_command(&tool), None, TIMEOUT, &|| false).unwrap();
  assert_eq!(output.failure_message("tool"), "boom");

  let tool = fake_executable(dir.path(), "tool", "exit 3");
  let output = run_child(&mut windowless_command(&tool), None, TIMEOUT, &|| false).unwrap();
  assert_eq!(output.failure_message("tool"), "tool exited with code 3");
}

#[test]
fn finds_the_first_executable_in_path() {
  let first = tempfile::tempdir().unwrap();
  let second = tempfile::tempdir().unwrap();
  let expected = fake_executable(second.path(), "tool", "true");
  let path = std::env::join_paths([first.path(), second.path()]).unwrap();
  assert_eq!(find_executable("tool", &path), Some(expected));
  assert_eq!(find_executable("other", &path), None);
}

#[test]
fn finds_a_tool_before_path_then_on_path_then_after_it() {
  const LOOKUP: ToolLookup = ToolLookup {
    name: "tool",
    before_path: &[ToolDir::Home("first")],
    after_path: &[ToolDir::Home("last")],
    login_shell: false,
  };
  let home = tempfile::tempdir().unwrap();
  let bin = tempfile::tempdir().unwrap();
  let path = bin.path().as_os_str();
  assert_eq!(find_tool_in(&LOOKUP, Some(home.path()), Some(path)), None);

  std::fs::create_dir_all(home.path().join("last")).unwrap();
  let last = fake_executable(&home.path().join("last"), "tool", "true");
  assert_eq!(find_tool_in(&LOOKUP, Some(home.path()), Some(path)), Some(last));

  let on_path = fake_executable(bin.path(), "tool", "true");
  assert_eq!(find_tool_in(&LOOKUP, Some(home.path()), Some(path)), Some(on_path.clone()));
  // without a home directory, the home dirs are skipped, not an error
  assert_eq!(find_tool_in(&LOOKUP, None, Some(path)), Some(on_path));

  std::fs::create_dir_all(home.path().join("first")).unwrap();
  let first = fake_executable(&home.path().join("first"), "tool", "true");
  assert_eq!(find_tool_in(&LOOKUP, Some(home.path()), Some(path)), Some(first));
}

#[test]
fn hands_over_each_line_as_it_comes_and_returns_the_output() {
  let dir = tempfile::tempdir().unwrap();
  let child = fake(&dir, "echo one; echo two >&2; printf 'three\\rfour\\n'");
  let lines = std::cell::RefCell::new(Vec::new());
  let output = run_child_lines(&mut Command::new(child), TIMEOUT, &|line| lines.borrow_mut().push(line.to_string()), &|| false).unwrap();
  let mut lines = lines.into_inner();
  // the order of stdout and stderr between their readers is not fixed
  lines.sort();
  assert_eq!(lines, ["four", "one", "three", "two"]);
  assert_eq!((String::from_utf8_lossy(&output.stdout).as_ref(), output.stderr.as_str()), ("one\nthree\nfour\n", "two"));
}

#[cfg(unix)]
#[test]
fn a_cancel_ends_the_whole_process_group() {
  let dir = tempfile::tempdir().unwrap();
  let grandchild_pid = dir.path().join("grandchild-pid");
  // the grandchild outlives a kill of the child alone, by two minutes
  let child = fake(&dir, &format!("(sleep 120; :) & echo $! > {}; echo started; sleep 120", grandchild_pid.display()));
  let canceled = AtomicBool::new(false);
  let started = Instant::now();
  let result = run_child_lines(
    &mut Command::new(child),
    TIMEOUT,
    &|line| {
      if line == "started" {
        canceled.store(true, Ordering::SeqCst);
      }
    },
    &|| canceled.load(Ordering::SeqCst),
  );
  assert!(matches!(result, Err(ChildProcessError::Superseded)), "{result:?}");
  assert!(started.elapsed() < Duration::from_mins(1), "{:?}", started.elapsed());
  // the group got its signals before the cancel returned: the grandchild ends within moments, never after its sleep
  let pid: u32 = std::fs::read_to_string(&grandchild_pid).unwrap().trim().parse().unwrap();
  let deadline = Instant::now() + Duration::from_mins(1);
  while is_running(pid, None) {
    assert!(Instant::now() < deadline, "the grandchild {pid} survived the cancel");
    std::thread::sleep(Duration::from_millis(5));
  }
}

/// A long child spawned beside a short one may inherit its pipes: the short one ends when it exits, not with the long one
#[cfg(unix)]
#[test]
fn a_child_that_exited_does_not_wait_for_a_neighbour_that_holds_its_pipes() {
  let dir = tempfile::tempdir().unwrap();
  let short = fake(&dir, "echo hi");
  let go = dir.path().join("go");
  // the long children end only once every short one ended: a short one that waited for them would run into its timeout
  let long = fake_executable(dir.path(), "long", &format!("while [ ! -e {} ]; do sleep 0.01; done", go.display()));
  std::thread::scope(|scope| {
    for _ in 0..4 {
      scope.spawn(|| run_child_lines(&mut Command::new(&long), Duration::from_mins(1), &|_| {}, &|| false));
    }
    let shorts: Vec<_> = (0..8)
      .map(|_| {
        scope.spawn(|| {
          for _ in 0..30 {
            let output = run_child_lines(&mut Command::new(&short), TIMEOUT, &|_| {}, &|| false).unwrap();
            assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\n");
          }
        })
      })
      .collect();
    let mut failed = None;
    for short in shorts {
      if let Err(panic) = short.join() {
        failed.get_or_insert(panic);
      }
    }
    std::fs::write(&go, "").unwrap();
    if let Some(panic) = failed {
      std::panic::resume_unwind(panic);
    }
  });
}

/// The niceness on Unix, the priority class on Windows
#[cfg(unix)]
type Priority = libc::c_int;
#[cfg(windows)]
type Priority = u32;

/// The priority of a background child of a parent at `parent`: nice 19, or the parent's when that is higher
#[cfg(unix)]
fn background_under(parent: Priority) -> Priority {
  parent.max(19)
}

/// The priority of a background child of a parent of the class `parent`: the below-normal class, or the idle class of
/// an idle parent
#[cfg(windows)]
fn background_under(parent: Priority) -> Priority {
  use windows_sys::Win32::System::Threading::{BELOW_NORMAL_PRIORITY_CLASS, IDLE_PRIORITY_CLASS};
  if parent == IDLE_PRIORITY_CLASS { parent } else { BELOW_NORMAL_PRIORITY_CLASS }
}

/// The niceness of `child`, alive
#[cfg(unix)]
#[expect(unsafe_code, reason = "getpriority(2) of a child, which std does not offer")]
fn priority_of(child: &Child) -> Priority {
  // SAFETY: getpriority takes plain integers; a child that is alive has a priority to answer
  unsafe { libc::getpriority(libc::PRIO_PROCESS, child.id()) }
}

/// The priority class of `child`, alive
#[cfg(windows)]
#[expect(unsafe_code, reason = "GetPriorityClass of a child, which std does not offer")]
fn priority_of(child: &Child) -> Priority {
  use std::os::windows::io::AsRawHandle;
  // SAFETY: the handle belongs to `child`, which lives for the call
  unsafe { windows_sys::Win32::System::Threading::GetPriorityClass(child.as_raw_handle()) }
}

/// A child of work done ahead of a need runs below the app (nice 19, the below-normal class on Windows); a child the
/// user waits for keeps the priority a child of the app gets by default, whatever the priority the tests run at (a test
/// at nice 20, the top on macOS, keeps its background child there)
#[test]
fn a_background_child_runs_below_the_app() {
  let dir = tempfile::tempdir().unwrap();
  let waiting = fake(&dir, "read line");
  let default = priority_of_started(&mut Command::new(&waiting));
  let normal = priority_of_started(&mut windowless_command_at(&waiting, ChildPriority::Normal));
  let background = priority_of_started(&mut windowless_command_at(&waiting, ChildPriority::Background));
  assert_eq!((normal, background), (default, background_under(default)));
}

/// The priority of the child of `command`, whose program waits for its stdin (`fake(dir, "read line")`): alive while
/// its priority is read
fn priority_of_started(command: &mut Command) -> Priority {
  let mut child = command.stdin(Stdio::piped()).spawn().unwrap();
  let priority = priority_of(&child);
  drop(child.stdin.take());
  child.wait().unwrap();
  priority
}

/// Lowers the priority of this test process below the one a child asks for: nice 15 at least on Unix, the idle class
/// on Windows. A process never takes its priority back, so only a test that runs in a process of its own calls it, as
/// nextest runs each test
#[cfg(unix)]
#[expect(unsafe_code, reason = "setpriority(2) of this process, which std does not offer")]
fn lower_own_priority() -> Priority {
  let nice = crate::child_process::own_niceness().max(15);
  // SAFETY: setpriority takes plain integers; a process may raise its own niceness
  let set = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
  assert_eq!(set, 0, "nice {nice}: {}", std::io::Error::last_os_error());
  nice
}

/// Lowers the priority of this test process below the one a child asks for: nice 15 at least on Unix, the idle class
/// on Windows. A process never takes its priority back, so only a test that runs in a process of its own calls it, as
/// nextest runs each test
#[cfg(windows)]
#[expect(unsafe_code, reason = "SetPriorityClass of this process, which std does not offer")]
fn lower_own_priority() -> Priority {
  use windows_sys::Win32::System::Threading::{GetCurrentProcess, IDLE_PRIORITY_CLASS, SetPriorityClass};
  // SAFETY: the pseudo handle of this process needs no closing and is valid for the call
  let set = unsafe { SetPriorityClass(GetCurrentProcess(), IDLE_PRIORITY_CLASS) };
  assert_ne!(set, 0, "the idle class: {}", std::io::Error::last_os_error());
  IDLE_PRIORITY_CLASS
}

/// A child never runs above its parent, and no priority it asks for fails its start: under a parent at nice 15 a child
/// asked for nice 10 starts at 15, the niceness `child_niceness` names, and a background child at 19; under a parent of
/// the idle class on Windows a background child is idle too
#[test]
fn a_child_never_runs_above_its_parent_and_starts_whatever_priority_it_asks_for() {
  let dir = tempfile::tempdir().unwrap();
  let waiting = fake(&dir, "read line");
  let parent = lower_own_priority();
  let normal = priority_of_started(&mut windowless_command_at(&waiting, ChildPriority::Normal));
  let background = priority_of_started(&mut windowless_command_at(&waiting, ChildPriority::Background));
  #[cfg(unix)]
  {
    let mut asked = windowless_command_at(&waiting, ChildPriority::Normal);
    crate::child_process::set_niceness(&mut asked, 10);
    let asked = priority_of_started(&mut asked);
    let named = crate::child_process::child_niceness(10);
    assert_eq!((normal, background, asked, named), (parent, background_under(parent), parent, parent));
  }
  #[cfg(windows)]
  assert_eq!((normal, background), (parent, background_under(parent)));
}
