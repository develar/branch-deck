use crate::regenerate::{confirmation, fix_of, row_fact};
use domain_types::conflict::RegenerateFile;
use domain_types::landing::{Confirmation, LandingFix, StackCommit, Subject};
use domain_types::next_step::RowFact;
use generated_files::generated_by::GeneratedBy;
use generated_files::names::GeneratedName;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

fn commit(id: &str, branch: &str) -> StackCommit {
  StackCommit {
    id: id.to_string(),
    subject: format!("Commit {id}"),
    branch: branch.to_string(),
  }
}

/// A lock file the words merged in the copy of `b1` of `feature`, which lacks `lacks`
fn file(lacks: Option<StackCommit>) -> RegenerateFile {
  RegenerateFile {
    commit: commit("b1", "feature"),
    path: "Cargo.lock".to_string(),
    generated_by: GeneratedBy::Name { name: GeneratedName::LockFile },
    lacks,
  }
}

/// The table of the fix: the branch of the commit the copy lacks, and what it is built on
#[test]
fn the_fix_restacks_the_root_onto_the_branch_of_the_commit_the_copy_lacks_else_starts_what_lands_both_or_groups_the_commit() {
  let parents: HashMap<&str, Vec<&str>> = HashMap::from([("core", vec![]), ("ui", vec!["feature", "base"]), ("feature", vec!["base"]), ("side", vec!["base"])]);
  let ancestors = |branch: &str| parents.get(branch).cloned().unwrap_or_default();
  let lands = ["base", "feature"];
  let cases = [
    // another branch: the root of what lands is stacked on it
    (
      Some(commit("a1", "core")),
      Some(LandingFix::Restack {
        branch: "base".to_string(),
        onto: "core".to_string(),
      }),
    ),
    // a branch built on the branch itself: a restack would close a cycle, its own Safe Push lands both
    (Some(commit("a1", "ui")), Some(LandingFix::Start { subject: Subject::branch("ui") })),
    // a branch built on the root only: the whole stack lands both
    (
      Some(commit("a1", "side")),
      Some(LandingFix::Start {
        subject: Subject::whole_stack("base"),
      }),
    ),
    // an unassigned commit is grouped into the branch that needs it
    (
      Some(commit("a1", "")),
      Some(LandingFix::MoveCommit {
        commit: commit("a1", ""),
        into: "feature".to_string(),
      }),
    ),
    // no commit is known: nothing to offer
    (None, None),
  ];
  for (lacks, expected) in cases {
    assert_eq!(fix_of(&[file(lacks.clone())], "base", &lands, ancestors), expected, "{lacks:?}");
  }
}

#[test]
fn the_cell_and_the_question_name_the_files_unless_the_branch_lands_nothing() {
  let files = vec![file(Some(commit("a1", "core")))];
  let fix = LandingFix::Restack {
    branch: "feature".to_string(),
    onto: "core".to_string(),
  };
  assert_eq!(
    row_fact(&files, Some(&fix), false, false),
    Some(RowFact::Regenerate {
      files: files.clone(),
      fix: Some(fix.clone()),
    })
  );
  assert_eq!(row_fact(&files, Some(&fix), true, false), None, "in master");
  assert_eq!(row_fact(&files, Some(&fix), false, true), None, "parked");
  assert_eq!(row_fact(&[], None, false, false), None);
  let subject = Subject::branch("feature");
  assert_eq!(
    confirmation(&subject, &files),
    Some(Confirmation::Regenerate {
      subject: subject.clone(),
      files: files.clone(),
    })
  );
  assert_eq!(confirmation(&subject, &[]), None);
}
