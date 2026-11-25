use crate::process_scan::{ProcessScan, host_name, is_running, process_started, start_clock, uptime_ticks};

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

/// A recorded start time tells the process from another one that got its pid later; once it exited it never runs, on
/// Windows also while the handle of its `Child` still keeps its pid
#[test]
fn a_process_runs_only_with_the_start_time_it_was_recorded_with() {
  let mut sleeping = if cfg!(windows) {
    let mut ping = std::process::Command::new("ping");
    ping.args(["-n", "60", "127.0.0.1"]);
    ping
  } else {
    let mut sleep = std::process::Command::new("sleep");
    sleep.arg("60");
    sleep
  }
  .stdout(std::process::Stdio::null())
  .spawn()
  .unwrap();
  let pid = sleeping.id();
  let started = process_started(pid).unwrap();
  assert!(is_running(pid, Some(started)));
  assert!(is_running(pid, None));
  assert!(!is_running(pid, Some(started + 1)));
  assert!(is_running(std::process::id(), Some(process_started(std::process::id()).unwrap())));
  sleeping.kill().unwrap();
  sleeping.wait().unwrap();
  assert_eq!(process_started(pid), None);
  assert!(!is_running(pid, Some(started)));
  assert!(!is_running(pid, None));
}

/// The holders of a file are the processes with a descriptor of it, each once: this process with two, a child that
/// opened it itself (its descriptors are not inherited); nobody once they closed it; no answer for a file that is gone
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn the_holders_of_a_file_are_the_processes_with_a_descriptor_of_it_each_once() {
  use crate::process_scan::processes_with_open_file;
  let dir = tempfile::tempdir().unwrap();
  let (path, opened) = (dir.path().join("index.lock"), dir.path().join("opened"));
  let written = std::fs::File::create(&path).unwrap();
  let read = std::fs::File::open(&path).unwrap();
  let mut child = std::process::Command::new("sh")
    .args(["-c", "exec 3<\"$1\"; : > \"$2\"; exec sleep 60", "sh"])
    .args([&path, &opened])
    .spawn()
    .unwrap();
  // the child marks that it holds the file
  while !opened.exists() {
    std::thread::sleep(std::time::Duration::from_millis(10));
  }
  let mut holders = processes_with_open_file(&path).unwrap();
  holders.sort_unstable();
  child.kill().unwrap();
  child.wait().unwrap();
  let mut expected = vec![std::process::id(), child.id()];
  expected.sort_unstable();
  assert_eq!(holders, expected);
  drop((written, read));
  assert_eq!(processes_with_open_file(&path), Some(Vec::new()));
  std::fs::remove_file(&path).unwrap();
  assert_eq!(processes_with_open_file(&path), None);
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
    terminal: false,
    session_leader: false,
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

/// Git's maintenance, git's fsmonitor daemon and `caffeinate` in a directory are not inside it, and the maintenance is
/// named on its own; any other process there is, also a command that `caffeinate` wraps and another git
#[test]
fn git_maintenance_the_fsmonitor_daemon_and_caffeinate_do_not_work_in_a_directory() {
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
      in_dir(process(
        40,
        1,
        &["/opt/homebrew/opt/git/libexec/git-core/git", "fsmonitor--daemon", "run", "--detach", "--ipc-threads=8"],
      )),
      in_dir(process(41, 1, &["git", "-c", "core.fsmonitor=true", "status"])),
    ],
    own: std::collections::HashSet::new(),
  };
  let inside: Vec<u32> = scan.inside(dir.path()).iter().map(|process| process.pid).collect();
  assert_eq!(inside, [21, 30, 41]);
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
    process(200, 100, &["zsh", "-c", "brd lease return | jq"]),
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

/// The holder of a lease is an ancestor of `bd` by its pid and its start time, whatever runs between them (a shell,
/// `cargo`); a sibling, a process of the same group and a pid with another start time are not
#[test]
fn an_ancestor_is_a_process_above_this_one_with_its_start_time() {
  let me = std::process::id();
  let mut processes = vec![
    process(1, 0, &["launchd"]),
    process(100, 1, &["claude"]),
    process(200, 100, &["cargo", "run", "-p", "branch-deck", "--", "cli"]),
    process(me, 200, &["bd", "lease", "pick"]),
    process(300, 100, &["zsh"]),
  ];
  processes[1].started = Some(7);
  processes[4].group = 200;
  let scan = ProcessScan::Done {
    processes,
    own: std::collections::HashSet::new(),
  };
  assert_eq!(scan.has_ancestor(100, Some(7)), Some(true));
  assert_eq!(scan.has_ancestor(100, None), Some(true));
  assert_eq!(scan.has_ancestor(200, None), Some(true));
  assert_eq!(scan.has_ancestor(100, Some(8)), Some(false), "another process with the pid of the holder");
  assert_eq!(scan.has_ancestor(300, None), Some(false), "a sibling of the same group");
  assert_ne!(ProcessScan::Unavailable.has_ancestor(100, None), Some(true));
}

