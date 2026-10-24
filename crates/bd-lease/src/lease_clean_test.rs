use crate::lease_test::{Fixture, path_of};
#[cfg(unix)]
use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::assert_error;
#[cfg(unix)]
use bd_cli::fixture_test::git;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use test_utils::git_in_directory::GitInDirectory;

/// Rules that clean the cargo units of `target` and delete `tests/results`, committed with the `.gitignore` that
/// ignores both, so a slot with them stays clean
fn commit_rules(fixture: &Fixture, rules: &str) {
  fixture
    .repo
    .create_commit_with_files("Add the clean rules", &[(".gitignore", "/target/\n/tests/results/\n/link\n"), (".branchdeck.toml", rules)]);
}

#[cfg(unix)]
const RULES: &str = "[lease]\nclean = [\n  { cargo = \"target\", keep = 1, age = \"1h\" },\n  { delete = \"tests/results\" },\n]\n";

/// A file of `size` bytes at `path` under `root`
fn file(root: &Path, path: &str, size: usize) {
  let path = root.join(path);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, vec![b'x'; size]).unwrap();
}

/// Two units of `git-ops` in `target/debug` of `root`: `old`, built two hours ago, and `new`
#[cfg(unix)]
fn cargo_units(root: &Path) {
  file(root, "target/debug/.cargo-lock", 0);
  for (hash, hours) in [("old", 2), ("new", 0)] {
    let unit = root.join("target/debug/build/git-ops").join(hash);
    file(&unit, "fingerprint/dep", 8192);
    let built = std::time::SystemTime::now() - std::time::Duration::from_secs(hours * 60 * 60);
    for path in [unit.join("fingerprint/dep"), unit.join("fingerprint"), unit.clone()] {
      std::fs::File::open(path).unwrap().set_modified(built).unwrap();
    }
  }
}

/// `(rule, error)` of each entry of `cleaned`
#[cfg(unix)]
fn rules_of(cleaned: &Value) -> Vec<(String, Option<String>)> {
  cleaned
    .as_array()
    .unwrap_or_else(|| panic!("no cleaned: {cleaned}"))
    .iter()
    .map(|entry| (entry["rule"].as_str().unwrap().to_string(), entry["error"].as_str().map(str::to_string)))
    .collect()
}

/// The return applies the rules of the source after its checks, before it frees the slot: the stale unit and the
/// e2e results go, the newest unit stays
#[cfg(unix)]
#[test_log::test]
fn the_return_applies_the_clean_rules_of_the_source() {
  let fixture = Fixture::new();
  commit_rules(&fixture, RULES);
  let path = path_of(&fixture.get(&[]));
  let slot = Path::new(&path);
  cargo_units(slot);
  file(slot, "tests/results/trace.zip", 4096);

  let returned = fixture.give_back(&path);
  assert_eq!(
    rules_of(&returned["cleaned"]),
    [("cargo target".to_string(), None), ("delete tests/results".to_string(), None)]
  );
  assert!(
    returned["cleaned"].as_array().unwrap().iter().all(|entry| entry["freed_bytes"].as_u64().unwrap() > 0),
    "{returned}"
  );
  assert!(!slot.join("target/debug/build/git-ops/old").exists());
  assert!(slot.join("target/debug/build/git-ops/new").exists());
  assert!(!slot.join("tests/results").exists());
  assert_eq!(fixture.status()["slots"][0]["state"], json!("free"));
}

