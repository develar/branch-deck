//! The git of a host (`lease_pool::host_slots::ensure_host_git`), with a fake Homebrew (`BD_TEST_HOST_BREW`) and a
//! floor of the test (`BD_TEST_HOST_MIN_VERSION`): below the floor it installs git when Homebrew has none and upgrades
//! it else, refuses with the hint without Homebrew, and then the lends refuse too; the upgrade waits for the lent slots,
//! and the lends queue while it runs; a second upgrade never records over the one of a live process. Unix only: the
//! fakes are shell scripts

use crate::host_slots_test::{context, edit_table, host_fixture, params, record_live_upgrade, remove_upgrade};
use crate::lease_test::Fixture;
use bd_cli::fixture_test::gone_process;
use lease_pool::host_git::{TEST_BREW, TEST_MIN_VERSION, find_git, min_version, run_path};
use lease_pool::host_slots::{EnsuredGit, HostGitState, HostSlotRefusal, LendAnswer, LendRequest, Queued, ensure_host_git, fill, give_back_run, lend, read_host_slots};
use pretty_assertions::assert_eq;
use serde_json::json;
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

/// The `PATH` of the host, as an agent of launchd has it
const LAUNCHD_PATH: &str = "/usr/bin:/bin";

/// `ensure_host_git` with `LAUNCHD_PATH`, without its progress
fn ensure(fixture: &Fixture) -> anyhow::Result<EnsuredGit> {
  let context = context(fixture);
  ensure_host_git(&params(fixture, &context), OsStr::new(LAUNCHD_PATH), &|_| {})
}

/// The host slots of the fixture filled
fn filled(fixture: &Fixture) {
  let context = context(fixture);
  fill(&context.git, &params(fixture, &context), None).unwrap();
}

/// The lend of `run` to this process, or its refusal
fn lend_now(fixture: &Fixture, run: &str) -> anyhow::Result<LendAnswer> {
  let context = context(fixture);
  let pid = std::process::id();
  let request = LendRequest {
    run,
    pid,
    process_started: process_started(pid),
    affinity: None,
  };
  lend(&params(fixture, &context), &request)
}

/// Whether `error` is the refusal `OldGit` whose words name `text`
fn is_old_git(error: &anyhow::Error, text: &str) -> bool {
  matches!(error.downcast_ref::<HostSlotRefusal>(), Some(HostSlotRefusal::OldGit(message)) if message.contains(text))
}

fn brew_log(brew: &Path) -> String {
  std::fs::read_to_string(brew.with_file_name("brew.log")).unwrap_or_default()
}

/// Below the floor, the ensure installs git with Homebrew when its `bin` has none, then answers the git of Homebrew,
/// which the `PATH` of the runs finds before `/usr/bin`; a second call finds it ready; an old git of Homebrew is upgraded
#[test_log::test]
fn ensure_installs_or_upgrades_git_with_homebrew_below_the_floor() {
  let fixture = Fixture::new();
  let (_dir, brew, _env) = homebrew_below_the_floor();

  let installed = ensure(&fixture).unwrap();
  let floor = min_version();
  assert_eq!(
    (
      installed.found.version.map(|version| version.to_string()),
      installed.upgraded,
      installed.found.path.parent()
    ),
    (Some("99.1.0".to_string()), true, brew.parent())
  );
  assert!(installed.found.is_new_enough(floor));
  assert_eq!(brew_log(&brew), "install git\n");
  assert!(!ensure(&fixture).unwrap().upgraded);

  fake_git(brew.parent().unwrap(), "98.0.0");
  let upgraded = ensure(&fixture).unwrap();
  assert_eq!(
    (upgraded.upgraded, upgraded.found.version.map(|version| version.to_string())),
    (true, Some("99.1.0".to_string()))
  );
  assert_eq!(brew_log(&brew), "install git\nupgrade git\n");
  let found = find_git(&run_path(OsStr::new(LAUNCHD_PATH)).0).unwrap();
  assert!(found.is_new_enough(floor), "{found:?}");
}

/// Without Homebrew the ensure refuses with the hint as `OldGit`, and so does every lend until a git meets the floor
#[test_log::test]
fn without_homebrew_ensure_and_the_lends_refuse_until_git_meets_the_floor() {
  let fixture = host_fixture(1);
  filled(&fixture);
  {
    let _env = EnvVars::set(&[(TEST_BREW, "none"), (TEST_MIN_VERSION, "99.0.0")]);
    let refused = ensure(&fixture).unwrap_err();
    assert!(is_old_git(&refused, "99.0.0"), "{refused:#}");
    let lend = lend_now(&fixture, "run-1").unwrap_err();
    assert!(is_old_git(&lend, ""), "{lend:#}");
  }
  let _env = EnvVars::set(&[(TEST_BREW, "none"), (TEST_MIN_VERSION, "0.0.0")]);
  assert!(!ensure(&fixture).unwrap().upgraded);
  assert!(matches!(lend_now(&fixture, "run-1").unwrap(), LendAnswer::Lent(_)));
}