/// The path from a process to an ancestor names the processes between them, the parent first: a child of the ancestor
/// has an empty one; a sibling, a process above it and another process with the pid of the ancestor have none
#[test]
fn the_path_to_an_ancestor_names_the_processes_between_them() {
  let mut processes = vec![
    process(1, 0, &["launchd"]),
    process(100, 1, &["claude"]),
    process(200, 100, &["zsh", "-c", "tail -F log | grep x"]),
    process(300, 200, &["tail", "-F", "log"]),
    process(400, 1, &["sleep", "30"]),
  ];
  processes[1].started = Some(7);
  let scan = ProcessScan::Done {
    processes,
    own: std::collections::HashSet::new(),
  };
  let pids = |path: Option<Vec<&crate::process_scan::ProcessInfo>>| path.map(|path| path.iter().map(|process| process.pid).collect::<Vec<_>>());
  assert_eq!(pids(scan.path_to(300, (100, 7))), Some(vec![200]));
  assert_eq!(pids(scan.path_to(200, (100, 7))), Some(vec![]), "a child of the ancestor");
  assert_eq!(pids(scan.path_to(400, (100, 7))), None, "an orphan of launchd");
  assert_eq!(pids(scan.path_to(100, (100, 7))), None, "the ancestor itself");
  assert_eq!(pids(scan.path_to(300, (100, 8))), None, "another process with the pid of the ancestor");
  assert_eq!(pids(ProcessScan::Unavailable.path_to(300, (100, 7))), None);
}

/// The chain of Windows: exes without `.exe`, a runner whatever its case (`Bash`, `cmd`), the agent above them. A parent
/// that started after its child is another process that got the pid of a gone one: the chain ends there, not whole, so
/// nothing above it is told and no ancestor is denied. A link whose start the system won't tell stays in it, but is no
/// holder. A chain that reaches the init process or a parent no longer in the list is whole: what it lacks is no
/// ancestor
#[test]
fn the_chain_skips_the_runners_of_windows_and_ends_at_a_parent_younger_than_its_child() {
  use crate::process_scan::chain;
  let me = 500;
  let mut processes = vec![
    process(4, 0, &["System"]),
    process(100, 4, &["claude"]),
    process(200, 100, &["Bash"]),
    process(300, 200, &["cmd"]),
    process(me, 300, &["brd"]),
  ];
  for (process, started) in processes.iter_mut().zip([1, 10, 20, 30, 40]) {
    process.started = Some(started);
  }
  let started = |process: &crate::process_scan::ProcessInfo| process.started;
  let ancestors = chain(&processes, me, started, true);
  assert_eq!(ancestors.session().map(|process| process.pid), Some(100));
  assert_eq!(ancestors.contains(100, Some(10)), Some(true));
  assert_eq!(ancestors.contains(100, Some(11)), Some(false), "a whole chain tells a reused pid apart");

  // the cmd exited and its pid went to a younger process, which is not the parent
  processes[3].started = Some(50);
  let ancestors = chain(&processes, me, started, true);
  assert_eq!(ancestors.session().map(|process| process.pid), None);
  assert_eq!(ancestors.contains(100, Some(10)), None);
  // where the system gives an orphan to the init process, a younger parent is a clock that stepped back
  assert_eq!(chain(&processes, me, started, false).session().map(|process| process.pid), Some(100));

  // a link whose start the system won't tell (a service of another user) stays in the chain: what is above it is still
  // told, and a younger parent above it is still found by the start of the nearest child that has one
  processes[3].started = Some(30);
  processes[2].started = None;
  let ancestors = chain(&processes, me, started, true);
  assert_eq!((ancestors.session().map(|process| process.pid), ancestors.contains(100, Some(10))), (Some(100), Some(true)));
  processes[1].started = Some(35);
  assert_eq!(
    chain(&processes, me, started, true).contains(100, Some(35)),
    None,
    "older than the cmd below the unknown link"
  );
  // a holder must have a start time: the nearest non-runner without one names no holder
  processes[1].started = None;
  assert_eq!(chain(&processes, me, started, true).session().map(|process| process.pid), None);
  processes[1].started = Some(10);
  processes[2].started = Some(20);

  // the cmd is gone from the list: nothing above it is an ancestor any more
  processes.remove(3);
  let ancestors = chain(&processes, me, started, true);
  assert_eq!((ancestors.session().map(|process| process.pid), ancestors.contains(100, Some(10))), (None, Some(false)));
}

