#[cfg(unix)]
use crate::process_scan::{ProcessScan, is_running, process_started};

/// A child that sleeps in a directory is inside it; this process, the parent of the child, never is
#[cfg(unix)]
#[test]
fn a_process_in_a_directory_is_inside_it_and_this_process_is_left_out() {
  let dir = tempfile::tempdir().unwrap();
  let mut child = std::process::Command::new("sleep").arg("30").current_dir(dir.path()).spawn().unwrap();
  let scan = ProcessScan::scan();
  let inside: Vec<u32> = scan.inside(dir.path()).iter().map(|process| process.pid).collect();
  let nested: Vec<u32> = scan.inside(&dir.path().join("missing")).iter().map(|process| process.pid).collect();
  let own_directory = std::env::current_dir().unwrap();
  let own = scan.inside(&own_directory).iter().any(|process| process.pid == std::process::id());
  child.kill().unwrap();
  child.wait().unwrap();
  assert!(scan.is_available());
  assert_eq!(inside, [child.id()]);
  assert_eq!(nested, Vec::<u32>::new());
  assert!(!own);
}

/// A recorded start time tells the process from another one that got its pid later
#[cfg(unix)]
#[test]
fn a_process_runs_only_with_the_start_time_it_was_recorded_with() {
  let pid = std::process::id();
  let started = process_started(pid).unwrap();
  assert!(is_running(pid, Some(started)));
  assert!(!is_running(pid, Some(started + 1)));
  let mut child = std::process::Command::new("true").spawn().unwrap();
  let child_pid = child.id();
  child.wait().unwrap();
  assert!(!is_running(child_pid, Some(started)));
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