/// `--no-clean` applies no rule on the return; `--keep` and `--age` change every cargo rule for one call, on the return
/// and on `bd lease clean`; a bad `--age` is invalid
#[cfg(unix)]
#[test_log::test]
fn no_clean_and_the_overrides_change_what_a_call_cleans() {
  let fixture = Fixture::new();
  commit_rules(&fixture, RULES);
  let path = path_of(&fixture.get(&[]));
  let slot = Path::new(&path);
  let units = slot.join("target/debug/build/git-ops");
  cargo_units(slot);
  let (exit_code, returned) = fixture.run(&["lease", "return", "--workspace", &path, "--no-clean"]);
  assert_eq!((exit_code, returned.get("cleaned")), (0, None), "{returned}");
  assert!(units.join("old").exists());

  // the file keeps 1 unit, the call keeps 2
  assert_eq!(path_of(&fixture.get(&[])), path);
  let (exit_code, returned) = fixture.run(&["lease", "return", "--workspace", &path, "--keep", "2"]);
  assert_eq!(exit_code, 0, "{returned}");
  assert_eq!(returned["cleaned"][0]["freed_bytes"], json!(0), "{returned}");
  assert!(units.join("old").exists() && units.join("new").exists());

  let (exit_code, output) = fixture.run(&["lease", "clean", "--free", "--age", "1week"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = fixture.run(&["lease", "clean", "--free", "--keep", "0", "--age", "0s"]);
  assert_eq!(exit_code, 0, "{output}");
  assert!(!units.join("old").exists() && !units.join("new").exists(), "{output}");
}

/// A rule out of the checkout, absolute, with `.git`, on a tracked path, or through a symlink is skipped and named;
/// the other rules apply, and nothing outside the checkout is touched
#[cfg(unix)]
#[test_log::test]
fn an_unsafe_rule_is_skipped_and_reported() {
  let fixture = Fixture::new();
  let outside = tempfile::tempdir().unwrap();
  file(outside.path(), "data/keep.txt", 10);
  let absolute = outside.path().join("data").to_string_lossy().into_owned();
  let rules = format!(
    "[lease]\nclean = [\n  {{ delete = \"../outside\" }},\n  {{ delete = \"{absolute}\" }},\n  {{ delete = \".git/hooks\" }},\n  {{ delete = \"a.txt\" }},\n  {{ delete = \"link/data\" }},\n  {{ delete = \"tests/results\" }},\n]\n"
  );
  commit_rules(&fixture, &rules);
  let repo = fixture.repo.path();
  std::os::unix::fs::symlink(outside.path(), repo.join("link")).unwrap();
  file(repo, "tests/results/trace.zip", 4096);
  // a test repository has no hooks (`TestRepo::new` copies no template): the rule must find one to keep
  file(repo, ".git/hooks/pre-commit", 10);

  let (exit_code, output) = fixture.run(&["lease", "clean"]);
  assert_eq!(exit_code, 0, "{output}");
  let rules = rules_of(&output["targets"][0]["cleaned"]);
  let errors: Vec<(&str, bool)> = rules.iter().map(|(rule, error)| (rule.as_str(), error.is_some())).collect();
  assert_eq!(
    errors,
    [
      ("delete ../outside", true),
      (format!("delete {absolute}").as_str(), true),
      ("delete .git/hooks", true),
      ("delete a.txt", true),
      ("delete link/data", true),
      ("delete tests/results", false),
    ]
  );
  for ((_, error), says) in rules.iter().zip(["..", "absolute", ".git", "git tracks a.txt", "symlink"]) {
    assert!(error.as_deref().unwrap().contains(says), "{error:?}");
  }
  assert!(outside.path().join("data/keep.txt").exists() && repo.join("a.txt").exists() && repo.join(".git/hooks/pre-commit").exists());
  assert!(!repo.join("tests/results").exists());
}

/// A file that does not parse never fails the return, which names the error; `bd lease clean` refuses it
#[test_log::test]
fn a_bad_file_does_not_fail_the_return() {
  let fixture = Fixture::new();
  commit_rules(&fixture, "[lease]\nclean = [{ command = \"rm -rf target\" }]\n");
  let path = path_of(&fixture.get(&[]));
  let returned = fixture.give_back(&path);
  assert!(returned["clean_error"].as_str().unwrap().contains("unknown kind"), "{returned}");
  assert_eq!(returned.get("cleaned"), None);
  let (exit_code, output) = fixture.run(&["lease", "clean"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

/// `bd lease clean --free` cleans the free slots and skips one with a process in it; a leased slot named on the command
/// line is skipped; with --dry-run nothing is deleted
#[cfg(unix)]
#[test_log::test]
fn clean_skips_a_busy_and_a_leased_slot_and_a_dry_run_deletes_nothing() {
  let fixture = Fixture::new();
  commit_rules(&fixture, RULES);
  let slots: Vec<String> = (0..3).map(|_| path_of(&fixture.get(&[]))).collect();
  for slot in &slots[..2] {
    fixture.give_back(slot);
  }
  for slot in &slots {
    file(Path::new(slot), "tests/results/trace.zip", 4096);
  }
  let mut busy = crate::lease_test::busy_in(&slots[1]);

  let (exit_code, dry_run) = fixture.run(&["lease", "clean", "--free", "--dry-run", &slots[2]]);
  assert_eq!((exit_code, &dry_run["dry_run"]), (0, &json!(true)), "{dry_run}");
  assert!(dry_run["freed_bytes"].as_u64().unwrap() > 0, "{dry_run}");
  assert!(slots.iter().all(|slot| Path::new(slot).join("tests/results").exists()), "a dry run deletes nothing");

  let (exit_code, output) = fixture.run(&["lease", "clean", "--free", &slots[2]]);
  let _ = busy.kill();
  let _ = busy.wait();
  assert_eq!(exit_code, 0, "{output}");
  let skipped: Vec<(&str, Option<&str>)> = output["targets"]
    .as_array()
    .unwrap()
    .iter()
    .map(|target| (target["path"].as_str().unwrap(), target["skipped"].as_str()))
    .collect();
  assert_eq!(skipped.len(), 3, "{output}");
  assert_eq!(skipped[0], (slots[0].as_str(), None));
  assert!(skipped[1].0 == slots[1] && skipped[1].1.unwrap().contains("sleep"), "{output}");
  assert!(skipped[2].0 == slots[2] && skipped[2].1.unwrap().starts_with("leased by"), "{output}");
  assert!(!Path::new(&slots[0]).join("tests/results").exists());
  assert!(Path::new(&slots[1]).join("tests/results").exists() && Path::new(&slots[2]).join("tests/results").exists());

  let (_, schema) = fixture.run(&["schema"]);
  let outputs = &schema["outputs"];
  assert_covered(&outputs["lease_clean"], &outputs["lease_clean"], &output, "lease_clean");
  // the return of the leased one, with what its rules did
  let returned = fixture.give_back(&slots[2]);
  assert_covered(&outputs["lease_return"], &outputs["lease_return"], &returned, "lease_return with cleaned");
  assert_eq!(git(&fixture.repo, &["status", "--porcelain"]), "");
}

/// While `bd lease clean --free` cleans a free slot, the slot is `cleaning`: a get passes it over, so the clean never
/// deletes in a slot that another agent leased meanwhile; after the clean it is free again
#[cfg(unix)]
#[test_log::test]
fn a_slot_that_clean_cleans_is_reserved_against_a_get() {
  use crate::lease_test::{held_environment, slot_path, states};
  let fixture = Fixture::new();
  commit_rules(&fixture, RULES);
  let slot = path_of(&fixture.get(&[]));
  fixture.give_back(&slot);
  file(Path::new(&slot), "tests/results/trace.zip", 4096);
  let dir = tempfile::tempdir().unwrap();
  let environment = held_environment(&fixture, fixture.repo.path(), dir.path(), "*\" ls-files \"*");
  let (other, (exit_code, output)) = std::thread::scope(|scope| {
    let clean = scope.spawn(|| bd_cli::fixture_test::run_json(&environment, &["lease", "clean", "--free"]));
    while !dir.path().join("held").exists() && !clean.is_finished() {
      std::thread::yield_now();
    }
    assert!(dir.path().join("held").exists(), "the clean reached its rules");
    assert_eq!(states(&fixture.status()), [(slot_path(&slot), "cleaning".to_string())]);
    let other = path_of(&fixture.get(&[]));
    std::fs::write(dir.path().join("go"), "").unwrap();
    (other, clean.join().unwrap())
  });
  assert_eq!(exit_code, 0, "{output}");
  assert_ne!(other, slot, "a get passes the slot that is cleaned over");
  assert_eq!(output["targets"][0]["path"], json!(slot));
  assert_eq!(output["targets"][0].get("skipped"), None, "{output}");
  assert!(!Path::new(&slot).join("tests/results").exists());
  let mut actual = states(&fixture.status());
  actual.sort();
  let mut expected = vec![(slot_path(&slot), "free".to_string()), (slot_path(&other), "ready".to_string())];
  expected.sort();
  assert_eq!(actual, expected);
}

/// A new slot cloned from the source leaves out what the `delete` rules of the source delete, but for their `except`
/// entries, a link among them as the link: a slot starts as a returned one. A tracked file under a skipped path comes
/// back with the checkout of the clone
#[cfg(target_os = "macos")]
#[test_log::test]
fn a_cloned_slot_leaves_out_what_the_delete_rules_delete() {
  let fixture = Fixture::new();
  let rules = "[lease]\nclean = [\n  { delete = \"out/dev-run\", except = [\"jar-cache\"] },\n  { delete = \"out/deploy\" },\n  { delete = \"out/bazel-bin\" },\n  { delete = \"docs\" },\n]\n";
  fixture.repo.create_commit_with_files(
    "Add the clean rules",
    &[(".gitignore", "/out/\n"), (".branchdeck.toml", rules), ("docs/guide.md", "tracked\n")],
  );
  let root = fixture.repo.path();
  for path in [
    "out/dev-run/jar-cache/j",
    "out/dev-run/idea/lib.jar",
    "out/deploy/installer.dmg",
    "out/notes/handoff.md",
    "docs/generated.html",
  ] {
    file(root, path, 4096);
  }
  std::os::unix::fs::symlink(root.join("out/deploy"), root.join("out/bazel-bin")).unwrap();
  let got = fixture.get(&[]);
  assert_eq!(got["created_by"], json!("clone"), "{got}");
  let slot = Path::new(got["path"].as_str().unwrap());
  for kept in ["out/dev-run/jar-cache/j", "out/notes/handoff.md", "docs/guide.md"] {
    assert!(slot.join(kept).exists(), "{kept}");
  }
  for left_out in ["out/dev-run/idea", "out/deploy", "out/bazel-bin", "docs/generated.html"] {
    assert!(slot.join(left_out).symlink_metadata().is_err(), "{left_out}");
  }
  assert_eq!(crate::lease_test::slot_git(got["path"].as_str().unwrap(), &["status", "--porcelain"]), "");
}

/// A return moves what a `delete` removes into the trash of the repository and deletes it after it freed the slot,
/// counting its bytes, and deletes what a return that died before its delete left there; nothing stays in the trash
#[cfg(unix)]
#[test_log::test]
fn a_return_deletes_through_the_trash_and_what_a_gone_run_left_there() {
  let fixture = Fixture::new();
  commit_rules(&fixture, RULES);
  let path = path_of(&fixture.get(&[]));
  file(Path::new(&path), "tests/results/trace.zip", 16384);
  // the trash of a return whose process is gone: no process has the pid 0 with the start time 1
  let trash = fixture.repo.path().join(".git/branch-deck/trash");
  file(&trash, "0-1-gone/0/trace.zip", 8192);
  assert!(fixture.status()["trash_bytes"].as_u64().unwrap() > 0);
  let returned = fixture.give_back(&path);
  let cleaned = returned["cleaned"].as_array().unwrap();
  let results = cleaned.iter().find(|rule| rule["rule"] == json!("delete tests/results")).unwrap();
  assert!(results["freed_bytes"].as_u64().unwrap() >= 16384, "{returned}");
  assert!(!Path::new(&path).join("tests/results").exists());
  assert_eq!(std::fs::read_dir(&trash).unwrap().count(), 0, "the trash is empty");
  assert_eq!((returned.get("trash_left"), fixture.status().get("trash_bytes")), (None, None));
}

/// A git that works in what a rule deletes (the shell of an agent left in a returned slot): macOS and Linux see it and
/// skip the slot, naming it; Windows has no process scan (`processes_checked: false`) and cleans the slot, but refuses the
/// move of the held entry into the trash: the entry stays whole, and the error of the rule names it and says that a
/// process holds it. Once the git exited, the next clean finishes on every platform
#[test_log::test]
fn a_held_entry_of_a_free_slot_stays_whole_until_its_process_exits() {
  let fixture = Fixture::new();
  commit_rules(&fixture, "[lease]\nclean = [{ delete = \"tests/results\" }]\n");
  let path = path_of(&fixture.get(&[]));
  fixture.give_back(&path);
  let slot = Path::new(&path);
  file(slot, "tests/results/run/trace.zip", 4096);
  let git = GitInDirectory::start(&slot.join("tests/results/run"));

  let (exit_code, output) = fixture.run(&["lease", "clean", "--free"]);

  let named = format!("git ({}: git hash-object --stdin-paths)", git.pid());
  assert_eq!((exit_code, &output["processes_checked"]), (0, &json!(cfg!(unix))), "{output}");
  let target = &output["targets"][0];
  if cfg!(windows) {
    let error = target["cleaned"][0]["error"].as_str().unwrap_or_default();
    assert!(target["skipped"].is_null() && error.contains("results") && error.contains("a process holds it"), "{output}");
  } else {
    assert!(target["skipped"].as_str().is_some_and(|reason| reason.contains(&named)), "{output}");
  }
  assert!(slot.join("tests/results/run/trace.zip").exists(), "the held entry is not whole: {output}");
  drop(git);
  let (exit_code, output) = fixture.run(&["lease", "clean", "--free"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["targets"][0]["cleaned"][0]["error"], json!(null), "{output}");
  assert!(!slot.join("tests/results").exists(), "{output}");
}
