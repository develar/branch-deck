use crate::process_scan::ProcessScan;
#[cfg(unix)]
use crate::process_scan::{is_running, process_started};

/// A child that sleeps in a directory in a process group of its own is inside it; this process, the parent of the
/// child, never is, and neither is a child in its group (the other commands of a pipeline that runs `bd`)
#[cfg(unix)]
#[test]
fn a_process_in_a_directory_is_inside_it_and_this_process_and_its_group_are_left_out() {
  use std::os::unix::process::CommandExt;
  let dir = tempfile::tempdir().unwrap();
  let mut child = std::process::Command::new("sleep").arg("30").current_dir(dir.path()).process_group(0).spawn().unwrap();
  let mut pipeline = std::process::Command::new("sleep").arg("30").current_dir(dir.path()).spawn().unwrap();
  let scan = ProcessScan::scan();
  let inside: Vec<u32> = scan.inside(dir.path()).iter().map(|process| process.pid).collect();
  let nested: Vec<u32> = scan.inside(&dir.path().join("missing")).iter().map(|process| process.pid).collect();
  let own_directory = std::env::current_dir().unwrap();
  let own = scan.inside(&own_directory).iter().any(|process| process.pid == std::process::id());
  for process in [&mut child, &mut pipeline] {
    process.kill().unwrap();
    process.wait().unwrap();
  }
  assert!(scan.is_available());
  assert_eq!(inside, [child.id()]);
  assert_eq!(nested, Vec::<u32>::new());
  assert!(!own);
}

/// A recorded start time tells the process from another one that got its pid later
#[cfg(unix)]
#[test]
fn a_process_runs_only_with_the_start_time_it_was_recorded_with() {
  let mut sleeping = std::process::Command::new("sleep").arg("60").spawn().unwrap();
  let pid = sleeping.id();
  let started = process_started(pid).unwrap();
  assert!(is_running(pid, Some(started)));
  assert!(is_running(pid, None));
  assert!(!is_running(pid, Some(started + 1)));
  sleeping.kill().unwrap();
  sleeping.wait().unwrap();
  assert!(!is_running(pid, Some(started)));
  assert!(!is_running(pid, None));
}

/// `KERN_PROCARGS2`: argc, the executable path with its padding, the arguments, then the environment, which is left out
#[cfg(target_os = "macos")]
#[test]
fn the_argv_comes_from_the_procargs_buffer_without_the_environment() {
  let mut buffer = 2i32.to_ne_bytes().to_vec();
  buffer.extend_from_slice(b"/usr/bin/java\0\0\0\0bazel(idea-1)\0--workspace_directory=/w\0HOME=/Users/me\0");
  assert_eq!(crate::process_scan::platform::parse_procargs(&buffer), ["bazel(idea-1)", "--workspace_directory=/w"]);
  assert_eq!(crate::process_scan::platform::parse_procargs(&[1, 0]), Vec::<String>::new());
}

/// A process of the scan with `argv` and `parent`, in the group of its own pid
fn process(pid: u32, parent: u32, argv: &[&str]) -> crate::process_scan::ProcessInfo {
  crate::process_scan::ProcessInfo {
    pid,
    parent,
    group: pid,
    name: argv[0].to_string(),
    cwd: None,
    argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
    started: Some(1),
  }
}

/// `git maintenance` and `git gc --auto` are git's background maintenance, with every git below them; a plain `gc`, a
/// shell they start and another git are not
#[test]
fn git_maintenance_is_the_maintenance_and_the_gits_it_starts() {
  let scan = ProcessScan::Done {
    processes: vec![
      process(10, 1, &["git", "maintenance", "run", "--auto", "--detach"]),
      process(11, 10, &["/opt/homebrew/bin/git", "repack", "-d", "-l", "--geometric=2", "--write-midx"]),
      process(12, 11, &["git", "-c", "core.x=y", "multi-pack-index", "write"]),
      process(13, 10, &["sh", "-c", "hook"]),
      process(14, 1, &["git", "status"]),
      process(20, 1, &["git", "-C", "/slot", "gc", "--auto"]),
      process(21, 20, &["git", "pack-refs", "--all"]),
      process(30, 1, &["git", "gc"]),
      process(31, 1, &["sh", "-c", "git maintenance run"]),
    ],
    own: std::collections::HashSet::new(),
  };
  let mut pids: Vec<u32> = scan.git_maintenance().into_iter().collect();
  pids.sort_unstable();
  assert_eq!(pids, [10, 11, 12, 20, 21]);
}

/// Git's maintenance and `caffeinate` in a directory are not inside it, and the maintenance is named on its own; any
/// other process there is, also a command that `caffeinate` wraps
#[test]
fn git_maintenance_and_caffeinate_do_not_work_in_a_directory() {
  let dir = tempfile::tempdir().unwrap();
  let in_dir = |mut process: crate::process_scan::ProcessInfo| {
    process.cwd = Some(std::fs::canonicalize(dir.path()).unwrap());
    process
  };
  let scan = ProcessScan::Done {
    processes: vec![
      in_dir(process(10, 1, &["git", "maintenance", "run", "--auto", "--detach"])),
      in_dir(process(11, 10, &["git", "repack", "-d"])),
      in_dir(process(20, 1, &["caffeinate", "-i", "-t", "300"])),
      in_dir(process(21, 20, &["bazel", "build"])),
      in_dir(process(30, 1, &["java", "-jar", "server.jar"])),
    ],
    own: std::collections::HashSet::new(),
  };
  let inside: Vec<u32> = scan.inside(dir.path()).iter().map(|process| process.pid).collect();
  assert_eq!(inside, [21, 30]);
  assert_eq!(scan.maintenance_inside(dir.path()), [10, 11]);
}

/// The session of a scan: this process, its ancestors and every process of their groups, not the init process's group
#[test]
fn the_session_is_the_ancestors_and_their_groups() {
  let me = std::process::id();
  let mut processes = vec![
    process(1, 0, &["launchd"]),
    process(100, 1, &["claude"]),
    process(101, 100, &["caffeinate", "-i"]),
    process(200, 100, &["zsh", "-c", "bd lease return | jq"]),
    process(me, 200, &["bd", "lease", "return"]),
    process(201, 200, &["jq"]),
    process(300, 100, &["zsh", "-c", "bazel test //..."]),
    process(400, 1, &["sleep"]),
  ];
  processes[2].group = 100;
  processes[4].group = 200;
  processes[5].group = 200;
  processes[7].group = 1;
  let own = crate::process_scan::own_processes(&processes, me);
  let mut own: Vec<u32> = own.into_iter().collect();
  own.sort_unstable();
  let mut expected = vec![1, 100, 101, 200, 201, me];
  expected.sort_unstable();
  assert_eq!(own, expected);
  let scan = ProcessScan::Done {
    processes,
    own: std::collections::HashSet::new(),
  };
  assert_eq!(scan.session().map(|process| process.pid), Some(100));
}
