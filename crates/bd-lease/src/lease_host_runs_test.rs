//! The runs of brd-host that a lease leaves (openspec/specs/cli/lease/spec.md, "Return: A return cancels the runs of
//! brd-host that the lease leaves"): every end of a lease runs the `brd-host` beside brd in the slot before it changes
//! the slot, and its answer names the runs it cancelled and the ones it could not. The `brd-host` here is a script beside
//! a fake binary of brd (`Environment::executable`): it writes what it saw and answers the JSON of a cancel

use crate::lease_test::{Fixture, commit_in, path_of, slot_path};
use bd_cli::fixture_test::{PREFIX, assert_covered, make_lease_idle, run_in, run_json};
use git_ops::lease_slot::{git_dir_of, host_runs_dir};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The answer of the fake `brd-host`: the run `2-2` cancelled, the run `3-3` of a host that did not answer
const ANSWER: &str = r#"{"schema":1,"cancelled":[{"run":"2-2","host":"studio","answer":"the supervisor of the run 2-2 has the cancel request"}],"not_cancelled":[{"run":"3-3","host":"laptop","code":"timed_out","message":"laptop did not answer the cancel within 5.0s"}]}"#;

/// A fake binary of brd with a `brd-host` beside it, which writes its arguments and whether the slot still has its notes
/// and its branch, then answers `ANSWER`
struct FakeHost {
  dir: tempfile::TempDir,
}

impl FakeHost {
  fn new() -> Self {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("brd"), "").unwrap();
    let program = dir.path().join("brd-host");
    let script = format!("#!/bin/sh\necho \"$@\" > \"$0.args\"\nif [ -L notes.local ]; then echo notes > \"$0.saw\"; else echo none > \"$0.saw\"; fi\necho '{ANSWER}'\n");
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    Self { dir }
  }

  /// brd of `fixture` in `cwd`, whose binary has the fake `brd-host` beside it
  fn run(&self, fixture: &Fixture, cwd: &Path, args: &[&str]) -> (i32, Value) {
    let mut environment = fixture.environment(cwd);
    environment.executable = Some(self.dir.path().join("brd"));
    run_json(&environment, args)
  }

  /// The arguments of the last run of the fake, or `None` when it never ran
  fn args(&self) -> Option<String> {
    std::fs::read_to_string(self.dir.path().join("brd-host.args")).ok()
  }

  /// Whether the slot had its notes when the fake ran
  fn saw(&self) -> String {
    std::fs::read_to_string(self.dir.path().join("brd-host.saw")).unwrap().trim().to_owned()
  }
}

/// The records of the slot at `path`: `1-1` ended, `2-2` and `3-3` without an end
fn record_runs(path: &str) -> PathBuf {
  let runs = host_runs_dir(&git_dir_of(Path::new(path)).unwrap());
  std::fs::create_dir_all(&runs).unwrap();
  for (run, host, ended) in [("1-1", "studio", true), ("2-2", "studio", false), ("3-3", "laptop", false)] {
    std::fs::write(runs.join(format!("{run}.json")), "{}").unwrap();
    std::fs::write(runs.join(format!("{run}.host")), format!("{host}\n")).unwrap();
    if ended {
      std::fs::write(runs.join(format!("{run}.end.json")), "{}").unwrap();
    }
  }
  runs
}

/// A run of an answer: its id, its host, and its answer or its code
type NamedRun = (String, String, String);

/// The runs of an answer: the cancelled ones, then the others
fn runs_of(answer: &Value) -> (Vec<NamedRun>, Vec<NamedRun>) {
  let take = |key: &str, field: &str| -> Vec<NamedRun> {
    answer[key].as_array().map_or_else(Vec::new, |runs| {
      runs
        .iter()
        .map(|run| {
          (
            run["run"].as_str().unwrap().to_owned(),
            run["host"].as_str().unwrap().to_owned(),
            run[field].as_str().unwrap().to_owned(),
          )
        })
        .collect()
    })
  };
  (take("cancelled_runs", "answer"), take("uncancelled_runs", "code"))
}

/// What the fake answers, as an end of a lease names it
fn expected() -> (Vec<NamedRun>, Vec<NamedRun>) {
  (
    vec![("2-2".to_owned(), "studio".to_owned(), "the supervisor of the run 2-2 has the cancel request".to_owned())],
    vec![("3-3".to_owned(), "laptop".to_owned(), "timed_out".to_owned())],
  )
}

