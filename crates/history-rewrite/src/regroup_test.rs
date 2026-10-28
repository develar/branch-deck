//! The lock span of the regroup family (C11 of docs/design/rust-domain-authority.md): the snapshot, the change and the
//! new objects come before the repository lock; under it only the witness of the refs the regroup read and the
//! compare-and-swap transaction, and a ref that moved after the snapshot is read once more under the lock. A git that
//! logs the state of the lock (`test_utils::lock_span`) tells which commands ran under it.

use crate::move_commits::{MoveCommitsParams, move_commits_to_branch_core};
use crate::parked::{PARKED, PARKED_DROP_MESSAGE, PARKED_REF, ParkLine, park_core, unpark_core};
use crate::regroup::{RegroupError, RegroupOptions, UndoToken, rename_branch_core, restack_branch_core, undo_regroup_core};
use crate::repository_lock::{LockWait, RewriteOptions};
use crate::reword::{RewordRequest, reword_request_core};
use crate::stack_parents::{STACK_PARENTS, STACK_PARENTS_DROP_MESSAGE, STACK_PARENTS_REF};
use crate::store_lines::LineStore;
use domain_types::rewrite::{CommitReword, RewordText};
use git_ops::commit_writer::write_store_updates;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::lock_span::LockSpan;
use test_utils::process_budget::alpha_and_beta_on_origin;
use test_utils::wrapped_git::WrappedGit;

/// The witness and the transaction: what a regroup runs under the lock
const WITNESS_AND_WRITE: [&str; 2] = ["for-each-ref", "update-ref"];

/// Three commits of `alpha` and three of `beta` ahead of `origin/master`, stacked branches on, and a git that logs the
/// state of its repository lock
fn stacked() -> (TestRepo, Vec<String>, LockSpan) {
  let (repo, commits) = alpha_and_beta_on_origin();
  repo.git(&["config", "branchdeck.stacks", "true"]);
  let span = LockSpan::new(&repo.path().join(".git"));
  (repo, commits, span)
}

/// The options of a regroup the app asks on the HEAD it shows
fn on_head(repo: &TestRepo) -> RegroupOptions<'static> {
  RegroupOptions {
    expect_head: Some(repo.head()),
    ..RegroupOptions::default()
  }
}

/// Only `under` ran under the lock, and the new objects were written before it
#[track_caller]
fn assert_span(span: &LockSpan, under: &[&str]) {
  let commands = span.commands();
  assert_eq!(span.commands_under_the_lock(), under, "{commands:#?}");
  if under.contains(&"for-each-ref") {
    assert!(span.commands_before_the_lock().iter().any(|command| command == "fast-import"), "{commands:#?}");
  }
}

fn subjects(repo: &TestRepo) -> Vec<String> {
  repo.log(&["--format=%s", "origin/master..HEAD"]).unwrap().lines().map(str::to_string).collect()
}

#[test]
fn a_move_locks_only_the_witness_and_the_write() {
  let (repo, commits, span) = stacked();
  let mut params = MoveCommitsParams::new(repo.path_str(), vec![commits[0].clone()], Some("gamma"));
  params.expect_head = Some(repo.head());
  let moved = move_commits_to_branch_core(span.executor(), &params, LockWait::default()).unwrap();
  assert_eq!(moved.commits.len(), 1);
  assert_eq!(subjects(&repo).last().map(String::as_str), Some("(gamma) Alpha 1"));
  assert_span(&span, &WITNESS_AND_WRITE);
}

#[test]
fn a_rename_locks_only_the_witness_and_the_write() {
  let (repo, _, span) = stacked();
  rename_branch_core(span.executor(), repo.path_str(), "alpha", "delta", &on_head(&repo)).unwrap();
  assert_eq!(subjects(&repo).last().map(String::as_str), Some("(delta) Alpha 1"));
  assert_span(&span, &WITNESS_AND_WRITE);
}

