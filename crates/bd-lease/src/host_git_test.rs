//! `bd lease host git --ensure` end to end through the Cli, with a fake Homebrew (`BD_TEST_HOST_BREW`) and a floor of
//! the test (`BD_TEST_HOST_MIN_VERSION`): below the floor it installs git when Homebrew has none and upgrades it else,
//! refuses with the hint without Homebrew, and then the lends refuse too; the upgrade waits for the lent slots, and the
//! lends queue while it runs; a second upgrade never records over the one of a live process. Unix only: the fakes are
//! shell scripts

use crate::host_command_test::host;
use crate::host_slots_test::{context, edit_table, host_fixture, params};
use crate::lease_test::{Fixture, pool_environment};
use bd_cli::fixture_test::{assert_error, run_json};
use lease_pool::host_git::{TEST_BREW, TEST_MIN_VERSION};
use lease_pool::host_slots::{HostGitState, ensure_host_git, read_host_slots};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use sync_utils::process_scan::process_started;
use test_utils::env_vars::EnvVars;

/// A fake Homebrew in `<dir>/bin`: each call appends its arguments to `brew.log` and writes a `git` that answers
/// `git version 99.1.0` beside it
fn fake_brew(dir: &Path) -> PathBuf {
  let bin = dir.join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  let brew = bin.join("brew");
  std::fs::write(
    &brew,
    "#!/bin/sh\nhere=$(dirname \"$0\")\necho \"$@\" >> \"$here/brew.log\"\nprintf '#!/bin/sh\\necho \"git version 99.1.0\"\\n' > \"$here/git\"\nchmod +x \"$here/git\"\n",
  )
  .unwrap();
  make_executable(&brew);
  brew
}

/// A fake Homebrew (`fake_brew`) in a directory of its own, and the variables that make it the Homebrew of the host
/// below a floor of 99.0.0
fn homebrew_below_the_floor() -> (tempfile::TempDir, PathBuf, EnvVars) {
  let dir = tempfile::tempdir().unwrap();
  let brew = fake_brew(dir.path());
  let env = EnvVars::set(&[(TEST_BREW, brew.to_str().unwrap()), (TEST_MIN_VERSION, "99.0.0")]);
  (dir, brew, env)
}

fn fake_git(bin: &Path, version: &str) {
  let git = bin.join("git");
  std::fs::write(&git, format!("#!/bin/sh\necho \"git version {version}\"\n")).unwrap();
  make_executable(&git);
}