/// The upgrade starts only once no slot is lent; meanwhile a new lend queues and says that git is upgrading, and gets
/// its slot after the upgrade
#[test_log::test]
fn the_upgrade_waits_for_the_lent_slots_and_the_lends_queue_while_it_runs() {
  let fixture = host_fixture(1);
  filled(&fixture);
  let (_dir, brew, _env) = homebrew_below_the_floor();
  assert!(matches!(lend_now(&fixture, "run-1").unwrap(), LendAnswer::Lent(_)));
  let table = fixture.repo.path().join(".git/branch-deck/host-slots.json");
  let upgrading = || std::fs::read_to_string(&table).is_ok_and(|text| text.contains("\"upgrading\""));

  let ensured = std::thread::scope(|scope| {
    let ensure = scope.spawn(|| ensure(&fixture));
    while !upgrading() {
      std::thread::yield_now();
    }
    let queued = lend_now(&fixture, "run-2").unwrap();
    assert!(
      matches!(&queued, LendAnswer::Queued(Queued { git_upgrading: Some(command), .. }) if command == "brew install git"),
      "{queued:?}"
    );
    assert_eq!(brew_log(&brew), "", "no Homebrew while a slot is lent");
    let context = context(&fixture);
    give_back_run(&context.git, &params(&fixture, &context), "run-1").unwrap();
    ensure.join().unwrap()
  });
  assert!(ensured.unwrap().upgraded);
  assert!(!upgrading());
  assert!(matches!(lend_now(&fixture, "run-2").unwrap(), LendAnswer::Lent(_)));
}

/// `ensure_host_git` with `LAUNCHD_PATH`, in a thread: `while_it_waits` gets its first progress line,
/// said while it waits, and ends the wait
fn ensure_while(fixture: &Fixture, while_it_waits: impl FnOnce(&str)) -> anyhow::Result<EnsuredGit> {
  let context = context(fixture);
  let params = params(fixture, &context);
  std::thread::scope(|scope| {
    let (said, heard) = std::sync::mpsc::channel::<String>();
    let params = &params;
    let ensure = scope.spawn(move || {
      let progress = move |line: &str| said.send(line.to_string()).unwrap_or_default();
      ensure_host_git(params, OsStr::new(LAUNCHD_PATH), &progress)
    });
    while_it_waits(&heard.recv().unwrap());
    ensure.join().unwrap()
  })
}

/// A second ensure leaves the upgrade of a live process as it is (also of a call of its own process) and waits for
/// it, so two never run Homebrew at once; once it is gone, it plans again and records its own
#[test_log::test]
fn a_second_ensure_never_records_over_the_upgrade_of_a_live_process() {
  let fixture = host_fixture(1);
  filled(&fixture);
  let (_dir, brew, _env) = homebrew_below_the_floor();
  record_live_upgrade(&fixture);
  let git_dir = fixture.repo.path().join(".git");

  let ensured = ensure_while(&fixture, |line| {
    assert_eq!(line, "another upgrade runs `brew upgrade git`");
    assert!(
      matches!(read_host_slots(&git_dir).unwrap().git, Some(HostGitState::Upgrading { since_ms: 1, .. })),
      "the record of the live process stays"
    );
    assert_eq!(brew_log(&brew), "", "no Homebrew while another upgrade runs");
    remove_upgrade(&fixture);
  });
  assert!(ensured.unwrap().upgraded);
  assert_eq!(brew_log(&brew), "install git\n");
  assert_eq!(read_host_slots(&git_dir).unwrap().git, None);
}

/// Homebrew also waits for a host-object call of a live process (`users`), without a limit, and the progress names its
/// process; the call of a gone process does not count
#[test_log::test]
fn the_upgrade_waits_for_a_live_host_object_call_and_not_for_a_gone_one() {
  let fixture = host_fixture(1);
  filled(&fixture);
  let (_dir, brew, _env) = homebrew_below_the_floor();
  let pid = std::process::id();
  edit_table(&fixture, |table| table["users"] = json!([{ "pid": pid, "process_started": process_started(pid) }]));

  let ensured = ensure_while(&fixture, |line| {
    assert!(line.starts_with("`brew install git` waits for the users of git ("), "{line}");
    assert!(line.ends_with(&format!("): 1 host-object calls of the processes {pid}")), "{line}");
    assert_eq!(brew_log(&brew), "", "no Homebrew while a host-object call runs");
    let (gone, started) = gone_process();
    edit_table(&fixture, |table| table["users"] = json!([{ "pid": gone, "process_started": started }]));
  });
  assert!(ensured.unwrap().upgraded);
  assert_eq!(brew_log(&brew), "install git\n");
}