#[test]
fn a_reword_locks_only_the_witness_and_the_write() {
  let (repo, commits, span) = stacked();
  let request = RewordRequest {
    repository_path: repo.path_str().to_owned(),
    rewords: vec![CommitReword {
      commit: commits[0].clone(),
      text: RewordText::Subject("(alpha) The first".to_owned()),
    }],
  };
  reword_request_core(span.executor(), &request, &on_head(&repo)).unwrap();
  assert_eq!(subjects(&repo).last().map(String::as_str), Some("(alpha) The first"));
  assert_span(&span, &WITNESS_AND_WRITE);
}

/// A restack and an unstack write only the stack parents: the store commit before the lock, under it the witness and
/// the transaction that verifies HEAD
#[test]
fn a_restack_locks_only_the_witness_and_the_write() {
  let (repo, _, span) = stacked();
  let head = repo.head();
  let restacked = restack_branch_core(span.executor(), repo.path_str(), "beta", Some("alpha"), &on_head(&repo)).unwrap();
  assert_eq!(restacked.stack_parents.unwrap().entries, BTreeMap::from([("beta".to_owned(), "alpha".to_owned())]));
  assert_eq!(repo.head(), head);
  assert_span(&span, &WITNESS_AND_WRITE);

  let span = LockSpan::new(&repo.path().join(".git"));
  let unstacked = restack_branch_core(span.executor(), repo.path_str(), "beta", None, &on_head(&repo)).unwrap();
  assert_eq!(unstacked.stack_parents.unwrap().entries, BTreeMap::new());
  assert_span(&span, &WITNESS_AND_WRITE);
}

/// The undo of a regroup that moved HEAD reads HEAD and the stores under the lock; the undo of the stores alone has the
/// compare-and-swap of its transaction as its witness
#[test]
fn an_undo_locks_only_the_witness_and_the_write() {
  let (repo, _, span) = stacked();
  let renamed = rename_branch_core(repo.git_executor(), repo.path_str(), "alpha", "delta", &on_head(&repo)).unwrap();
  undo_regroup_core(span.executor(), repo.path_str(), &UndoToken::of(&renamed).unwrap(), LockWait::default()).unwrap();
  assert_eq!(repo.head(), renamed.previous_head);
  assert_eq!(span.commands_under_the_lock(), WITNESS_AND_WRITE, "{:#?}", span.commands());

  let restacked = restack_branch_core(repo.git_executor(), repo.path_str(), "beta", Some("alpha"), &on_head(&repo)).unwrap();
  let span = LockSpan::new(&repo.path().join(".git"));
  undo_regroup_core(span.executor(), repo.path_str(), &UndoToken::of(&restacked).unwrap(), LockWait::default()).unwrap();
  assert_eq!(repo.rev_parse(STACK_PARENTS_REF).ok(), None);
  assert_eq!(span.commands_under_the_lock(), ["update-ref"], "{:#?}", span.commands());
}

/// A park and an unpark write the one store they read: the store commit before the lock, under it only its move, whose
/// compare-and-swap is the witness
#[test]
fn a_park_and_an_unpark_lock_only_the_write() {
  let (repo, _, span) = stacked();
  let parked = park_core(span.executor(), repo.path_str(), &[ParkLine { branch: "alpha", note: "later" }], &RewriteOptions::default()).unwrap();
  assert!(parked.new_commit.is_some());
  assert_eq!(span.commands_under_the_lock(), ["update-ref"], "{:#?}", span.commands());
  assert!(span.commands_before_the_lock().iter().any(|command| command == "fast-import"));

  let span = LockSpan::new(&repo.path().join(".git"));
  let unparked = unpark_core(span.executor(), repo.path_str(), &["alpha"], &RewriteOptions::default()).unwrap();
  assert!(unparked.new_commit.is_some());
  assert_eq!(span.commands_under_the_lock(), ["update-ref"], "{:#?}", span.commands());
}

