use super::{SETTLE, named_forms, processes_of};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_utils::process_scan::{ProcessInfo, ProcessScan};

/// A process of a scan with `argv`, working in `cwd`
fn process(pid: u32, argv: &[&str], cwd: &Path) -> ProcessInfo {
  ProcessInfo {
    pid,
    parent: 1,
    group: pid,
    name: argv[0].to_owned(),
    cwd: Some(cwd.to_path_buf()),
    argv: argv.iter().map(|arg| (*arg).to_owned()).collect(),
    started: Some(1),
    terminal: false,
  }
}

/// A process of a home names a path in it in an argument, as the home is given or canonical, or works in the home or in
/// a directory of `also`, so a supervisor of `serve` under launchd, whose working directory is `/`, counts by its run
/// directory. A neighbour whose name only starts as the home's, the session of the test and any other process do not
#[test]
fn a_process_of_a_home_names_it_or_works_in_it() {
  let base = tempfile::tempdir().expect("a temporary directory");
  let base = std::fs::canonicalize(base.path()).expect("the canonical directory");
  let home = base.join("home");
  let repo = base.join("repo");
  std::fs::create_dir_all(&home).expect("the home");
  std::fs::create_dir_all(&repo).expect("the repository");
  // the home as a test gives it, a link to the canonical one, as `/var` is `/private/var` on macOS
  let given = base.join("given");
  std::os::unix::fs::symlink(&home, &given).expect("the link");
  let path = |dir: &Path, rest: &str| -> String { dir.join(rest).to_string_lossy().into_owned() };
  let root = PathBuf::from("/");
  let run_dir = path(&given, ".cache/brd-host/runs/1-1");
  let script = format!("while [ ! -e '{}' ]; do sleep 0.05; done", path(&home, "gate"));
  // neighbours whose names only start as the home's, in either form
  let neighbour = path(&base, "home2/gate");
  let given_neighbour = path(&base, "given2/.cache/brd-host/runs/1-1");
  let scan = ProcessScan::Done {
    processes: vec![
      process(10, &["/usr/local/bin/brd-host", "supervise", &run_dir, "--lock-fd", "3"], &root),
      process(11, &["/bin/sh", "-c", &script], &root),
      process(12, &["sleep", "0.05"], &home.join("slot")),
      process(13, &["git", "status"], &repo),
      process(20, &["/bin/sh", "-c", &format!(": > '{neighbour}'")], &root),
      process(21, &["/usr/local/bin/brd-host", "supervise", &given_neighbour], &root),
      process(22, &["/usr/local/bin/brd-host", "supervise", &run_dir], &root),
      process(23, &["sleep", "1"], &root),
    ],
    own: HashSet::from([22]),
  };
  let dirs = [given.as_path(), repo.as_path()];
  let pids: Vec<u32> = processes_of(&scan, &named_forms(&given), &dirs).iter().map(|process| process.pid).collect();
  assert_eq!(pids, [10, 11, 12, 13]);
}

/// The cancel of a teardown ends every command of a process test within its `SETTLE`: the longest way of a cancelled
/// command to its end (the grace of `PROCESS_TEST_ENV` before the SIGKILL, the wait for the command after it, the end
/// of what it left in its group: a settle, the wait after SIGTERM and a settle after SIGKILL, and the drain of its
/// pipes), about 41 s, comes within three quarters of it, so the scan and the give-back have the rest. With the grace
/// of a host (120 s) a command that a SIGINT did not end outlived the teardown, which killed it and failed the test
/// (`the_end_of_a_test_leaves_no_process_of_its_runs`, 63 s)
#[test]
fn the_grace_of_a_process_test_ends_a_cancelled_command_within_the_settle_of_its_teardown() {
  let grace = crate::serve::PROCESS_TEST_ENV
    .iter()
    .find(|(name, _)| *name == "BD_HOST_TEST_GRACE_MS")
    .map(|(_, ms)| Duration::from_millis(ms.parse().expect("milliseconds")))
    .expect("the process tests set the grace");
  let end_group = crate::tuning::group_settle() + crate::tuning::group_term_wait() + crate::tuning::group_settle();
  let longest = grace + crate::supervise::KILL_WAIT + end_group + crate::supervise::DRAIN;
  assert!(longest <= SETTLE * 3 / 4, "{longest:?} against {SETTLE:?}");
}
