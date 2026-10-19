#[cfg(unix)]
use crate::lease::stop_servers;
use crate::lease::{glob_matches, repository_name, seed_candidates};
#[cfg(unix)]
use crate::process_scan::{ProcessInfo, is_running, process_started};
use std::path::Path;
#[cfg(unix)]
use std::time::Duration;

#[test]
fn a_glob_matches_by_stars_question_marks_and_classes() {
  for (pattern, name) in [
    ("*.local.json", "settings.local.json"),
    ("*", ".env"),
    ("a?c", "abc"),
    ("[ab]x", "bx"),
    ("[a-c]x", "cx"),
    ("[!a]x", "bx"),
    ("[^a]x", "bx"),
    ("[]]x", "]x"),
    ("a[", "a["),
  ] {
    assert!(glob_matches(pattern, name), "{pattern} {name}");
  }
  for (pattern, name) in [("*.json", "a.txt"), ("a?c", "ac"), ("[ab]x", "cx"), ("[!a]x", "ax"), ("x", "xy")] {
    assert!(!glob_matches(pattern, name), "{pattern} {name}");
  }
}

/// Anchored literal paths, a glob in the last component of one directory, a directory; comments, negations, `**`, a
/// glob in a directory component and a path out of the root are left out
#[test]
fn the_seed_candidates_are_literal_paths_and_globs_of_one_directory() {
  let root = tempfile::tempdir().unwrap();
  let write = |path: &str| {
    let path = root.path().join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "x").unwrap();
  };
  write(".claude/settings.local.json");
  write("config/a.local.json");
  write("config/b.local.json");
  write("config/c.json");
  write(".env");
  write("cache/data.bin");
  write("deep/config/d.local.json");
  let text =
    "# local files\n/.claude/settings.local.json\n\n/config/*.local.json\n.env\n/cache/\n!/config/a.local.json\n**/d.local.json\n/*/config/*.json\n../outside\n/missing.txt\n";
  assert_eq!(
    seed_candidates(root.path(), text),
    [".claude/settings.local.json", "config/a.local.json", "config/b.local.json", ".env", "cache"]
  );
}

#[test]
fn the_repository_name_is_the_directory_of_its_main_worktree() {
  assert_eq!(repository_name(Path::new("/projects/idea-1/.git")), "idea-1");
  assert_eq!(repository_name(Path::new("/srv/ultimate.git")), "ultimate");
  assert_eq!(repository_name(Path::new("/srv/bare")), "bare");
}

/// A fake Bazel server of `pid` with the start time `started`
#[cfg(unix)]
fn server(pid: u32, started: Option<u64>) -> ProcessInfo {
  ProcessInfo {
    pid,
    parent: 0,
    name: "bazel(fake)".to_string(),
    cwd: None,
    argv: vec!["bazel(fake)".to_string(), "--workspace_directory=/slot".to_string()],
    started,
  }
}

/// A server whose start time is unknown is never signalled and stays running; one whose pid now has another start time
/// (the system reused the pid) is gone, and the process with that pid is not signalled either
#[cfg(unix)]
#[test]
fn a_server_is_signalled_only_while_its_pid_has_the_start_time_of_the_scan() {
  let mut child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
  let pid = child.id();
  let started = process_started(pid);
  assert!(started.is_some());

  let unknown = server(pid, None);
  assert_eq!(stop_servers(&[&unknown], Duration::from_millis(200)), Vec::new());
  assert!(child.try_wait().unwrap().is_none(), "a server without a start time was signalled");

  let reused = server(pid, started.map(|started| started + 1));
  let stopped = stop_servers(&[&reused], Duration::from_millis(200));
  assert_eq!(stopped.iter().map(|stopped| stopped.pid).collect::<Vec<_>>(), [pid], "the scanned server is gone");
  assert!(child.try_wait().unwrap().is_none(), "the process that got the pid was signalled");
  assert!(is_running(pid, started));

  child.kill().unwrap();
  child.wait().unwrap();
}