fn make_executable(path: &Path) {
  use std::os::unix::fs::PermissionsExt;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// `bd lease host <args>` with `/usr/bin:/bin` as the `PATH` of the host, as an agent of launchd has it
fn host_with_path(fixture: &Fixture, args: &[&str]) -> (i32, Value) {
  let mut environment = pool_environment(fixture.repo.path(), fixture.home.path(), fixture.global_config.path());
  environment.path = Some("/usr/bin:/bin".into());
  run_json(&environment, &[&["lease", "host"][..], args].concat())
}

fn brew_log(brew: &Path) -> String {
  std::fs::read_to_string(brew.with_file_name("brew.log")).unwrap_or_default()
}

/// Below the floor, --ensure installs git with Homebrew when its `bin` has none, then answers the git of Homebrew,
/// which the `PATH` of the runs finds before `/usr/bin`; a second call finds it ready; an old git of Homebrew is upgraded
#[test_log::test]
fn ensure_installs_or_upgrades_git_with_homebrew_below_the_floor() {
  let fixture = Fixture::new();
  let (_dir, brew, _env) = homebrew_below_the_floor();

  let (exit_code, installed) = host_with_path(&fixture, &["git", "--ensure"]);
  assert_eq!(exit_code, 0, "{installed}");
  let bin = brew.parent().unwrap().to_str().unwrap();
  assert_eq!(
    (&installed["version"], &installed["upgraded"], &installed["dir"], &installed["meets_floor"]),
    (&json!("99.1.0"), &json!(true), &json!(bin), &json!(true)),
    "{installed}"
  );
  assert_eq!(brew_log(&brew), "install git\n");
  let (_, again) = host_with_path(&fixture, &["git", "--ensure"]);
  assert_eq!(again["upgraded"], json!(false), "{again}");

  fake_git(brew.parent().unwrap(), "98.0.0");
  let (_, upgraded) = host_with_path(&fixture, &["git", "--ensure"]);
  assert_eq!((&upgraded["upgraded"], &upgraded["version"]), (&json!(true), &json!("99.1.0")), "{upgraded}");
  assert_eq!(brew_log(&brew), "install git\nupgrade git\n");
  let (_, plain) = host_with_path(&fixture, &["git"]);
  assert_eq!((&plain["upgraded"], &plain["meets_floor"]), (&json!(false), &json!(true)), "{plain}");
}

/// Without Homebrew --ensure refuses with the hint as `git_not_found`, and so does every lend until a git meets the
/// floor
#[test_log::test]
fn without_homebrew_ensure_and_the_lends_refuse_until_git_meets_the_floor() {
  let fixture = host_fixture(1);
  host(&fixture, "lease_host_fill", &["fill"]);
  let pid = std::process::id().to_string();
  {
    let _env = EnvVars::set(&[(TEST_BREW, "none"), (TEST_MIN_VERSION, "99.0.0")]);
    let (exit_code, refused) = host_with_path(&fixture, &["git", "--ensure"]);
    assert_error(&refused, exit_code, (5, "git_not_found"));
    assert!(refused["error"]["message"].as_str().unwrap().contains("99.0.0"), "{refused}");
    let (exit_code, lend) = host_with_path(&fixture, &["lend", "--run", "run-1", "--pid", &pid]);
    assert_error(&lend, exit_code, (5, "git_not_found"));
  }
  let _env = EnvVars::set(&[(TEST_BREW, "none"), (TEST_MIN_VERSION, "0.0.0")]);
  let (exit_code, ready) = host_with_path(&fixture, &["git", "--ensure"]);
  assert_eq!((exit_code, &ready["upgraded"]), (0, &json!(false)), "{ready}");
  let (_, lend) = host_with_path(&fixture, &["lend", "--run", "run-1", "--pid", &pid]);
  assert_eq!(lend["state"], json!("lent"), "{lend}");
}

/// The upgrade starts only once no slot is lent; meanwhile a new lend queues and says that git is upgrading, and gets
/// its slot after the upgrade
#[test_log::test]
fn the_upgrade_waits_for_the_lent_slots_and_the_lends_queue_while_it_runs() {
  let fixture = host_fixture(1);
  host(&fixture, "lease_host_fill", &["fill"]);
  let (_dir, brew, _env) = homebrew_below_the_floor();
  let pid = std::process::id().to_string();
  let (_, first) = host_with_path(&fixture, &["lend", "--run", "run-1", "--pid", &pid]);
  assert_eq!(first["state"], json!("lent"), "{first}");
  let table = fixture.repo.path().join(".git/branch-deck/host-slots.json");
  let upgrading = || std::fs::read_to_string(&table).is_ok_and(|text| text.contains("\"upgrading\""));

  let ensured = std::thread::scope(|scope| {
    let ensure = scope.spawn(|| host_with_path(&fixture, &["git", "--ensure"]));
    while !upgrading() {
      std::thread::yield_now();
    }
    let (_, queued) = host_with_path(&fixture, &["lend", "--run", "run-2", "--pid", &pid]);
    assert_eq!((&queued["state"], &queued["git_upgrading"]), (&json!("queued"), &json!("brew install git")), "{queued}");
    assert_eq!(brew_log(&brew), "", "no Homebrew while a slot is lent");
    host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-1"]);
    ensure.join().unwrap()
  });
  assert_eq!((ensured.0, &ensured.1["upgraded"]), (0, &json!(true)), "{}", ensured.1);
  assert!(!upgrading());
  let (_, second) = host_with_path(&fixture, &["lend", "--run", "run-2", "--pid", &pid]);
  assert_eq!(second["state"], json!("lent"), "{second}");
}

/// The upgrade of git that a live process records, `brew upgrade git` since the millisecond 1: this process
pub(crate) fn record_live_upgrade(fixture: &Fixture) {
  let pid = std::process::id();
  edit_table(fixture, |table| {
    table["git"] = json!({
      "kind": "upgrading",
      "owner": { "pid": pid, "process_started": process_started(pid) },
      "command": "brew upgrade git",
      "since_ms": 1,
    });
  });
}

/// Remove the record of the upgrade of git from the table
pub(crate) fn remove_upgrade(fixture: &Fixture) {
  edit_table(fixture, |table| {
    table.as_object_mut().unwrap().remove("git");
  });
}

/// A second --ensure leaves the upgrade of a live process as it is (also of a call of its own process) and waits for
/// it, so two never run Homebrew at once; once it is gone, it plans again and records its own
#[test_log::test]
fn a_second_ensure_never_records_over_the_upgrade_of_a_live_process() {
  let fixture = host_fixture(1);
  host(&fixture, "lease_host_fill", &["fill"]);
  let (_dir, brew, _env) = homebrew_below_the_floor();
  record_live_upgrade(&fixture);
  let context = context(&fixture);
  let params = params(&fixture, &context);
  let git_dir = fixture.repo.path().join(".git");

  let ensured = std::thread::scope(|scope| {
    let (said, heard) = std::sync::mpsc::channel::<String>();
    let params = &params;
    let ensure = scope.spawn(move || {
      let progress = move |line: &str| said.send(line.to_string()).unwrap_or_default();
      ensure_host_git(params, OsStr::new("/usr/bin:/bin"), &progress)
    });
    assert_eq!(heard.recv().unwrap(), "another upgrade runs `brew upgrade git`");
    assert!(
      matches!(read_host_slots(&git_dir).unwrap().git, Some(HostGitState::Upgrading { since_ms: 1, .. })),
      "the record of the live process stays"
    );
    assert_eq!(brew_log(&brew), "", "no Homebrew while another upgrade runs");
    remove_upgrade(&fixture);
    ensure.join().unwrap()
  });
  assert!(ensured.unwrap().upgraded);
  assert_eq!(brew_log(&brew), "install git\n");
  assert_eq!(read_host_slots(&git_dir).unwrap().git, None);
}
