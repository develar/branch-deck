use pretty_assertions::assert_eq;

use super::*;

fn step() -> Step {
  Step {
    name: "pnpm install".to_owned(),
    run: vec!["pnpm".to_owned(), "install".to_owned()],
    inputs: vec!["pnpm-lock.yaml".to_owned()],
    creates: vec!["node_modules".to_owned()],
  }
}

#[test]
fn a_prepare_step_runs_when_its_inputs_change_or_a_created_path_is_missing() {
  let slot = tempfile::tempdir().expect("a slot");
  let step = step();
  let mut prepared = BTreeMap::new();
  let first = fingerprint(&step, &[Some("a".repeat(40))]);
  assert_eq!(reason(&step, &first, &prepared, slot.path()).as_deref(), Some("node_modules is missing"));
  std::fs::create_dir_all(slot.path().join("node_modules")).expect("node_modules");
  assert_eq!(reason(&step, &first, &prepared, slot.path()).as_deref(), Some("the first run in this slot"));
  prepared.insert(key(&step), first.clone());
  assert_eq!(reason(&step, &first, &prepared, slot.path()), None);

  let changed = fingerprint(&step, &[Some("b".repeat(40))]);
  assert_ne!(changed, first);
  assert_eq!(reason(&step, &changed, &prepared, slot.path()).as_deref(), Some("pnpm-lock.yaml changed"));
  let missing = fingerprint(&step, &[None]);
  assert_ne!(missing, first);

  let other_command = Step {
    run: vec!["pnpm".to_owned(), "install".to_owned(), "--frozen-lockfile".to_owned()],
    ..step.clone()
  };
  assert_ne!(fingerprint(&other_command, &[Some("a".repeat(40))]), first);
  assert_eq!(key(&other_command), key(&step));
}

#[test]
fn the_fingerprint_does_not_join_two_words_into_one() {
  let one = Step {
    run: vec!["ab".to_owned()],
    inputs: vec![],
    ..step()
  };
  let two = Step {
    run: vec!["a".to_owned(), "b".to_owned()],
    ..one.clone()
  };
  assert_ne!(fingerprint(&one, &[]), fingerprint(&two, &[]));
}

/// The fingerprints of a slot survive a run, and a new generation of its files or another lease id starts them empty
#[test]
fn the_fingerprints_of_a_slot_are_kept_until_its_next_clean() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let mut prepared = Prepared::load(dir.path(), "lease-1", 10);
  assert!(prepared.steps.is_empty());
  prepared.steps.insert("00aa".to_owned(), "f1".to_owned());
  prepared.save().expect("saved");
  assert_eq!(Prepared::load(dir.path(), "lease-1", 10).steps, prepared.steps);
  assert!(Prepared::load(dir.path(), "lease-1", 11).steps.is_empty(), "a give-back discarded changes, or a clean ran");
  assert!(Prepared::load(dir.path(), "lease-2", 10).steps.is_empty(), "another slot");
}
