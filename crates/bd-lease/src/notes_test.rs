//! The notes of a lease (openspec/specs/cli/lease/spec.md, "Notes"): `notes.local` of a leased worktree links to a directory
//! of the lease beside the state of the pool, which outlives the return and expires after `branchdeck.leaseNotesKeep`

use crate::lease_test::{Fixture, path_of, slot_path};
use bd_cli::fixture_test::canonical;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const DAY: u64 = 24 * 60 * 60;

/// The directory of the notes that the answer of a get names
fn notes_of(got: &Value) -> PathBuf {
  PathBuf::from(got["notes"].as_str().unwrap_or_else(|| panic!("no notes: {got}")))
}

/// `brd lease notes <args>`, which must succeed: its notes
fn notes(fixture: &Fixture, cwd: &Path, args: &[&str]) -> Vec<Value> {
  let (exit_code, output) = fixture.run_in(cwd, &[&["lease", "notes"][..], args].concat());
  assert_eq!(exit_code, 0, "{output}");
  output["notes"].as_array().unwrap().clone()
}

/// The record of the notes of `lease_id`, beside their directory
fn record_file(fixture: &Fixture, lease_id: &str) -> PathBuf {
  fixture.repo.path().join(".git/branch-deck/notes").join(format!("{lease_id}.json"))
}

/// The get links `notes.local` of its worktree to the notes of the lease, outside the worktree, and names them; the
/// status and `brd lease notes` in the worktree name them too. The return removes the link and keeps the notes; the next
/// holder of the slot starts with an empty `notes.local` of its own, and `--holder` finds the notes of the one before,
/// returned, with their files and when they expire
#[test_log::test]
fn the_notes_of_a_lease_outlive_its_return_and_the_next_holder_of_the_slot_starts_empty() {
  let fixture = Fixture::new();
  let got = fixture.get(&["--holder", "lane-1"]);
  let (slot, lease_id) = (path_of(&got), got["lease_id"].as_str().unwrap().to_string());
  let dir = notes_of(&got);
  assert_eq!(dir, canonical(&fixture.repo.path().join(".git/branch-deck/notes").join(&lease_id)));
  assert!(got["next"].as_array().unwrap().contains(&json!(format!("brd lease notes --lease-id {lease_id}"))), "{got}");
  std::fs::create_dir_all(Path::new(&slot).join("notes.local/logs")).unwrap();
  std::fs::write(Path::new(&slot).join("notes.local/logs/gate.log"), "green\n").unwrap();
  assert_eq!(std::fs::read_to_string(dir.join("logs/gate.log")).unwrap(), "green\n");

  let status = fixture.status();
  let leased = status["slots"].as_array().unwrap().iter().find(|entry| entry["path"] == json!(slot_path(&slot))).unwrap();
  assert_eq!(leased["lease"]["notes"], json!(dir.to_string_lossy()), "{status}");
  let own = notes(&fixture, Path::new(&slot), &[]);
  assert_eq!(
    (&own[0]["lease_id"], &own[0]["held"], &own[0]["expires_at"]),
    (&json!(lease_id), &json!(true), &Value::Null)
  );

  let returned = fixture.give_back(&slot);
  assert_eq!(returned["notes"], json!(dir.to_string_lossy()), "{returned}");
  assert!(
    Path::new(&slot).join("notes.local").symlink_metadata().is_err(),
    "the return takes notes.local out of the worktree"
  );
  assert_eq!(std::fs::read_to_string(dir.join("logs/gate.log")).unwrap(), "green\n");

  let next = fixture.get(&["--holder", "lane-2"]);
  assert_eq!(path_of(&next), slot, "the slot is leased again");
  assert_ne!(notes_of(&next), dir);
  assert!(
    std::fs::read_dir(Path::new(&slot).join("notes.local")).unwrap().next().is_none(),
    "the next holder starts empty"
  );

  let before = notes(&fixture, fixture.repo.path(), &["--holder", "lane-1", "--list"]);
  assert_eq!(before.len(), 1, "{before:?}");
  let returned_at = before[0]["returned_at"].as_u64().unwrap();
  assert_eq!(
    (&before[0]["held"], &before[0]["expires_at"], &before[0]["files"]),
    (&json!(false), &json!(returned_at + 14 * DAY), &json!([{ "path": "logs/gate.log", "bytes": 6 }]))
  );
  assert!(before[0]["bytes"].as_u64().unwrap() >= 6, "{before:?}");
}

