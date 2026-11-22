//! A slot whose add never finished (`lease_pool::lease::is_unfinished`): a get that failed or was killed after it
//! registered the slot left it free, without a name, its tree half written perhaps. No get takes it as free or clones
//! it, and the next get that adds a slot removes it (openspec/specs/cli/lease/spec.md, issue 256)

#[cfg(unix)]
use crate::lease_test::busy_in;
use crate::lease_test::{Fixture, path_of, slot_git, states};
use lease_pool::lease::{Pool, canonical_path};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

/// The free slot `path` as a get left it that failed after its registration: no name, never returned, its tree half
/// written (a tracked file missing), marked dirty by a later get whose proof holds while that file stays missing
fn leave_unfinished(fixture: &Fixture, path: &str) {
  let common_dir = canonical_path(&fixture.repo.path().join(".git"));
  Pool::new(Path::new(&common_dir))
    .update(Duration::from_secs(10), |store| {
      let slot = store.slots.iter_mut().find(|slot| slot.path == path).unwrap();
      assert!(slot.lease.is_none(), "{slot:?}");
      slot.name = String::new();
      slot.returned = None;
      slot.returned_at = None;
      slot.dirty = true;
      Ok(())
    })
    .unwrap();
  std::fs::remove_file(Path::new(path).join("a.txt")).unwrap();
}

/// Whether a step of the get output `got` names `path`
fn a_step_names(got: &Value, path: &str) -> bool {
  got["steps"].as_array().unwrap().iter().any(|step| step["step"].as_str().unwrap().contains(path))
}

/// The paths of the linked worktrees of the repository
fn worktrees(fixture: &Fixture) -> Vec<String> {
  slot_git(&fixture.repo.path().to_string_lossy(), &["worktree", "list", "--porcelain"])
    .lines()
    .filter_map(|line| line.strip_prefix("worktree "))
    .map(|path| canonical_path(Path::new(path)))
    .collect()
}

/// A get with a clean free slot takes that one and never looks into the unfinished one; the next get, with no free slot
/// left, removes it (its directory, its registration, its entry) before it adds a slot
#[test_log::test]
fn a_slot_whose_add_never_finished_is_removed_by_the_next_get_that_adds_a_slot() {
  let fixture = Fixture::new();
  let unfinished = path_of(&fixture.get(&["--holder", "unfinished"]));
  let clean = path_of(&fixture.get(&["--holder", "clean"]));
  // returned last, so a get would take it first
  fixture.give_back(&clean);
  fixture.give_back(&unfinished);
  leave_unfinished(&fixture, &unfinished);
  let status = fixture.status();
  assert_eq!(states(&status), [(unfinished.clone(), "unfinished".to_string()), (clean.clone(), "free".to_string())]);
  assert_eq!(status["slots"][0]["slot"], json!(""));

  let reused = fixture.get(&[]);
  assert_eq!((path_of(&reused), &reused["reused"]), (clean.clone(), &json!(true)), "{reused}");
  assert_eq!(reused.get("skipped"), None, "{reused}");
  assert!(!a_step_names(&reused, &unfinished), "{reused}");

  let added = fixture.get(&[]);
  assert_eq!(added["reused"], json!(false), "{added}");
  assert_eq!(added.get("skipped"), None, "{added}");
  assert_eq!(added["removed_unfinished"].as_array().unwrap().len(), 1, "{added}");
  assert_eq!(added["removed_unfinished"][0]["path"], json!(unfinished), "{added}");
  assert_eq!(added["removed_unfinished"][0].get("errors"), None, "{added}");
  assert!(a_step_names(&added, "removing 1 slot(s) whose add never finished"), "{added}");
  let path = path_of(&added);
  assert_eq!(std::fs::read_to_string(Path::new(&path).join("a.txt")).unwrap(), "a\n");
  assert_eq!(slot_git(&path, &["status", "--porcelain"]), "");
  let mut expected = vec![(clean, "ready".to_string()), (path.clone(), "ready".to_string())];
  let mut actual = states(&fixture.status());
  expected.sort();
  actual.sort();
  assert_eq!(actual, expected);
  // the new slot may take the number of the removed one; the removed tree is in the trash, not in the pool
  if path != unfinished {
    assert!(!Path::new(&unfinished).exists(), "{unfinished}");
  }
  let linked = worktrees(&fixture);
  assert!(linked.contains(&path), "{linked:?}");
  assert_eq!(linked.len(), 3, "the main worktree and the two leased slots: {linked:?}");
}

/// An unfinished slot with a process in it stays, named as busy, and is never the template of the new slot
#[cfg(unix)]
#[test_log::test]
fn an_unfinished_slot_with_a_process_in_it_stays_and_is_no_template() {
  let fixture = Fixture::new();
  let unfinished = path_of(&fixture.get(&[]));
  fixture.give_back(&unfinished);
  leave_unfinished(&fixture, &unfinished);
  let _busy = busy_in(&unfinished);

  let got = fixture.get(&[]);
  let path = path_of(&got);
  assert_ne!(path, unfinished);
  assert_eq!(got["reused"], json!(false), "{got}");
  assert_eq!(got.get("removed_unfinished"), None, "{got}");
  assert_eq!((&got["skipped"][0]["path"], &got["skipped"][0]["reason"]), (&json!(unfinished), &json!("busy")), "{got}");
  assert_ne!(got.get("cloned_from"), Some(&json!(unfinished)), "{got}");
  assert_eq!(slot_git(&path, &["status", "--porcelain"]), "");
  assert_eq!(states(&fixture.status()), [(unfinished, "unfinished".to_string()), (path, "ready".to_string())]);
}