/// The ancestors of this process are read on every platform, the scan's or, without one (Windows), a snapshot of the
/// process list: the parent first, with its start time, still running and told as an ancestor; none only under a
/// parent of pid 1 (a test runner in a container)
#[test]
fn the_ancestors_of_this_process_are_read_on_every_platform() {
  for scan in [ProcessScan::scan(), ProcessScan::Unavailable] {
    let ancestors = scan.ancestors().unwrap();
    #[cfg(unix)]
    let orphan = std::os::unix::process::parent_id() <= 1;
    #[cfg(not(unix))]
    let orphan = false;
    let Some(parent) = ancestors.links.first() else {
      assert!(orphan, "{ancestors:?}");
      continue;
    };
    #[cfg(unix)]
    assert_eq!(parent.pid, std::os::unix::process::parent_id());
    let started = parent.started.unwrap();
    assert!(is_running(parent.pid, Some(started)));
    assert_eq!(scan.has_ancestor(parent.pid, Some(started)), Some(true));
    assert_eq!(scan.session().is_some(), ancestors.session().is_some());
  }
}

/// A kill names the start time of the process it means: another start time (the pid of a process that exited, given to
/// another one) is never signalled, the right one is killed at once
#[cfg(unix)]
#[test]
fn a_kill_signals_only_the_process_that_has_the_start_time() {
  use crate::process_scan::kill;
  let mut child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
  let started = process_started(child.id()).unwrap();
  assert!(!kill(child.id(), started + 1), "another start time is another process");
  assert!(is_running(child.id(), Some(started)));
  assert!(kill(child.id(), started));
  let status = child.wait().unwrap();
  assert!(!status.success());
}

/// A process is halted once `stop` took effect, which each of its threads does as it runs again, never while it runs;
/// one that exited and was reaped tells nothing
#[cfg(unix)]
#[test]
fn a_process_is_halted_once_it_stopped_and_never_while_it_runs() {
  use crate::process_scan::{halted, stop};
  use std::time::{Duration, Instant};
  let mut child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
  let started = process_started(child.id()).unwrap();
  assert_eq!(halted(child.id()), Some(false));
  assert!(stop(child.id(), started));
  let deadline = Instant::now() + Duration::from_mins(1);
  while halted(child.id()) != Some(true) {
    assert!(Instant::now() < deadline, "the stopped sleep is not halted: {:?}", halted(child.id()));
    std::thread::sleep(Duration::from_millis(1));
  }
  child.kill().unwrap();
  child.wait().unwrap();
  assert_eq!(halted(child.id()), None);
}

/// A child killed under its stopped parent has exited only as the zombie it ends as: libproc describes it no more from
/// the start of its exit, while the kernel still has to give its children away, and a kill of the parent then sent
/// SIGHUP to the group of the caller (`process_tree`). The kernel lists it as a zombie the moment `exited` says so (the
/// parent is a `sleep`, which never reaps it; a shell in `wait` would reap it even stopped, as its call returns), and
/// once the parent is reaped it is gone and stays exited
#[cfg(target_os = "macos")]
#[test]
fn a_killed_child_of_a_stopped_parent_has_exited_only_as_its_zombie() {
  use crate::process_scan::{descendants, exited, kill, stop};
  use std::time::{Duration, Instant};
  let mut parent = std::process::Command::new("sh").args(["-c", "sleep 60 & exec sleep 61"]).spawn().unwrap();
  let parent_started = process_started(parent.id()).unwrap();
  let deadline = Instant::now() + Duration::from_mins(1);
  let child = loop {
    if let Some(child) = descendants(parent.id(), parent_started).unwrap().into_iter().next() {
      break child;
    }
    assert!(Instant::now() < deadline, "the sleep of the shell did not start");
    std::thread::sleep(Duration::from_millis(1));
  };
  assert!(stop(parent.id(), parent_started));
  assert!(!exited(child.pid, child.started), "the sleep runs");
  assert!(kill(child.pid, child.started));
  while !exited(child.pid, child.started) {
    assert!(Instant::now() < deadline, "the killed sleep did not exit");
    std::thread::yield_now();
  }
  assert_eq!(
    super::platform::exited(child.pid, child.started),
    Some(true),
    "the kernel lists the sleep as a zombie of its stopped parent"
  );
  parent.kill().unwrap();
  parent.wait().unwrap();
  assert!(exited(child.pid, child.started), "gone with its parent");
}