/// The notes of a returned lease expire `branchdeck.leaseNotesKeep` after its return (14 days by default) through
/// `brd lease trim`, never those of a held lease; `0` keeps them; `--plan` only names them
#[test_log::test]
fn the_notes_of_a_returned_lease_expire_through_the_trim_and_never_while_held() {
  let fixture = Fixture::new();
  let old = fixture.get(&["--holder", "old"]);
  let held = fixture.get(&["--holder", "held"]);
  let (old_id, held_id) = (old["lease_id"].as_str().unwrap().to_string(), held["lease_id"].as_str().unwrap().to_string());
  std::fs::write(notes_of(&old).join("old.log"), "old\n").unwrap();
  fixture.give_back(&path_of(&old));
  // returned 15 days ago
  let file = record_file(&fixture, &old_id);
  let mut record: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
  let returned_at = record["returned_at"].as_u64().unwrap() - 15 * DAY;
  record["returned_at"] = json!(returned_at);
  std::fs::write(&file, serde_json::to_vec(&record).unwrap()).unwrap();
  // and the held lease got 15 days ago: held, it never expires
  let held_file = record_file(&fixture, &held_id);
  let mut held_record: Value = serde_json::from_slice(&std::fs::read(&held_file).unwrap()).unwrap();
  held_record["leased_at"] = json!(held_record["leased_at"].as_u64().unwrap() - 15 * DAY);
  std::fs::write(&held_file, serde_json::to_vec(&held_record).unwrap()).unwrap();
  let trim = |args: &[&str]| {
    let (exit_code, output) = fixture.run(&[&["lease", "trim"][..], args].concat());
    assert_eq!(exit_code, 0, "{output}");
    let pool = &output["pools"][0];
    assert_eq!(pool["git_common_dir"], json!(canonical(&fixture.repo.path().join(".git")).to_string_lossy()), "{output}");
    pool["expired_notes"].clone()
  };

  fixture.repo.set_config("branchdeck.leaseNotesKeep", "0").unwrap();
  assert_eq!(trim(&[]), Value::Null, "0 keeps every note");
  fixture.repo.set_config("branchdeck.leaseNotesKeep", "30d").unwrap();
  assert_eq!(trim(&[]), Value::Null, "not 30 days yet");
  git_unset(&fixture, "branchdeck.leaseNotesKeep");
  let planned = trim(&["--plan"]);
  assert_eq!((&planned[0]["lease_id"], &planned[0]["holder"]), (&json!(old_id), &json!("old")), "{planned}");
  assert!(notes_of(&old).is_dir(), "a plan deletes nothing");

  let expired = trim(&[]);
  assert_eq!(expired.as_array().map(Vec::len), Some(1), "{expired}");
  assert_eq!((&expired[0]["lease_id"], &expired[0]["returned_at"]), (&json!(old_id), &json!(returned_at)), "{expired}");
  assert!(expired[0]["bytes"].as_u64().unwrap() >= 4, "{expired}");
  assert!(!notes_of(&old).exists() && !file.exists());
  assert_eq!(notes(&fixture, fixture.repo.path(), &["--lease-id", &old_id]), Vec::<Value>::new());
  assert!(notes_of(&held).is_dir() && record_file(&fixture, &held_id).exists(), "the held lease keeps its notes");
}

fn git_unset(fixture: &Fixture, key: &str) {
  bd_cli::fixture_test::git(&fixture.repo, &["config", "--unset", key]);
}

/// The notes are no local change of the worktree, also where `info/exclude` lost the line that ignores them: the return
/// of a lease with notes and nothing else succeeds and keeps them
#[test_log::test]
fn the_notes_are_no_local_change_of_the_worktree() {
  let fixture = Fixture::new();
  let got = fixture.get(&["--holder", "lane-1"]);
  let slot = path_of(&got);
  let exclude = fixture.repo.path().join(".git/info/exclude");
  assert!(std::fs::read_to_string(&exclude).unwrap().lines().any(|line| line == "/notes.local"));
  std::fs::write(&exclude, "").unwrap();
  std::fs::write(Path::new(&slot).join("notes.local/probe.txt"), "probe\n").unwrap();
  let returned = fixture.give_back(&slot);
  assert_eq!(returned["discarded"], Value::Null, "{returned}");
  assert_eq!(std::fs::read_to_string(notes_of(&got).join("probe.txt")).unwrap(), "probe\n");
}

/// Where `notes.local` is a directory (a system that makes no link) and `info/exclude` lost its line, a status with
/// `status.showUntrackedFiles=all` names each file in it (`?? notes.local/probe.txt`): that is no local change either,
/// and the return moves the files into the notes of the lease
#[test_log::test]
fn a_directory_of_notes_is_no_local_change_with_every_untracked_file_shown() {
  let fixture = Fixture::new();
  fixture.repo.set_config("status.showUntrackedFiles", "all").unwrap();
  let got = fixture.get(&["--holder", "lane-1"]);
  let slot = path_of(&got);
  let notes_local = Path::new(&slot).join("notes.local");
  // a directory link of Windows goes with remove_dir, a symlink elsewhere with remove_file
  std::fs::remove_dir(&notes_local).or_else(|_| std::fs::remove_file(&notes_local)).unwrap();
  std::fs::create_dir_all(notes_local.join("probe")).unwrap();
  std::fs::write(notes_local.join("probe/probe.txt"), "probe\n").unwrap();
  std::fs::write(fixture.repo.path().join(".git/info/exclude"), "").unwrap();
  let untracked = bd_cli::fixture_test::git(&fixture.repo, &["-C", &slot, "status", "--porcelain", "--untracked-files=all"]);
  assert!(untracked.contains("?? notes.local/probe/probe.txt"), "{untracked}");

  let returned = fixture.give_back(&slot);
  assert_eq!(returned["discarded"], Value::Null, "{returned}");
  assert_eq!(std::fs::read_to_string(notes_of(&got).join("probe/probe.txt")).unwrap(), "probe\n");
  assert!(notes_local.symlink_metadata().is_err(), "the return moves notes.local out of the worktree");
}

/// `brd lease disk` counts what the notes of the pool hold
#[test_log::test]
fn the_disk_report_counts_the_notes_of_the_pool() {
  let fixture = Fixture::new();
  let got = fixture.get(&["--holder", "lane-1"]);
  std::fs::write(notes_of(&got).join("big.log"), vec![7u8; 64 * 1024]).unwrap();
  let (exit_code, output) = fixture.run(&["lease", "disk", "--fresh"]);
  assert_eq!(exit_code, 0, "{output}");
  assert!(output["pools"][0]["notes_bytes"].as_u64().unwrap() >= 64 * 1024, "{output}");
}