/// A return of a lease with an ended run and two runs without an end runs `brd-host cancel-unfinished` in the slot
/// before it changes the slot (its notes are still in it), names the runs it cancelled and the ones it could not, and
/// frees the slot all the same; the next return of the slot, whose runs all ended, starts no `brd-host`
#[test_log::test]
fn a_return_cancels_the_runs_of_brd_host_that_the_lease_leaves_before_it_changes_the_slot() {
  let fixture = Fixture::new();
  let host = FakeHost::new();
  let got = fixture.get(&[]);
  let path = path_of(&got);
  assert!(got["notes"].is_string(), "{got}");
  let runs = record_runs(&path);

  let (exit_code, returned) = host.run(&fixture, Path::new(&path), &["lease", "return"]);
  assert_eq!(exit_code, 0, "{returned}");
  assert_eq!(runs_of(&returned), expected(), "{returned}");
  assert_eq!(host.args().as_deref(), Some("cancel-unfinished --json --max 5s\n"));
  assert_eq!(host.saw(), "notes", "the cancel runs before the return takes the notes out of the slot");
  let (_, schema) = fixture.run(&["schema", "--all"]);
  assert_covered(&schema["outputs"]["lease_return"], &schema["outputs"]["lease_return"], &returned, "lease_return");

  // the fake forgot no record; a slot whose runs all ended starts none
  for run in ["2-2", "3-3"] {
    std::fs::write(runs.join(format!("{run}.end.json")), "{}").unwrap();
  }
  std::fs::remove_file(host.dir.path().join("brd-host.args")).unwrap();
  let path = path_of(&fixture.get(&[]));
  let (exit_code, returned) = host.run(&fixture, Path::new(&path), &["lease", "return"]);
  assert_eq!(exit_code, 0, "{returned}");
  assert_eq!((returned.get("cancelled_runs"), host.args()), (None, None), "{returned}");
}

/// `brd lease pick --return` cancels the runs that the lease leaves as a return does, and names them in `returned`
#[test_log::test]
fn a_pick_with_return_cancels_the_runs_that_the_lease_leaves() {
  let fixture = Fixture::new();
  let host = FakeHost::new();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Add a", "lane-a.txt");
  record_runs(&path);
  let (exit_code, picked) = host.run(&fixture, Path::new(&path), &["--prefix", PREFIX, "lease", "pick", "--return"]);
  assert_eq!((exit_code, &picked["state"]), (0, &json!("picked")), "{picked}");
  assert_eq!(runs_of(&picked["returned"]), expected(), "{picked}");
}

/// A get that takes over an idle lease and `brd lease reclaim` cancel the runs that the old lease leaves, and name them
#[test_log::test]
fn a_takeover_and_a_reclaim_cancel_the_runs_that_the_old_lease_leaves() {
  let fixture = Fixture::new();
  let host = FakeHost::new();
  let landed = |holder: &str| {
    let path = path_of(&fixture.get(&["--holder", holder, "-b", holder]));
    commit_in(&path, &format!("({holder}) Add c"), &format!("{holder}-c.txt"));
    let (exit_code, picked) = run_in(&fixture.repo, &["pick", "--from", &path]);
    assert_eq!(exit_code, 0, "{picked}");
    record_runs(&path);
    make_lease_idle(&fixture.state_file(), &slot_path(&path));
    path
  };

  let gone = landed("gone-agent");
  let (exit_code, got) = host.run(&fixture, fixture.repo.path(), &["lease", "get", "--holder", "next-agent"]);
  assert_eq!(exit_code, 0, "{got}");
  assert_eq!(path_of(&got), gone, "{got}");
  assert_eq!(runs_of(&got["took_over"]), expected(), "{got}");
  let (_, schema) = fixture.run(&["schema", "--all"]);
  assert_covered(&schema["outputs"]["lease_get"], &schema["outputs"]["lease_get"], &got, "lease_get");

  let idle = landed("idle-agent");
  let (exit_code, reclaimed) = host.run(&fixture, fixture.repo.path(), &["lease", "reclaim", "--idle"]);
  assert_eq!(exit_code, 0, "{reclaimed}");
  assert_eq!(reclaimed["reclaimed"][0]["path"], json!(slot_path(&idle)), "{reclaimed}");
  assert_eq!(runs_of(&reclaimed["reclaimed"][0]), expected(), "{reclaimed}");
  assert_covered(&schema["outputs"]["lease_reclaim"], &schema["outputs"]["lease_reclaim"], &reclaimed, "lease_reclaim");
}