/// `store` with the line `line` of `gone`, an integrated branch, and the commit a sync writes when it drops that line
/// (`drop_store_lines`, `drop_message`), written as an object only: `(the store's commit, the drop commit)`
fn store_with_a_drop(repo: &TestRepo, store: &LineStore, line: &str, drop_message: &str) -> (String, String) {
  let planted = write_store_updates(repo.git_executor(), repo.path_str(), &[store.update(None, line)], None, "test").unwrap();
  let drop = store_commit(repo, store, "", &planted[0], drop_message);
  (planted[0].clone(), drop)
}

/// A commit of `store` with `content` on `parent`, written as an object only, as a sync writes its drop
fn store_commit(repo: &TestRepo, store: &LineStore, content: &str, parent: &str, message: &str) -> String {
  let (git, path) = (repo.git_executor(), repo.path_str());
  let blob = git.cmd(&["hash-object", "-w", "--stdin"], path).stdin(content.as_bytes()).text().unwrap();
  let tree = git.cmd(&["mktree"], path).stdin(format!("100644 blob {blob}\t{}\n", store.file).as_bytes()).text().unwrap();
  git.execute_command(&["commit-tree", &tree, "-p", parent, "-m", message], path).unwrap()
}

/// A git that moves `store_ref` from `planted` to `drop` once, as a sync that drops the line of an integrated branch,
/// before the first command that matches `arm` (a `case` pattern)
fn dropping_once(arm: &str, store_ref: &str, (planted, drop): &(String, String)) -> WrappedGit {
  WrappedGit::new(&format!(
    "{arm}) [ -f '{{dir}}/dropped' ] || {{ git update-ref {store_ref} {drop} {planted} && touch '{{dir}}/dropped'; }};;"
  ))
}

/// No temporary ref of a discarded computation is left
#[track_caller]
fn assert_no_temporary_refs(repo: &TestRepo) {
  assert_eq!(repo.git(&["for-each-ref", "refs/branch-deck/tmp/"]), "");
}

/// A sync drops the line of an integrated branch from the stack parents after the restack read its snapshot (here
/// before the restack writes its store commit): the witness under the lock misses, and the restack is computed once
/// more under the lock on the store the sync left, so it passes and keeps the drop
#[test]
fn a_drop_commit_between_the_snapshot_and_the_transaction_is_read_once_more_under_the_lock() {
  let (repo, _) = alpha_and_beta_on_origin();
  repo.git(&["config", "branchdeck.stacks", "true"]);
  let stores = store_with_a_drop(&repo, &STACK_PARENTS, "gone\talpha\n", STACK_PARENTS_DROP_MESSAGE);
  let wrapped = dropping_once("*\" fast-import \"*", STACK_PARENTS_REF, &stores);

  let restacked = restack_branch_core(wrapped.executor(), repo.path_str(), "beta", Some("alpha"), &on_head(&repo)).unwrap();
  assert!(wrapped.marked("dropped"), "the sync dropped the line after the snapshot");
  let stack_parents = restacked.stack_parents.unwrap();
  assert_eq!(stack_parents.entries, BTreeMap::from([("beta".to_owned(), "alpha".to_owned())]));
  let store = stack_parents.new_commit.unwrap();
  assert_eq!(repo.rev_parse(STACK_PARENTS_REF).unwrap(), store);
  assert_eq!(repo.rev_parse(&format!("{store}^")).unwrap(), stores.1, "written on the drop commit of the sync");
  assert_no_temporary_refs(&repo);
}

/// A sync drops a line of the parked branches after a park read them: the compare-and-swap of the park, its witness,
/// fails, and the park is computed once more under the lock on the store the sync left
#[test]
fn a_park_over_a_drop_after_its_read_is_written_once_more_under_the_lock() {
  let (repo, _) = alpha_and_beta_on_origin();
  let stores = store_with_a_drop(&repo, &PARKED, "gone\t\n", PARKED_DROP_MESSAGE);
  let wrapped = dropping_once("*\" update-ref \"*", PARKED_REF, &stores);
  let parked = park_core(wrapped.executor(), repo.path_str(), &[ParkLine { branch: "alpha", note: "" }], &RewriteOptions::default()).unwrap();
  assert!(wrapped.marked("dropped"));
  let store = parked.new_commit.unwrap();
  assert_eq!(repo.rev_parse(&format!("{store}^")).unwrap(), stores.1, "written on the drop commit of the sync");
  assert_eq!(
    PARKED.read(repo.git_executor(), repo.path_str()).unwrap().entries,
    BTreeMap::from([("alpha".to_owned(), String::new())])
  );
  assert_no_temporary_refs(&repo);
}

