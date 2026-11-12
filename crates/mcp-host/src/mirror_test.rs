use crate::mirror::MirrorView;
use domain_types::session::{BranchChange, BranchHead, RepositoryView, TableChanges, Version, ViewPatch};
use pretty_assertions::assert_eq;
use serde_json::json;

fn head(name: &str) -> BranchHead {
  serde_json::from_value(json!({ "name": name, "latestCommitTime": 1, "summary": "", "allCommitsHaveIssueReferences": false, "origin": "Prefix" })).unwrap()
}

fn added(name: &str) -> BranchChange {
  BranchChange {
    head: Some(head(name)),
    commits: Some(Vec::new()),
    ..BranchChange::default()
  }
}

fn patch(view: &MirrorView, seq: u32, changes: TableChanges) -> ViewPatch {
  let _ = view;
  let base = Version { epoch: 0, seq };
  ViewPatch { base, next: base.next(), changes }
}

/// The state of a mirror is the view of the session without its version: air-ide counts its own sequence
#[test]
fn the_state_is_the_view_without_its_version() {
  let state = MirrorView::new(RepositoryView::default()).state();
  assert!(state.get("version").is_none(), "{state}");
  assert!(state.get("branches").is_some() && state.get("sections").is_some() && state.get("news").is_some(), "{state}");
}

/// A renamed branch is one change: the new key set, the old one removed, the order replaced, in the order of the parts
#[test]
fn a_renamed_branch_sets_the_new_key_removes_the_old_and_replaces_the_order() {
  let mut mirror = MirrorView::new(RepositoryView::default());
  let mut first = TableChanges {
    order: Some(vec!["a".to_owned()]),
    ..TableChanges::default()
  };
  first.branches.insert("a".to_owned(), added("a"));
  mirror.apply(patch(&mirror, 0, first)).unwrap();

  let mut renamed = TableChanges {
    order: Some(vec!["b".to_owned()]),
    removed_branches: vec!["a".to_owned()],
    ..TableChanges::default()
  };
  renamed.branches.insert("b".to_owned(), added("b"));
  let ops = mirror.apply(patch(&mirror, 1, renamed)).unwrap();
  let kinds: Vec<_> = ops.iter().map(|op| (op["op"].clone(), op["part"].clone(), op.get("key").cloned())).collect();
  assert_eq!(
    kinds,
    [
      (json!("set"), json!("branches"), Some(json!("b"))),
      (json!("remove"), json!("branches"), Some(json!("a"))),
      (json!("part"), json!("order"), None),
    ]
  );
  // the value of a set key is the whole row, as the view holds it after the patch
  assert_eq!(ops[0]["value"]["head"]["name"], "b");
  assert_eq!(ops[2]["value"], json!(["b"]));
  assert_eq!(
    mirror.state()["branches"].as_object().map(|branches| branches.keys().cloned().collect::<Vec<_>>()),
    Some(vec!["b".to_owned()])
  );
}

/// A change of a section replaces the part `sections` of the view, whole
#[test]
fn a_change_of_a_section_replaces_the_sections() {
  let mut mirror = MirrorView::new(RepositoryView::default());
  let changes = TableChanges {
    unassigned: Some(Vec::new()),
    ..TableChanges::default()
  };
  let ops = mirror.apply(patch(&mirror, 0, changes)).unwrap();
  assert_eq!(ops.len(), 1);
  assert_eq!((ops[0]["op"].as_str(), ops[0]["part"].as_str()), (Some("part"), Some("sections")));
  assert!(ops[0]["value"].get("unassigned").is_some(), "{}", ops[0]);
}

/// A patch that does not start at the view is refused, and a catch-up leaves out the patches the view holds already
#[test]
fn a_patch_applies_only_to_the_view_at_its_base() {
  let mut mirror = MirrorView::new(RepositoryView::default());
  mirror.apply(patch(&mirror, 5, TableChanges::default())).unwrap_err();
  let held = TableChanges {
    order: Some(vec!["x".to_owned()]),
    ..TableChanges::default()
  };
  mirror.catch_up(patch(&mirror, 0, held.clone()));
  mirror.catch_up(patch(&mirror, 0, held));
  assert_eq!(mirror.state()["order"], json!(["x"]));
}
