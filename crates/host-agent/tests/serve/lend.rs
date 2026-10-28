//! The lend of a host slot and the git of the host: the keeper fills and gives back the slots, a failed lend and a git
//! below its floor refuse the run, and a run and a `Have` wait while another process upgrades git. A test writes the
//! lend table under its lock, as another process of the host would

use super::{ALL_PHASES, Agent, World, events, eventually, write_script};
use history_rewrite::repository_lock::lock_file;
use host_link::proto::{Reply, Request, Stream};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sync_utils::fs_retry;
use sync_utils::process_scan::process_started;
use test_utils::git_test_utils::TestRepo;

/// The line of a run that waits for an upgrade of git
const GIT_WAIT_LINE: &str = "bd-host: waiting for the upgrade of git on the host: `brew upgrade git` runs once no host slot is lent";

fn now_ms() -> u64 {
  u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).expect("after 1970").as_millis()).expect("a time in u64")
}

/// Changes the lend table of `repo` under its lock, as another process of the host would
fn edit_table(repo: &TestRepo, edit: impl FnOnce(&mut Value)) {
  let dir = repo.path().join(".git/branch-deck");
  fs::create_dir_all(&dir).expect("the state directory");
  let _lock = lock_file(dir.join("host-slots.lock"), Duration::from_secs(30), &|| {}).expect("the lock of the lend table");
  let file = dir.join("host-slots.json");
  let mut table = fs::read(&file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_else(|| json!({}));
  edit(&mut table);
  let written = dir.join("host-slots.json.edit");
  fs::write(&written, serde_json::to_vec_pretty(&table).expect("JSON")).expect("the table");
  fs_retry::rename(&written, &file).expect("the table in place");
}

/// Records an upgrade of git by a live process, this one, as `ensure_host_git` of another agent records it
fn record_live_upgrade(repo: &TestRepo) {
  let pid = std::process::id();
  edit_table(repo, |table| {
    table["git"] = json!({
      "kind": "upgrading",
      "owner": { "pid": pid, "process_started": process_started(pid) },
      "command": "brew upgrade git",
      "since_ms": 1,
    });
  });
}

/// Removes the record of the upgrade of git: Homebrew has ended
fn remove_upgrade(repo: &TestRepo) {
  edit_table(repo, |table| {
    if let Some(table) = table.as_object_mut() {
      table.remove("git");
    }
  });
}

impl Agent<'_> {
  /// The state of each host slot of `repo` in the lend table, as `Request::Slots` answers it
  fn slot_states(&self, repo: &Path) -> Vec<String> {
    let Reply::Slots { repositories } = self.reply(&Request::Slots) else {
      panic!("no slots");
    };
    repositories
      .iter()
      .filter(|repository| Path::new(&repository.repo) == repo)
      .flat_map(|repository| repository.table["slots"].as_array().cloned().unwrap_or_default())
      .map(|slot| slot["state"]["kind"].as_str().unwrap_or_default().to_owned())
      .collect()
  }

  /// The exit code of each run, as `Request::Runs` answers it
  fn exits(&self) -> Vec<(String, Option<i32>)> {
    let Reply::Runs { runs } = self.reply(&Request::Runs) else {
      panic!("no runs");
    };
    runs.into_iter().map(|run| (run.run, run.exit)).collect()
  }
}

impl World {
  /// Writes the repository list of the agent, so the keeper keeps those repositories from the start of `serve`
  fn remember(&self, repos: &[&Path]) {
    let text: String = repos.iter().flat_map(|repo| [repo.display().to_string(), "\n".to_owned()]).collect();
    let file = self.home.path().join(".cache/bd-host/repos");
    fs::create_dir_all(file.parent().expect("a parent")).expect("the cache directory");
    fs::write(&file, text).expect("the repository list");
  }
}

