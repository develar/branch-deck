//! The generated files the words merged in the copies of a sync that the tips still hold (`regenerate`): named with
//! why they are generated and the commit the copy lacks, a file a person writes never, cleared by a later commit that
//! writes the file and by a stack onto the branch the copy lacks, carried by a branch built on the one whose copy merged
//! it, and told of a stack branch.

use crate::regenerate::RegenerateFacts;
use crate::sync::{SyncOptions, sync_branches};
use domain_types::conflict::RegenerateFile;
use domain_types::landing::StackCommit;
use generated_files::generated_by::GeneratedBy;
use generated_files::names::GeneratedName;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::{TestReporter, WORD_CONFLICT_TIME as TIME, commit_files};

/// The file of the baseline, and the first line as alpha and then beta change it: beta copied onto the baseline without
/// alpha conflicts in git on the first line, and the words resolve it
const BASE: &str = "call(a, b, c)\nx\ny\nz\nend\n";
const ALPHA: &str = "call(A, b, c)\nx\ny\nz\nend\n";
const BETA: &str = "call(A, b, C)\nx\ny\nz\nend\n";

/// A repository whose baseline (`origin/master`) has `path` as `BASE`
fn repo_with(path: &str) -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit_with_timestamp(&format!("Add {path}"), path, BASE, Some(TIME));
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &repo.head()]);
  repo
}

/// A commit that adds `path`
fn add_file(repo: &TestRepo, message: &str, path: &str, time: i64) {
  std::fs::write(repo.path().join(path), "new\n").unwrap();
  repo.git(&["add", path]);
  commit_files(repo, message, &[], time);
}

async fn sync(repo: &TestRepo) -> RegenerateFacts {
  sync_branches(repo.git_executor(), repo.path_str(), "test", TestReporter::new(), SyncOptions::default())
    .await
    .unwrap()
    .regenerate
}

fn commit(id: &str, subject: &str, branch: &str) -> StackCommit {
  StackCommit {
    id: id.to_string(),
    subject: subject.to_string(),
    branch: branch.to_string(),
  }
}

fn lock_file(beta: &str, alpha: &str) -> RegenerateFile {
  RegenerateFile {
    commit: commit(beta, "Change the call again", "beta"),
    path: "Cargo.lock".to_string(),
    generated_by: GeneratedBy::Name { name: GeneratedName::LockFile },
    lacks: Some(commit(alpha, "Change the call", "alpha")),
  }
}

#[tokio::test]
async fn a_generated_file_the_words_merged_is_named_with_the_commit_its_copy_lacks_until_a_later_commit_writes_it() {
  let repo = repo_with("Cargo.lock");
  let alpha = commit_files(&repo, "(alpha) Change the call", &[("Cargo.lock", ALPHA)], TIME + 100);
  let beta = commit_files(&repo, "(beta) Change the call again", &[("Cargo.lock", BETA)], TIME + 200);
  assert_eq!(sync(&repo).await.branches, BTreeMap::from([("beta".to_string(), vec![lock_file(&beta, &alpha)])]));
  // the next sync reuses the copy by its note, which keeps the generated file
  assert_eq!(sync(&repo).await.branches, BTreeMap::from([("beta".to_string(), vec![lock_file(&beta, &alpha)])]));

  // a later commit of beta writes the file: its copy no longer holds what the words wrote
  commit_files(&repo, "(beta) Regenerate the lock", &[("Cargo.lock", &BETA.replace("end", "end2"))], TIME + 300);
  assert_eq!(sync(&repo).await, RegenerateFacts::default());
}

#[tokio::test]
async fn the_same_conflict_in_a_file_a_person_writes_needs_no_regenerate() {
  let repo = repo_with("Call.kt");
  commit_files(&repo, "(alpha) Change the call", &[("Call.kt", ALPHA)], TIME + 100);
  commit_files(&repo, "(beta) Change the call again", &[("Call.kt", BETA)], TIME + 200);
  assert_eq!(sync(&repo).await, RegenerateFacts::default());
}

#[tokio::test]
async fn a_branch_stacked_on_the_one_its_copy_lacks_has_nothing_to_regenerate() {
  let repo = repo_with("Cargo.lock");
  commit_files(&repo, "(alpha) Change the call", &[("Cargo.lock", ALPHA)], TIME + 100);
  commit_files(&repo, "(alpha/beta) Change the call again", &[("Cargo.lock", BETA)], TIME + 200);
  assert_eq!(sync(&repo).await, RegenerateFacts::default());
}

#[tokio::test]
async fn a_branch_built_on_the_one_whose_copy_merged_it_names_it_too_and_so_does_the_stack_branch() {
  let repo = repo_with("Cargo.lock");
  let alpha = commit_files(&repo, "(alpha) Change the call", &[("Cargo.lock", ALPHA)], TIME + 100);
  let beta = commit_files(&repo, "(beta) Change the call again", &[("Cargo.lock", BETA)], TIME + 200);
  add_file(&repo, "(beta/gamma) Add gamma", "gamma.txt", TIME + 300);
  add_file(&repo, "(beta/delta) Add delta", "delta.txt", TIME + 400);
  let facts = sync(&repo).await;
  let expected = vec![lock_file(&beta, &alpha)];
  assert_eq!(
    facts.branches,
    BTreeMap::from([
      ("beta".to_string(), expected.clone()),
      ("delta".to_string(), expected.clone()),
      ("gamma".to_string(), expected.clone()),
    ])
  );
  // the stack branch of beta, which forks, copies beta onto the baseline the same way
  assert_eq!(facts.stacks, BTreeMap::from([("beta".to_string(), expected)]));
}