/// A sync drops the line of an integrated branch after the undo of a restack checked the stack parents: the
/// compare-and-swap of the undo, its witness, fails, and the undo is checked once more under the lock: it writes the
/// store the restack found, without the dropped line
#[test]
fn an_undo_over_a_drop_after_its_checks_is_checked_once_more_under_the_lock() {
  let (repo, _) = alpha_and_beta_on_origin();
  repo.git(&["config", "branchdeck.stacks", "true"]);
  write_store_updates(repo.git_executor(), repo.path_str(), &[STACK_PARENTS.update(None, "gone\talpha\n")], None, "test").unwrap();
  let restacked = restack_branch_core(repo.git_executor(), repo.path_str(), "beta", Some("alpha"), &on_head(&repo)).unwrap();
  let written = restacked.stack_parents.as_ref().and_then(|change| change.new_commit.clone()).unwrap();
  let drop = store_commit(&repo, &STACK_PARENTS, "beta\talpha\n", &written, STACK_PARENTS_DROP_MESSAGE);
  let (git, path) = (repo.git_executor(), repo.path_str());
  let wrapped = dropping_once("*\" update-ref \"*", STACK_PARENTS_REF, &(written, drop.clone()));

  undo_regroup_core(wrapped.executor(), path, &UndoToken::of(&restacked).unwrap(), LockWait::default()).unwrap();
  assert!(wrapped.marked("dropped"));
  assert_eq!(STACK_PARENTS.read(git, path).unwrap().entries, BTreeMap::new(), "the restack undone, the dropped line gone");
  assert_eq!(repo.rev_parse(&format!("{STACK_PARENTS_REF}^")).unwrap(), drop, "written over the drop commit of the sync");
  assert_no_temporary_refs(&repo);
}

/// Another process rewrote HEAD after the regroup read its snapshot: the witness misses, and the regroup read once more
/// under the lock refuses the HEAD the app did not ask for; `bd` without `--expect-head` regroups the HEAD it finds
#[test]
fn a_head_moved_before_the_lock_is_read_once_more_under_it() {
  let (repo, commits) = alpha_and_beta_on_origin();
  let path = repo.path_str();
  let wrapped = WrappedGit::new("*\" fast-import \"*) [ -f '{dir}/committed' ] || { git commit -q --allow-empty -m '(beta) Beta 4' && touch '{dir}/committed'; };;");
  let mut params = MoveCommitsParams::new(path, vec![commits[0].clone()], Some("gamma"));
  params.expect_head = Some(repo.head());
  let error = move_commits_to_branch_core(wrapped.executor(), &params, LockWait::default()).unwrap_err();
  assert!(
    error.downcast_ref::<RegroupError>().is_some_and(|error| matches!(error, RegroupError::HeadMoved(_))),
    "{error:#}"
  );
  assert_eq!(subjects(&repo)[0], "(beta) Beta 4");
  assert_no_temporary_refs(&repo);

  let wrapped = WrappedGit::new("*\" fast-import \"*) [ -f '{dir}/committed' ] || { git commit -q --allow-empty -m '(beta) Beta 5' && touch '{dir}/committed'; };;");
  rename_branch_core(wrapped.executor(), path, "alpha", "delta", &RegroupOptions::default()).unwrap();
  assert_eq!(subjects(&repo)[..2], ["(beta) Beta 5", "(beta) Beta 4"]);
  assert_eq!(subjects(&repo).last().map(String::as_str), Some("(delta) Alpha 1"));
}