/// A `Have` is a git user of the host, so it waits while another process upgrades git, and its heartbeats go on
#[test]
fn a_have_sends_heartbeats_while_it_waits_for_an_upgrade_of_git() {
  let world = World::new(1, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_HEARTBEAT_MS", "100")]);
  record_live_upgrade(&world.repo);
  let (heartbeats, last) = thread::scope(|scope| {
    scope.spawn(|| {
      thread::sleep(Duration::from_millis(600));
      remove_upgrade(&world.repo);
    });
    agent.reply_after_heartbeats(&Request::Have {
      repo: world.repo.path_str().to_owned(),
      commits: vec![world.repo.head()],
    })
  });
  assert_eq!(last, Reply::Have { commits: vec![world.repo.head()] });
  assert!(heartbeats >= 3, "{heartbeats} heartbeats");
}

/// The keeper fills the slots of a repository of the repository list without a run, and gives back the slot of a lend
/// whose process is gone. A failed fill of another repository waits 1 min before the next one, while the keeper goes on
#[test]
fn the_keeper_fills_the_slots_and_backs_off_after_a_failed_fill() {
  let world = World::new(1, &[]);
  let broken = TestRepo::new();
  broken.create_commit_with_files("first", &[("a.txt", "a\n")]);
  broken.git(&["config", "branchdeck.hostSlots", "many"]);
  world.remember(&[world.repo.path(), broken.path()]);
  let agent = world.serve_with(&[("BD_HOST_TEST_KEEPER_MS", "100")]);

  eventually("the keeper fills the slot", || agent.slot_states(world.repo.path()) == ["idle"]);
  let failed_fill = format!("cannot add the host slots of {}: ", broken.path().display());
  let failed = || world.serve_log().matches(&failed_fill).count();
  eventually("the failed fill", || failed() == 1);
  assert!(world.serve_log().contains("; again in 1 min"), "{}", world.serve_log());

  // the slot is lent to a process that is gone
  let mut gone = Command::new("/usr/bin/true").spawn().expect("a process");
  gone.wait().expect("its end");
  let pid = gone.id();
  edit_table(&world.repo, |table| {
    table["slots"][0]["state"] = json!({"kind": "lent", "run": "gone-1", "pid": pid, "process_started": 1, "since_ms": now_ms()});
  });
  let gave_back = format!("the host slots of {}: slot 1: GaveBack", world.repo.path().display());
  eventually("the keeper gives the abandoned slot back", || {
    world.serve_log().contains(&gave_back) && agent.slot_states(world.repo.path()) == ["idle"]
  });
  thread::sleep(Duration::from_secs(1));
  assert_eq!(failed(), 1, "the keeper ticked every 100 ms, but a failed fill waits 1 min: {}", world.serve_log());
}

#[test]
fn a_failed_lend_refuses_the_run_with_lease_failed() {
  let world = World::new(1, &[]);
  let table = world.repo.path().join(".git/branch-deck/host-slots.json");
  fs::create_dir_all(table.parent().expect("a parent")).expect("the state directory");
  fs::write(&table, "{ the table is broken").expect("a broken table");
  let agent = world.serve();
  agent.start(world.spec("7-1", "true"));
  let followed = agent.follow("7-1");
  assert!(
    matches!(&followed.end, Reply::Refused { code, message } if code == "lease_failed" && message.contains("host-slots.json")),
    "{followed:?}"
  );
  assert_eq!(followed.phase_names(), ["slot"]);
  assert_eq!(agent.exits(), [("7-1".to_owned(), Some(255))]);
}

/// A git of the host below its floor, on a host without Homebrew, ends a run with `old_git` and the hint of Branch Deck,
/// and the keeper logs the refusal of its ensure
#[test]
fn a_git_below_the_floor_ends_the_run_with_old_git_and_the_hint_of_bd() {
  let world = World::new(1, &[]);
  let agent = world.serve_with(&[("BD_TEST_HOST_MIN_VERSION", "99.0.0"), ("BD_TEST_HOST_BREW", "none")]);
  agent.start(world.spec("8-1", "true"));
  let followed = agent.follow("8-1");
  let hint = "older than 99.0.0, the floor of a host of remote runs";
  assert!(
    matches!(&followed.end, Reply::Refused { code, message } if code == "old_git" && message.contains(hint)),
    "{followed:?}"
  );
  assert_eq!(followed.phase_names(), ["slot"]);
  assert_eq!(agent.exits(), [("8-1".to_owned(), Some(255))]);
  eventually("the refusal of the ensure in the log", || {
    let log = world.serve_log();
    log.contains("the git of the host stays below its floor: ") && log.contains(hint)
  });
}

/// While another process upgrades the git of the host, a run waits in the phase `git` and says why. Then its command
/// finds the git of the host first in `PATH`, and the line before the command names its version
#[test]
fn a_run_waits_while_homebrew_upgrades_git_and_its_command_gets_the_git_of_the_host() {
  let world = World::new(1, &[]);
  let agent_path = std::env::var_os("PATH").expect("a PATH");
  let real = lease_pool::host_git::which("git", &agent_path).expect("a git on the PATH of the test");
  let git_bin = world.home.path().join("host-git/bin");
  fs::create_dir_all(&git_bin).expect("the directory of the git of the host");
  write_script(
    &git_bin.join("git"),
    &format!(
      "#!/bin/sh\ncase $1 in version|--version) echo 'git version 99.0.0'; exit 0 ;; esac\nexec '{}' \"$@\"\n",
      real.display()
    ),
  );
  let path = format!("{}:{}", git_bin.display(), agent_path.to_string_lossy());
  let agent = world.serve_with(&[("PATH", &path), ("BD_TEST_HOST_BREW", "none")]);
  record_live_upgrade(&world.repo);
  agent.start(world.spec("9-1", "command -v git"));
  eventually("the wait of the run", || events(&world, "9-1").contains(GIT_WAIT_LINE.trim_start_matches("bd-host: ")));
  thread::sleep(Duration::from_millis(500));
  remove_upgrade(&world.repo);

  let followed = agent.follow("9-1");
  assert_eq!(followed.end, Reply::Exit { code: 0 }, "{}", agent.dump("9-1"));
  assert_eq!(followed.of(Stream::Stdout), [git_bin.join("git").to_str().expect("UTF-8")]);
  assert!(followed.lines.iter().any(|(_, text)| text == GIT_WAIT_LINE), "{followed:?}");
  assert!(followed.start_line().contains(", git 99.0.0, "), "{followed:?}");
  assert!(followed.phase_ms("git") >= 500, "{followed:?}");
  let mut phases = vec!["git"];
  phases.extend(ALL_PHASES);
  assert_eq!(followed.phase_names(), phases);
}