/// The host is named, the same on every call: the operations registry tells the records of this host by it
#[test]
fn the_host_of_this_process_has_a_name() {
  assert_ne!(host_name(), "");
  assert_eq!(host_name(), host_name());
}

/// The first field of `/proc/uptime` in clock ticks, whatever its decimals
#[test]
fn the_uptime_reads_in_clock_ticks() {
  assert_eq!(uptime_ticks("50.05 1234.56\n", 100), Some(5005));
  assert_eq!(uptime_ticks("12345.6 1.0", 100), Some(1_234_560));
  assert_eq!(uptime_ticks("7 0", 250), Some(1750));
  assert_eq!(uptime_ticks("0.999 0", 1000), Some(999));
  assert_eq!(uptime_ticks("x 0", 100), None);
  assert_eq!(uptime_ticks("", 100), None);
}

/// A process that starts after a reading of the start clock has a start of that reading or later, in the unit of
/// `process_started`: the clock carries no rounding of the boot to its second, which `started_at` of Linux has. No
/// clock where the system tells no start to compare. The child sleeps until its start is read: one that exits first is
/// a zombie, which has no start (`an_exited_child_that_is_not_reaped_yet_has_no_start`), and a loaded host lets a
/// `true` end before the read (126 of 150 runs at twice as many busy loops as cores)
#[test]
fn a_process_started_after_a_reading_of_the_start_clock_starts_at_it_or_later() {
  if cfg!(any(target_os = "macos", target_os = "linux")) {
    let clock = start_clock().expect("the start clock");
    let mut child = crate::child_process::windowless_command("sleep").arg("60").spawn().expect("a child");
    let started = process_started(child.id());
    child.kill().expect("the child killed");
    child.wait().expect("the child ended");
    let started = started.expect("the start of the child");
    assert!(started >= clock.now, "{started} < {clock:?}");
    assert!(clock.per_second > 0, "{clock:?}");
  } else {
    assert_eq!(start_clock(), None);
  }
}

/// A child that exited is a zombie until its parent reaps it, and a zombie has no start: it is no process that runs, so
/// a pid of it never counts as running (`process_started` answers `None` for one that exited). The state, not a time:
/// the loop waits until the child exited, on Linux while `/proc` still lists the zombie
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn an_exited_child_that_is_not_reaped_yet_has_no_start() {
  let mut child = crate::child_process::windowless_command("true").spawn().expect("a child");
  let pid = child.id();
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
  while process_started(pid).is_some() {
    assert!(std::time::Instant::now() < deadline, "the child did not exit");
    std::thread::sleep(std::time::Duration::from_millis(1));
  }
  #[cfg(target_os = "linux")]
  assert!(std::path::Path::new(&format!("/proc/{pid}/stat")).exists(), "the zombie is listed until it is reaped");
  child.wait().expect("the child reaped");
  assert_eq!(process_started(pid), None);
}

/// Computes until this process used `cpu` more of CPU time, the state an assertion needs, not a time; answers its CPU
/// time before
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn spin_for_cpu(cpu: std::time::Duration) -> std::time::Duration {
  let pid = std::process::id();
  let before = crate::process_scan::cpu_time(pid).unwrap();
  let mut spin = 0u64;
  while crate::process_scan::cpu_time(pid).unwrap() < before + cpu {
    for _ in 0..100_000 {
      spin = std::hint::black_box(spin.wrapping_add(1));
    }
  }
  before
}

