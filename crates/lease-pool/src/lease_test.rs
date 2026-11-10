#[cfg(unix)]
use crate::lease::stop_servers;
use crate::lease::{DirtyCheck, GetStep, Lease, LeaseState, LeaseStore, Pool, Slot, SlotMark, commit_takeover, glob_matches, repository_name, seed_candidates};
use std::path::Path;
use std::time::Duration;
#[cfg(unix)]
use sync_utils::process_scan::{ProcessInfo, is_running, process_started};

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

/// A dirty verdict holds while one dirty path has its stat of then: the other paths may change, a path deleted or
/// rewritten counts as changed, and once every path changed the verdict is gone. A path gone then and gone still is
/// unchanged (a deleted tracked file); `.gitignore` written anew ends it
#[test]
fn a_dirty_verdict_holds_while_one_dirty_path_has_its_stat() {
  let dir = tempfile::tempdir().unwrap();
  let slot = dir.path();
  std::fs::write(slot.join("a.txt"), "a\n").unwrap();
  std::fs::write(slot.join("b.txt"), "b\n").unwrap();
  let check = DirtyCheck::of(slot, vec!["a.txt".to_string(), "b.txt".to_string()]);
  assert!(check.holds(slot));
  std::fs::remove_file(slot.join("a.txt")).unwrap();
  assert!(check.holds(slot), "b.txt proves it");
  std::fs::remove_file(slot.join("b.txt")).unwrap();
  std::fs::write(slot.join("b.txt"), "other\n").unwrap();
  assert!(!check.holds(slot), "every path changed");

  let gone = DirtyCheck::of(slot, vec!["deleted.txt".to_string()]);
  assert!(gone.holds(slot));
  std::fs::write(slot.join(".gitignore"), "/deleted.txt\n").unwrap();
  assert!(!gone.holds(slot), "a new .gitignore");
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
    group: 0,
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
  assert_eq!(stop_servers(&[&unknown], None, Duration::from_millis(200)).stopped, Vec::new());
  assert!(child.try_wait().unwrap().is_none(), "a server without a start time was signalled");

  let reused = server(pid, started.map(|started| started + 1));
  let stopped = stop_servers(&[&reused], None, Duration::from_millis(200)).stopped;
  assert_eq!(stopped.iter().map(|stopped| stopped.pid).collect::<Vec<_>>(), [pid], "the scanned server is gone");
  assert!(child.try_wait().unwrap().is_none(), "the process that got the pid was signalled");
  assert!(is_running(pid, started));

  child.kill().unwrap();
  child.wait().unwrap();
}

/// The commit point of a takeover holds only while the slot is as the check found it: a commit of a resumed session in
/// the slot meanwhile (another mark) stops it and leaves the state alone; the mark the check saw moves the idle lease
/// into the record of the slot
#[test]
fn a_commit_during_the_check_stops_the_takeover() {
  let dir = tempfile::tempdir().unwrap();
  let pool = Pool::new(dir.path());
  let lease = |id: &str, state: LeaseState| Lease {
    id: id.to_string(),
    holder: id.to_string(),
    leased_at: 1,
    state,
    pid: std::process::id(),
    process_started: None,
    source_head: "a".repeat(40),
    source: String::new(),
    source_branch: Some("master".to_string()),
    branch: None,
    seeded: Vec::new(),
    reused: true,
    holder_pid: None,
    holder_started: None,
    taking_over: None,
    idle_check: None,
  };
  let get = Lease {
    taking_over: Some(Box::new(lease("idle", LeaseState::Ready))),
    ..lease("get", LeaseState::Preparing)
  };
  // no such directory: the mark of the slot is the empty one
  let path = dir.path().join("slot").to_string_lossy().into_owned();
  let before = LeaseStore {
    slots: vec![Slot {
      name: "slot".to_string(),
      path: path.clone(),
      dirty: false,
      dirty_check: None,
      returned: None,
      returned_at: None,
      lease: Some(get.clone()),
      reclaimed: None,
    }],
  };
  pool
    .update(Duration::from_secs(5), |store| {
      *store = before.clone();
      Ok(())
    })
    .unwrap();
  let moved = SlotMark {
    head: "ref: refs/heads/lane".to_string(),
    log_len: 1,
    log_modified: 1,
  };
  assert!(!commit_takeover(&pool, Duration::from_secs(5), &path, &get, &moved).unwrap());
  assert_eq!(pool.read().unwrap(), before);

  assert!(commit_takeover(&pool, Duration::from_secs(5), &path, &get, &SlotMark::default()).unwrap());
  let slot = pool.read().unwrap().slots.remove(0);
  assert_eq!(slot.lease.as_ref().map(|lease| (lease.id.as_str(), lease.taking_over.is_none())), Some(("get", true)));
  let reclaimed = slot.reclaimed.unwrap();
  assert_eq!((reclaimed.lease_id.as_str(), reclaimed.by_lease_id.as_deref()), ("idle", Some("get")));
}

/// The wait for the shrink or the eviction of another get names the holder, how long it holds the lock, and the free
/// space of the volume against the minimum, so a wait of minutes reads as disk pressure, never as a hang
#[test]
fn the_wait_for_the_shrink_of_another_names_the_holder_its_time_and_the_free_space() {
  let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() - 600;
  let holder = history_rewrite::repository_lock::LockHolder {
    command: "brd lease get --holder win-native -C ~/projects/idea-1".to_string(),
    pid: 4242,
    process_started: None,
    since,
    subject: None,
  };
  let step = GetStep::WaitForPressure {
    holder,
    free: Some(195 << 30),
    minimum: 200 << 30,
  };
  let words = step.to_string();
  assert!(
    words.starts_with("waiting for the shrink or the eviction of another get, `brd lease get --holder win-native -C ~/projects/idea-1` (pid 4242, for 10 min"),
    "{words}"
  );
  assert!(
    words.ends_with("the volume at 195.0 GiB of the 200.0 GiB this get needs: the free space is measured again after it, or once it has the minimum"),
    "{words}"
  );
}