/// The CPU time of a process grows while it computes, and the cheap list of the processes has this one with its group;
/// a gone process has no CPU time. The loop waits for the CPU time itself, not for a time: a loaded host gives this
/// process a fraction of a core (28 ms of CPU in 300 ms of a spin at load 90)
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn the_cpu_time_of_a_process_grows_while_it_computes() {
  let pid = std::process::id();
  spin_for_cpu(std::time::Duration::from_millis(100));
  let own = crate::process_scan::process_list().unwrap().into_iter().find(|process| process.pid == pid).unwrap();
  assert_ne!(own.group, 0);
  assert_eq!(crate::process_scan::cpu_time(u32::MAX - 1), None);
}

/// The CPU times of a process count its own time, in the unit of `cpu_time`, and the time of each child once it reaped
/// it: a shell that waited for a child that computed and ended has that time as its children's, while it waits itself
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn the_cpu_times_of_a_process_count_its_reaped_children() {
  use std::io::Write;
  let pid = std::process::id();
  let before = spin_for_cpu(std::time::Duration::from_millis(100));
  let own = crate::process_scan::cpu_times(pid).unwrap().own;
  assert!(
    own >= before + std::time::Duration::from_millis(100) && own <= crate::process_scan::cpu_time(pid).unwrap(),
    "{own:?}"
  );

  // the child computes and ends; the shell waits for it, then for a line of its stdin
  let mut shell = std::process::Command::new("sh")
    .args(["-c", "sh -c 'i=0; while [ $i -lt 100000 ]; do i=$((i+1)); done'; read line"])
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  let shell_pid = shell.id();
  // the state the assertion needs: the child reaped, not a time
  let times = loop {
    let times = crate::process_scan::cpu_times(shell_pid).unwrap();
    if !times.children.is_zero() {
      break times;
    }
    std::thread::yield_now();
  };
  assert!(times.children > times.own, "the loop of the child, not the wait of the shell: {times:?}");
  assert_eq!(times.total(), times.own + times.children);
  shell.stdin.take().unwrap().write_all(b"done\n").unwrap();
  shell.wait().unwrap();
  assert_eq!(crate::process_scan::cpu_times(u32::MAX - 1), None);
}

/// A process has a terminal when its session has a controlling terminal, as the shell of a terminal tab; a command in a
/// session of its own without one, as a coding agent runs each, has none
#[cfg(unix)]
#[test]
fn a_process_has_a_terminal_only_when_its_session_has_one() {
  let detached = test_utils::session::spawn_in_session(std::process::Command::new("sleep").arg("30"));
  let tab = test_utils::session::spawn_in_terminal(std::process::Command::new("sleep").arg("30"));
  let ProcessScan::Done { processes, .. } = ProcessScan::scan() else { panic!("no scan") };
  let terminal = |pid: u32| processes.iter().find(|process| process.pid == pid).map(|process| process.terminal);
  assert_eq!(terminal(detached.id()), Some(false));
  assert_eq!(terminal(tab.child.id()), Some(true));
}

/// A process leads its session only when it started it: the shell of a terminal tab and a command in a session of its
/// own do, and a command of that shell, which inherits its terminal, does not
#[cfg(unix)]
#[test]
fn a_process_leads_its_session_only_when_it_started_it() {
  let detached = test_utils::session::spawn_in_session(std::process::Command::new("sleep").arg("30"));
  let tab = test_utils::session::spawn_in_terminal(std::process::Command::new("sh").args(["-c", "sleep 30 & wait"]));
  let shell = tab.child.id();
  let deadline = std::time::Instant::now() + std::time::Duration::from_mins(1);
  let processes = loop {
    let ProcessScan::Done { processes, .. } = ProcessScan::scan() else { panic!("no scan") };
    if processes.iter().any(|process| process.parent == shell && process.name == "sleep") {
      break processes;
    }
    assert!(std::time::Instant::now() < deadline, "the shell started no sleep");
    std::thread::yield_now();
  };
  let facts = |found: &dyn Fn(&crate::process_scan::ProcessInfo) -> bool| processes.iter().find(|process| found(process)).map(|process| (process.terminal, process.session_leader));
  assert_eq!(facts(&|process| process.pid == detached.id()), Some((false, true)), "a session of its own");
  assert_eq!(facts(&|process| process.pid == shell), Some((true, true)), "the shell of a tab");
  assert_eq!(facts(&|process| process.parent == shell && process.name == "sleep"), Some((true, false)), "its command");
}
