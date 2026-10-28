use crate::events::NoEvents;
use crate::push::push_test::{Fixture, OFFLINE, alone, fixture, registry_on, test_host, waits_for_the_agent};
use crate::rewrite::execute_rewrite;
use domain_types::commit::CommitInfo;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, Intent, OperationResult, OperationStep, Outcome, Resource, RunningOperation};
use domain_types::repository_view::PatronusView;
use domain_types::rewrite::{CommitReword, ParkBranchLine, RewordText, RewriteIntent, RewriteResult, TableWitness, UndoKind};
use domain_types::session::{SyncReason, ViewPatch};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::regroup::RegroupOptions;
use history_rewrite::repository_lock::{RepositoryLock, RewriteOptions};
use history_rewrite::reword::{RewordRequest, reword_request_core};
use history_rewrite::squash::{SquashCommitsParams, squash_commits_core};
use pretty_assertions::assert_eq;
use repository_model::facts::{Needs, PatronusOf};
use repository_model::patronus_view::ContentMemo;
use repository_model::session::{RunReporter, SessionHost, SessionKey, Subscriber, SubscriberGone, SyncRun};
use repository_model::warm_start::{CachedSession, frame_of};
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use sync_core::sync_inputs::SyncInputs;
use tokio::sync::watch;

impl Fixture {
  /// HEAD and the stores of the table the session shows now, which a window names a rewrite on
  async fn witness(&self) -> TableWitness {
    self.registry.sessions().read(&self.key, |state, _| state.header().witness()).await.unwrap()
  }

  /// `rewrite` run by the engine, off Space
  async fn rewrite(&self, rewrite: &RewriteIntent) -> Result<Outcome, ApiError> {
    execute_rewrite(&self.registry, &self.key, rewrite.clone(), OFFLINE.hosts(), Arc::new(NoEvents)).await
  }

  /// The subjects of the commits ahead of the initial one, newest first
  fn subjects(&self) -> Vec<String> {
    self.repo.git(&["log", "--format=%s", "origin/master..HEAD"]).lines().map(str::to_owned).collect()
  }

  /// The commit ahead whose subject is `subject`
  fn commit(&self, subject: &str) -> String {
    let log = self.repo.git(&["log", "--format=%H %s", "origin/master..HEAD"]);
    log
      .lines()
      .find_map(|line| line.split_once(' ').filter(|(_, s)| *s == subject).map(|(id, _)| id.to_owned()))
      .unwrap()
  }

  /// The session synced as a window sees it after a rewrite
  async fn synced(&self) {
    self.registry.sessions().sync(&self.key, SyncReason::Manual).wait().await.unwrap();
  }
}

/// The unstack of `beta` on `witness`
fn unstack(witness: &TableWitness) -> RewriteIntent {
  RewriteIntent::Unstack {
    branch_name: "beta".to_owned(),
    witness: witness.clone(),
  }
}

/// The move of `commits` into `target` on `witness`
fn move_to(commits: &[String], target: &str, witness: &TableWitness) -> RewriteIntent {
  RewriteIntent::Move {
    commit_ids: commits.to_vec(),
    target_branch: Some(target.to_owned()),
    witness: witness.clone(),
  }
}

/// `body` with every task it waits on, within a minute
async fn within_a_minute<T>(body: Pin<Box<dyn Future<Output = T> + Send + '_>>) -> T {
  tokio::time::timeout(Duration::from_mins(1), body).await.unwrap()
}

impl Fixture {
  /// `rewrite` run by the engine beside `meanwhile`, what the test does while it waits, within a minute
  async fn rewrite_while(&self, rewrite: &RewriteIntent, meanwhile: impl Future<Output = ()> + Send) -> Result<Outcome, ApiError> {
    let (answer, ()) = within_a_minute(Box::pin(async { tokio::join!(self.rewrite(rewrite), meanwhile) })).await;
    answer
  }

  /// `answer` is of a rewrite whose wait the user stopped: `canceled`, HEAD still `head`, the operation gone
  async fn assert_stopped(&self, answer: Result<Outcome, ApiError>, head: &str) {
    assert_eq!(answer.unwrap_err().code, ErrorCode::Canceled);
    assert_eq!(self.repo.rev_parse("HEAD").unwrap(), head, "nothing was written");
    assert_eq!(self.operations().await, Vec::new());
  }
}

/// `answer` is a move of the regroup
fn assert_moved(answer: Result<Outcome, ApiError>) {
  let outcome = answer.unwrap();
  assert!(
    matches!(&outcome.result, OperationResult::Rewrite(result) if matches!(result.as_ref(), RewriteResult::Moved(_))),
    "{outcome:?}"
  );
}

/// The operations of the view once the first waits for the lock of the agent and the others are in the view
fn waits_with(count: usize) -> impl Fn(&[RunningOperation]) -> bool {
  move |operations| waits_for_the_agent(operations) && operations.len() == count
}

/// A rewrite is run by its core and ends once it asked the sync that shows it: the move of beta's commit into `gamma`
/// is one ref transaction of HEAD, its outcome the rewrite and the sync asked, what the move answered and its undo by
/// the regroup's token; the answer comes once that sync ended, with the version of its last patch, so the table has the
/// new HEAD then; the undo of that token moves HEAD back
#[tokio::test(flavor = "multi_thread")]
async fn a_rewrite_is_run_by_its_core_and_ends_once_it_asked_its_sync_with_its_undo() {
  let fixture = fixture(true).await;
  let beta = fixture.commit("(alpha/beta) Beta");
  let head = fixture.repo.rev_parse("HEAD").unwrap();
  let outcome = fixture.rewrite(&move_to(std::slice::from_ref(&beta), "gamma", &fixture.witness().await)).await.unwrap();
  assert_eq!(
    outcome.effects.iter().map(|record| (record.effect.clone(), record.status.clone())).collect::<Vec<_>>(),
    [
      (
        PlannedEffect::Rewrite {
          commits: vec![beta.clone()],
          branches: vec!["gamma".to_owned()],
        },
        EffectStatus::Done
      ),
      (PlannedEffect::Sync, EffectStatus::Done),
    ]
  );
  let OperationResult::Rewrite(result) = &outcome.result else { panic!("{outcome:?}") };
  let RewriteResult::Moved(moved) = result.as_ref() else { panic!("{result:?}") };
  assert_eq!(moved.target.as_deref(), Some("gamma"));
  assert_eq!(fixture.subjects(), ["(gamma) Beta", "(alpha) Alpha"]);
  let undo = outcome.undo.clone().unwrap();
  assert_eq!(undo.kind, UndoKind::Regroup);
  assert_eq!(fixture.operations().await, Vec::new(), "it left the view with its end");
  assert!(outcome.version_after.is_some(), "the version of the last patch of its sync: {outcome:?}");
  assert_eq!(
    fixture.registry.sessions().read(&fixture.key, |state, _| state.header().head.clone()).await,
    Some(fixture.repo.rev_parse("HEAD").unwrap()),
    "it answered once its sync showed the new HEAD"
  );

  let undone = fixture.rewrite(&RewriteIntent::Undo { token: undo }).await.unwrap();
  assert_eq!(undone.result, OperationResult::Rewrite(Box::new(RewriteResult::Undone { already_undone: false })));
  assert_eq!(fixture.repo.rev_parse("HEAD").unwrap(), head, "HEAD is back");
}

/// Three clicks of Unstack of one branch (from two windows too) while the first waits for the lock another process
/// holds are one rewrite: the second and the third join the first (its record counts them), all three get its outcome,
/// and HEAD moves once
#[tokio::test(flavor = "multi_thread")]
async fn a_triple_unstack_is_one_rewrite_with_one_outcome() {
  let fixture = fixture(true).await;
  let held = fixture.lock();
  let unstack = unstack(&fixture.witness().await);
  let release = async {
    let operations = fixture.until(|operations| waits_with(1)(operations) && operations[0].joined == 2).await;
    assert_eq!(operations[0].intent, Intent::Rewrite(Box::new(unstack.clone())));
    drop(held);
  };
  let (first, second, third, ()) = within_a_minute(Box::pin(async {
    tokio::join!(fixture.rewrite(&unstack), fixture.rewrite(&unstack), fixture.rewrite(&unstack), release)
  }))
  .await;
  let (first, second, third) = (first.unwrap(), second.unwrap(), third.unwrap());
  assert_eq!([&second.op, &third.op], [&first.op, &first.op], "one rewrite");
  assert_eq!(fixture.subjects(), ["(beta) Beta", "(alpha) Alpha"]);
  let rewrites = fixture
    .repo
    .git(&["reflog", "--format=%gs", "HEAD"])
    .lines()
    .filter(|line| line.starts_with("branch-deck:"))
    .count();
  assert_eq!(rewrites, 1, "HEAD moved once");
}

/// The operations of the view once `count` of them are in it, the first waiting for the lock of the agent and the
/// others in the queue behind the one before
async fn queued_behind_the_waiting_one(fixture: &Fixture, count: usize) -> Vec<RunningOperation> {
  let operations = fixture
    .until(|operations| {
      waits_for_the_agent(operations) && operations.len() == count && operations[1..].iter().all(|operation| matches!(operation.step, OperationStep::Queued { .. }))
    })
    .await;
  for pair in operations.windows(2) {
    assert_eq!(pair[1].step, OperationStep::Queued { behind: pair[0].id.clone() }, "{operations:?}");
  }
  operations
}

impl Fixture {
  /// `first`, then `second` once the first waits for the lock another process holds: the second waits in the queue
  /// behind the first, the lock is released, both run; their answers
  async fn one_after_another(&self, first: &RewriteIntent, second: &RewriteIntent) -> (Result<Outcome, ApiError>, Result<Outcome, ApiError>) {
    let held = self.lock();
    let release = async {
      let operations = queued_behind_the_waiting_one(self, 2).await;
      assert_eq!(operations[1].intent, Intent::Rewrite(Box::new(second.clone())), "the second waits behind the first");
      drop(held);
    };
    let second_later = async {
      self.until(waits_with(1)).await;
      self.rewrite(second).await
    };
    let (first, second, ()) = within_a_minute(Box::pin(async { tokio::join!(self.rewrite(first), second_later, release) })).await;
    (first, second)
  }
}

/// A second move clicked on the same table while the first waits for the lock waits behind it, and names the commit the
/// first moved through the new id the first's core answered: both run, the second on what the first left
#[tokio::test(flavor = "multi_thread")]
async fn a_move_queued_behind_a_move_names_its_commit_through_the_new_id_the_first_answered() {
  let fixture = fixture(true).await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let (first, second) = (
    move_to(std::slice::from_ref(&beta), "gamma", &witness),
    move_to(std::slice::from_ref(&beta), "delta", &witness),
  );
  let (first, second) = fixture.one_after_another(&first, &second).await;
  let (first, second) = (first.unwrap(), second.unwrap());
  assert_ne!(first.op, second.op);
  assert_eq!(fixture.subjects(), ["(delta) Beta", "(alpha) Alpha"], "both moves ran, the second last");
}

/// A move queued behind a move of another commit, whose core gave the queued one's commit a new id it does not name (a
/// commit on top of the moved one), finds it once the sync the first asked shows HEAD: by its author, its author time
/// and its subject without the prefix
#[tokio::test(flavor = "multi_thread")]
async fn a_move_queued_behind_a_move_of_another_commit_finds_its_commit_by_its_key_after_the_sync() {
  let fixture = fixture(true).await;
  let (alpha, beta, witness) = (fixture.commit("(alpha) Alpha"), fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let (first, second) = (move_to(&[alpha], "gamma", &witness), move_to(std::slice::from_ref(&beta), "delta", &witness));
  let (first, second) = fixture.one_after_another(&first, &second).await;
  let (first, second) = (first.unwrap(), second.unwrap());
  let OperationResult::Rewrite(moved) = &first.result else { panic!("{first:?}") };
  assert!(!moved.rewritten().contains_key(&beta), "the first names no new id of beta's commit: {moved:?}");
  let OperationResult::Rewrite(result) = &second.result else { panic!("{second:?}") };
  assert!(matches!(result.as_ref(), RewriteResult::Moved(_)), "{result:?}");
  assert_eq!(fixture.subjects(), ["(delta) Beta", "(gamma) Alpha"]);
}

/// The reword of `commit` to `message` on `witness`
fn reword(commit: &str, message: &str, witness: &TableWitness) -> RewriteIntent {
  RewriteIntent::Reword {
    rewords: vec![CommitReword {
      commit: commit.to_owned(),
      text: RewordText::Message(message.to_owned()),
    }],
    witness: witness.clone(),
  }
}

/// A reword saved in the message editor of one window and an Unstack of its branch clicked in another at once, both on
/// the table they showed: the second waits behind the first in the queue of the session and runs on what it left, so
/// both are applied, in order, without a refusal
#[tokio::test(flavor = "multi_thread")]
async fn a_reword_and_an_unstack_from_two_windows_at_once_are_both_applied_in_order() {
  let fixture = fixture(true).await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let (reworded, unstacked) = fixture.one_after_another(&reword(&beta, "(alpha/beta) Beta, reworded", &witness), &unstack(&witness)).await;
  let (reworded, unstacked) = (reworded.unwrap(), unstacked.unwrap());
  assert_ne!(reworded.op, unstacked.op);
  assert_eq!(fixture.subjects(), ["(beta) Beta, reworded", "(alpha) Alpha"]);
}

/// A reword queued behind the issue reference of its own commit names that commit through the new id the issue
/// reference answered: its key cannot find it, since the issue key changed its subject
#[tokio::test(flavor = "multi_thread")]
async fn a_reword_queued_behind_the_issue_reference_of_its_commit_names_it_through_the_new_id() {
  let fixture = fixture(true).await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let reference = RewriteIntent::AddIssueReference {
    branch_name: "alpha/beta".to_owned(),
    commits: vec![CommitInfo {
      hash: beta.clone(),
      message: "(alpha/beta) Beta".to_owned(),
    }],
    issue_reference: "GH-1".to_owned(),
    witness: witness.clone(),
  };
  let (referenced, reworded) = fixture.one_after_another(&reference, &reword(&beta, "(alpha/beta) GH-1 Beta, reworded", &witness)).await;
  let referenced = referenced.unwrap();
  let OperationResult::Rewrite(result) = &referenced.result else {
    panic!("{referenced:?}")
  };
  assert!(result.rewritten().contains_key(&beta), "{result:?}");
  reworded.unwrap();
  assert_eq!(fixture.subjects(), ["(alpha/beta) GH-1 Beta, reworded", "(alpha) Alpha"]);
}

/// An undo clicked in the toast of a park while an unstack waits for the lock waits behind it in the queue, never
/// refused, and runs after it: the store of the parked branches goes back with compare-and-swap
#[tokio::test(flavor = "multi_thread")]
async fn an_undo_during_a_rewrite_waits_behind_it_and_runs_after_it() {
  let fixture = fixture(true).await;
  let park = RewriteIntent::Park {
    branches: vec![ParkBranchLine {
      branch_name: "beta".to_owned(),
      note: "later".to_owned(),
    }],
    witness: fixture.witness().await,
  };
  let parked = fixture.rewrite(&park).await.unwrap();
  let token = parked.undo.clone().unwrap();
  assert_eq!(token.kind, UndoKind::Regroup);
  assert!(token.token.starts_with("parked:"), "{token:?}");
  fixture.synced().await;
  assert_eq!(fixture.witness().await.store_tips.parked.is_some(), true);

  let undo = RewriteIntent::Undo { token };
  let (unstacked, undone) = fixture.one_after_another(&unstack(&fixture.witness().await), &undo).await;
  unstacked.unwrap();
  assert_eq!(undone.unwrap().result, OperationResult::Rewrite(Box::new(RewriteResult::Undone { already_undone: false })));
  assert_eq!(fixture.repo.git(&["for-each-ref", "refs/branch-deck/parked"]), "", "the store is back to none");
  assert_eq!(fixture.subjects(), ["(beta) Beta", "(alpha) Alpha"]);
}

/// A push clicked while a rewrite waits for the lock waits behind it in the queue of the session (`Queued`), holding no
/// HEAD, then for the sync the rewrite asked: it pushes the branch as the rewrite left it, the copy that sync built
#[tokio::test(flavor = "multi_thread")]
async fn a_push_clicked_during_a_rewrite_waits_behind_it_and_its_sync_and_pushes_what_it_left() {
  let fixture = fixture(true).await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let held = fixture.lock();
  let reworded = reword(&beta, "(alpha/beta) Beta, reworded", &witness);
  let push = alone("beta", None);
  let release = async {
    let operations = fixture
      .until(|operations| waits_for_the_agent(operations) && operations.len() == 2 && matches!(operations[1].step, OperationStep::Queued { .. }))
      .await;
    assert_eq!(operations[1].step, OperationStep::Queued { behind: operations[0].id.clone() }, "{operations:?}");
    assert!(!operations[1].resources.contains(&Resource::Head), "it holds no HEAD: {operations:?}");
    drop(held);
  };
  let push_later = async {
    fixture.until(waits_with(1)).await;
    fixture.execute(&push, Vec::new()).await
  };
  let (rewritten, pushed, ()) = within_a_minute(Box::pin(async { tokio::join!(fixture.rewrite(&reworded), push_later, release) })).await;
  rewritten.unwrap();
  pushed.unwrap();
  let built = fixture.repo.rev_parse("test/virtual/beta").unwrap();
  let remote = fixture.repo.git(&["--git-dir", ".git/origin.git", "rev-parse", "refs/heads/test/virtual/beta"]);
  assert_eq!(remote, built, "the copy the sync after the reword built");
  let subject = fixture.repo.git(&["log", "-1", "--format=%s", &built]);
  assert!(subject.ends_with("Beta, reworded"), "{subject}");
}

/// A rewrite asked on a table whose HEAD another process moved since: when its commit is still there (a commit was
/// added on top) it finds it once the sync shows HEAD and runs; when it is gone (squashed into another) the plan
/// changed, and nothing is written
#[tokio::test(flavor = "multi_thread")]
async fn a_rewrite_on_a_moved_head_runs_while_its_commit_is_there_and_changes_the_plan_when_it_is_gone() {
  let fixture = fixture(true).await;
  let (alpha, beta, witness) = (fixture.commit("(alpha) Alpha"), fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  fixture.repo.create_commit("(alpha) Alpha 2", "alpha2.txt", "alpha 2\n");
  let outcome = fixture.rewrite(&move_to(std::slice::from_ref(&beta), "delta", &witness)).await.unwrap();
  assert!(matches!(&outcome.result, OperationResult::Rewrite(result) if matches!(result.as_ref(), RewriteResult::Moved(_))));
  assert_eq!(fixture.subjects(), ["(alpha) Alpha 2", "(delta) Beta", "(alpha) Alpha"]);

  let (alpha_2, beta, witness) = (fixture.commit("(alpha) Alpha 2"), fixture.commit("(delta) Beta"), fixture.witness().await);
  let params = SquashCommitsParams {
    repository_path: fixture.repo.path_str().to_owned(),
    commit_ids: vec![alpha, beta.clone(), alpha_2],
    message: "(alpha) Alpha all".to_owned(),
  };
  squash_commits_core(&GitCommandExecutor::new(), &params, &RewriteOptions::default()).unwrap();
  let head = fixture.repo.rev_parse("HEAD").unwrap();
  let refused = fixture.rewrite(&move_to(&[beta], "gamma", &witness)).await.unwrap_err();
  assert_eq!(refused.code, ErrorCode::PlanChanged, "{refused:?}");
  assert_eq!(fixture.repo.rev_parse("HEAD").unwrap(), head, "nothing was written");
}

/// Another process that holds the repository lock keeps the rewrite waiting as long as it lives, its step naming the
/// holder as the record of the lock names it; Stop Waiting (`Registry::cancel`) ends the wait: `canceled`, nothing
/// written, the lock still the holder's
#[tokio::test(flavor = "multi_thread")]
async fn a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told() {
  let fixture = fixture(true).await;
  let held = fixture.lock();
  let head = fixture.repo.rev_parse("HEAD").unwrap();
  let unstack = unstack(&fixture.witness().await);
  let stop = async {
    let operations = fixture.until(waits_with(1)).await;
    assert!(fixture.registry.cancel(&operations[0].id));
  };
  let refused = fixture.rewrite_while(&unstack, stop).await;
  fixture.assert_stopped(refused, &head).await;
  drop(held);
}

/// The kind of the undo of `outcome`, and whether it asked a sync
fn undo_and_sync(outcome: &Outcome) -> (Option<UndoKind>, bool) {
  let synced = outcome.effects.iter().any(|record| record.effect == PlannedEffect::Sync);
  (outcome.undo.as_ref().map(|undo| undo.kind), synced)
}

/// Unapply, Reapply and the delete of the unapplied branch are rewrites of the engine: the unapply and the reapply ask
/// the sync that shows them, each with its undo (an `unapply:` and a `reapply:` token); the delete holds only the ref it
/// drops, asks no sync and has no undo
#[tokio::test(flavor = "multi_thread")]
async fn an_unapply_a_reapply_and_the_delete_of_the_unapplied_branch_run_through_the_engine() {
  let fixture = fixture(true).await;
  let both = ["(alpha/beta) Beta", "(alpha) Alpha"];
  let unapply = async || {
    let witness = fixture.witness().await;
    let beta = fixture.commit("(alpha/beta) Beta");
    let rewrite = RewriteIntent::Unapply {
      branch_name: "beta".to_owned(),
      original_commit_ids: Some(vec![beta]),
      witness,
    };
    fixture.rewrite(&rewrite).await.unwrap()
  };
  let unapplied = unapply().await;
  assert_eq!(undo_and_sync(&unapplied), (Some(UndoKind::Unapply), true));
  assert_eq!(fixture.subjects(), ["(alpha) Alpha"]);
  let undone = fixture.rewrite(&RewriteIntent::Undo { token: unapplied.undo.unwrap() }).await.unwrap();
  assert_eq!(undone.result, OperationResult::Rewrite(Box::new(RewriteResult::Undone { already_undone: false })));
  assert_eq!(fixture.subjects(), both);

  unapply().await;
  let reapply = RewriteIntent::Reapply {
    branch_name: "beta".to_owned(),
    witness: fixture.witness().await,
  };
  let reapplied = fixture.rewrite(&reapply).await.unwrap();
  assert_eq!(undo_and_sync(&reapplied), (Some(UndoKind::Reapply), true));
  assert_eq!(fixture.subjects().len(), 2, "the commit is back on top");
  fixture.rewrite(&RewriteIntent::Undo { token: reapplied.undo.unwrap() }).await.unwrap();
  assert_eq!(fixture.subjects(), ["(alpha) Alpha"], "the undo took it out again");

  let delete = RewriteIntent::DeleteUnapplied {
    branch_name: "test/unapplied/beta".to_owned(),
    witness: fixture.witness().await,
  };
  let deleted = fixture.rewrite(&delete).await.unwrap();
  assert_eq!(deleted.result, OperationResult::Rewrite(Box::new(RewriteResult::Deleted)));
  assert_eq!(undo_and_sync(&deleted), (None, false));
  assert!(deleted.version_after.is_some(), "the version of the patch that removed its row: {deleted:?}");
  assert!(fixture.repo.rev_parse("refs/heads/test/unapplied/beta").is_err(), "the unapplied branch is gone");
}

impl Fixture {
  /// `rewrite` started while another process holds the lock; once it waits for it, that process changes HEAD by
  /// `change` (a commit on top, a squash under the lock it holds) and releases the lock; its answer
  async fn while_another_process_moves_head(&self, rewrite: &RewriteIntent, change: impl FnOnce(&Self, &RepositoryLock) + Send) -> Result<Outcome, ApiError> {
    let held = self.lock();
    let moved = async {
      self.until(waits_with(1)).await;
      change(self, &held);
      drop(held);
    };
    within_a_minute(Box::pin(async { tokio::join!(self.rewrite(rewrite), moved) })).await.0
  }
}

/// The change of another process that commits `subject` (its `file`) on top while it holds the lock
fn commit_on_top(subject: &'static str, file: &'static str) -> impl FnOnce(&Fixture, &RepositoryLock) + Send {
  move |fixture, _| {
    fixture.repo.create_commit(subject, file, "on top\n");
  }
}

/// A rewrite that waits for the lock another process holds while that process moves HEAD (`bd update`, `bd pick`, a
/// `bd` rewrite, a commit of an agent) runs on the HEAD it left: its core refuses the HEAD it did not expect, and the
/// rewrite is resolved again on HEAD and runs once more. An Unstack goes by the name of its branch, a move finds its
/// commit by its key once the sync shows HEAD; a commit the other process squashed away changes the plan, nothing
/// written
#[tokio::test(flavor = "multi_thread")]
async fn a_rewrite_waiting_for_the_lock_of_another_process_runs_on_the_head_it_left() {
  let fixture = fixture(true).await;
  let unstacked = fixture
    .while_another_process_moves_head(&unstack(&fixture.witness().await), commit_on_top("(alpha) Alpha 2", "alpha2.txt"))
    .await;
  assert!(matches!(&unstacked.unwrap().result, OperationResult::Rewrite(result) if matches!(result.as_ref(), RewriteResult::Moved(_))));
  assert_eq!(fixture.subjects(), ["(alpha) Alpha 2", "(beta) Beta", "(alpha) Alpha"]);

  fixture.synced().await;
  let (beta, witness) = (fixture.commit("(beta) Beta"), fixture.witness().await);
  let moved = fixture
    .while_another_process_moves_head(&move_to(std::slice::from_ref(&beta), "delta", &witness), commit_on_top("(alpha) Alpha 3", "alpha3.txt"))
    .await;
  moved.unwrap();
  assert_eq!(fixture.subjects(), ["(alpha) Alpha 3", "(alpha) Alpha 2", "(delta) Beta", "(alpha) Alpha"]);

  fixture.synced().await;
  let (alpha, delta, witness) = (fixture.commit("(alpha) Alpha"), fixture.commit("(delta) Beta"), fixture.witness().await);
  let squash_into_alpha = |fixture: &Fixture, held: &RepositoryLock| {
    let params = SquashCommitsParams {
      repository_path: fixture.repo.path_str().to_owned(),
      commit_ids: vec![alpha.clone(), delta.clone()],
      message: "(alpha) Alpha and Beta".to_owned(),
    };
    let options = RewriteOptions {
      held_lock: Some(held),
      ..RewriteOptions::default()
    };
    squash_commits_core(&GitCommandExecutor::new(), &params, &options).unwrap();
  };
  let refused = fixture
    .while_another_process_moves_head(&move_to(std::slice::from_ref(&delta), "gamma", &witness), squash_into_alpha)
    .await
    .unwrap_err();
  assert_eq!(refused.code, ErrorCode::PlanChanged, "{refused:?}");
  assert_eq!(fixture.subjects(), ["(alpha) Alpha 3", "(alpha) Alpha 2", "(alpha) Alpha and Beta"], "nothing was written");
}

/// The unapply of `branch` whose commits the window showed as `commits`, on `witness`
fn unapply(branch: &str, commits: &[String], witness: &TableWitness) -> RewriteIntent {
  RewriteIntent::Unapply {
    branch_name: branch.to_owned(),
    original_commit_ids: Some(commits.to_vec()),
    witness: witness.clone(),
  }
}

/// An unapply queued behind a reword of its branch's commit names that commit through the new id the reword answered
/// and waits for the sync that builds its copy, whose virtual branch its core checks: both are applied, and the
/// unapplied branch keeps the reworded commit. A reword that took the commit out of the branch changes the plan of the
/// unapply, nothing written
#[tokio::test(flavor = "multi_thread")]
async fn an_unapply_queued_behind_a_reword_of_its_branch_unapplies_what_the_reword_left() {
  let fixture = fixture(true).await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let unapply_beta = unapply("beta", std::slice::from_ref(&beta), &witness);
  let (reworded, unapplied) = fixture.one_after_another(&reword(&beta, "(alpha/beta) Beta, reworded", &witness), &unapply_beta).await;
  reworded.unwrap();
  assert_eq!(undo_and_sync(&unapplied.unwrap()), (Some(UndoKind::Unapply), true));
  assert_eq!(fixture.subjects(), ["(alpha) Alpha"]);
  let kept = fixture.repo.git(&["log", "-1", "--format=%s", "refs/heads/test/unapplied/beta"]);
  assert!(kept.ends_with("Beta, reworded"), "{kept}");

  let fixture = self::fixture(true).await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let unapply_beta = unapply("beta", std::slice::from_ref(&beta), &witness);
  let (reworded, refused) = fixture.one_after_another(&reword(&beta, "(gamma) Beta", &witness), &unapply_beta).await;
  reworded.unwrap();
  let head = fixture.repo.rev_parse("HEAD").unwrap();
  assert_eq!(refused.unwrap_err().code, ErrorCode::PlanChanged);
  assert_eq!(fixture.repo.rev_parse("HEAD").unwrap(), head, "nothing was written");
  assert_eq!(fixture.subjects(), ["(gamma) Beta", "(alpha) Alpha"]);
}

/// An unapply and a reword waiting for the lock another process holds while that process commits on top of the
/// branches run on the HEAD it left: the reword finds its commit by its key once the sync shows that HEAD, the unapply
/// takes the commits of its branch the same way and waits for the sync that builds their copies, which its core checks;
/// both are applied, and the unapplied branch keeps the reworded commit
#[tokio::test(flavor = "multi_thread")]
async fn an_unapply_and_a_reword_waiting_for_the_lock_of_another_process_run_on_the_head_it_left() {
  let fixture = fixture(true).await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let reworded = fixture
    .while_another_process_moves_head(&reword(&beta, "(alpha/beta) Beta, reworded", &witness), commit_on_top("(alpha) Alpha 4", "alpha4.txt"))
    .await;
  assert!(matches!(&reworded.unwrap().result, OperationResult::Rewrite(result) if matches!(result.as_ref(), RewriteResult::Reworded(_))));
  assert_eq!(fixture.subjects(), ["(alpha) Alpha 4", "(alpha/beta) Beta, reworded", "(alpha) Alpha"]);

  fixture.synced().await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta, reworded"), fixture.witness().await);
  let unapplied = fixture
    .while_another_process_moves_head(&unapply("beta", std::slice::from_ref(&beta), &witness), commit_on_top("(alpha) Alpha 5", "alpha5.txt"))
    .await;
  assert_eq!(undo_and_sync(&unapplied.unwrap()), (Some(UndoKind::Unapply), true));
  assert_eq!(fixture.subjects(), ["(alpha) Alpha 5", "(alpha) Alpha 4", "(alpha) Alpha"]);
  let kept = fixture.repo.git(&["log", "-1", "--format=%s", "refs/heads/test/unapplied/beta"]);
  assert!(kept.ends_with("Beta, reworded"), "{kept}");
}

/// A Stop Waiting that comes as the holder of the lock moves HEAD and lets the lock go, within one poll of the wait,
/// stops the rewrite: a core that took the lock refused the HEAD it did not expect, and no core runs after it, so
/// nothing is written on the HEAD the other process left (`canceled`)
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_waiting_as_the_holder_moves_head_and_lets_the_lock_go_writes_nothing() {
  let fixture = fixture(true).await;
  let held = fixture.lock();
  let unstack = unstack(&fixture.witness().await);
  let moved = async {
    let operations = fixture.until(waits_with(1)).await;
    fixture.repo.create_commit("(alpha) Alpha 2", "alpha2.txt", "on top\n");
    assert!(fixture.registry.cancel(&operations[0].id));
    drop(held);
  };
  let (refused, ()) = within_a_minute(Box::pin(async { tokio::join!(fixture.rewrite(&unstack), moved) })).await;
  assert_eq!(refused.unwrap_err().code, ErrorCode::Canceled);
  assert_eq!(fixture.subjects(), ["(alpha) Alpha 2", "(alpha/beta) Beta", "(alpha) Alpha"], "nothing was written");
}

/// A Stop Waiting that comes as the holder of the lock squashes away the commit a move names and lets the lock go is
/// `canceled` too, not `plan_changed`: the rewrite is not resolved again once it was stopped, so no sync runs for a
/// rewrite the user stopped and nothing is written
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_waiting_as_the_holder_squashes_the_named_commit_away_is_canceled_without_a_resolve() {
  let fixture = fixture(true).await;
  let (alpha, beta, witness) = (fixture.commit("(alpha) Alpha"), fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let held = fixture.lock();
  let move_beta = move_to(std::slice::from_ref(&beta), "delta", &witness);
  let squashed = async {
    let operations = fixture.until(waits_with(1)).await;
    let params = SquashCommitsParams {
      repository_path: fixture.repo.path_str().to_owned(),
      commit_ids: vec![alpha.clone(), beta.clone()],
      message: "(alpha) Alpha and Beta".to_owned(),
    };
    let options = RewriteOptions {
      held_lock: Some(&held),
      ..RewriteOptions::default()
    };
    squash_commits_core(&GitCommandExecutor::new(), &params, &options).unwrap();
    assert!(fixture.registry.cancel(&operations[0].id));
    drop(held);
  };
  let (refused, ()) = within_a_minute(Box::pin(async { tokio::join!(fixture.rewrite(&move_beta), squashed) })).await;
  assert_eq!(refused.unwrap_err().code, ErrorCode::Canceled);
  assert_eq!(fixture.subjects(), ["(alpha) Alpha and Beta"], "nothing was written");
}

/// An amend whose index cannot follow the branch it moved (a file unreadable since the amend was computed, an index
/// lock of another process past its wait) answers that failure as it is, the advice to `git reset -q` kept: HEAD moved
/// once, and the rewrite never runs again on the HEAD its own core wrote
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn an_amend_whose_index_cannot_follow_the_branch_it_moved_answers_that_and_runs_once() {
  use std::os::unix::fs::PermissionsExt;
  let fixture = fixture(true).await;
  let (beta, witness) = (fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  let file = fixture.repo.path().join("beta.txt");
  std::fs::write(&file, "beta, amended\n").unwrap();
  let amend = RewriteIntent::Amend {
    branch_name: "beta".to_owned(),
    original_commit_id: beta,
    files: vec!["beta.txt".to_owned()],
    witness,
  };
  let held = fixture.lock();
  let unreadable = async {
    // the amend was computed before the lock: the file is read again only to stage it once the branch moved
    fixture.until(waits_with(1)).await;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    assert!(std::fs::File::open(&file).is_err(), "the file is unreadable: the tests run as a user that is not root");
    drop(held);
  };
  let (failed, ()) = within_a_minute(Box::pin(async { tokio::join!(fixture.rewrite(&amend), unreadable) })).await;
  std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
  let failed = failed.unwrap_err();
  assert!(failed.message.contains("The branch was rewritten, but the index could not follow it"), "{failed:?}");
  let reflog = fixture.repo.git(&["reflog", "--format=%gs", "HEAD"]);
  let amends = reflog.lines().filter(|line| line.starts_with("branch-deck: amend")).count();
  assert_eq!(amends, 1, "HEAD moved once: {reflog}");
}

/// An amend that waits for the lock another process holds while that process rewrites the commit under the amended one
/// (`bd reword`, `bd update`) runs on the HEAD it left: its core refuses the HEAD it did not expect before it looks for
/// the amended commit, which that HEAD no longer has, and the rewrite amends the copy of the same key there
#[tokio::test(flavor = "multi_thread")]
async fn an_amend_waiting_for_the_lock_of_another_process_that_rewrites_below_its_commit_amends_the_copy() {
  let fixture = fixture(true).await;
  let (alpha, beta, witness) = (fixture.commit("(alpha) Alpha"), fixture.commit("(alpha/beta) Beta"), fixture.witness().await);
  std::fs::write(fixture.repo.path().join("beta.txt"), "beta, amended\n").unwrap();
  let amend = RewriteIntent::Amend {
    branch_name: "beta".to_owned(),
    original_commit_id: beta.clone(),
    files: vec!["beta.txt".to_owned()],
    witness,
  };
  let reword_alpha = move |fixture: &Fixture, held: &RepositoryLock| {
    let request = RewordRequest {
      repository_path: fixture.repo.path_str().to_owned(),
      rewords: vec![CommitReword {
        commit: alpha,
        text: RewordText::Message("(alpha) Alpha, reworded".to_owned()),
      }],
    };
    let options = RegroupOptions {
      held_lock: Some(held),
      ..RegroupOptions::default()
    };
    reword_request_core(&GitCommandExecutor::new(), &request, &options).unwrap();
  };
  let amended = fixture.while_another_process_moves_head(&amend, reword_alpha).await;
  assert!(
    matches!(&amended.as_ref().unwrap().result, OperationResult::Rewrite(result) if matches!(result.as_ref(), RewriteResult::Amended(_))),
    "{amended:?}"
  );
  assert_eq!(fixture.subjects(), ["(alpha/beta) Beta", "(alpha) Alpha, reworded"]);
  assert_ne!(fixture.commit("(alpha/beta) Beta"), beta, "the copy the reword left was amended");
  assert_eq!(fixture.repo.git(&["show", "HEAD:beta.txt"]), "beta, amended");
  assert_eq!(fixture.repo.git(&["status", "--porcelain"]), "", "the amended file is staged as HEAD has it");
}

/// The host of the sessions of a start after the one that saved `frame`: the worktree was synced before, so a session
/// seeds from the frame as it opens and syncs (`SyncReason::Open`); each sync waits until the test releases the syncs
struct Seeded {
  host: Arc<dyn SessionHost>,
  frame: CachedSession,
  released: watch::Receiver<bool>,
  runs: Arc<AtomicUsize>,
}

impl SessionHost for Seeded {
  fn inputs(&self, key: &SessionKey) -> Option<SyncInputs> {
    self.host.inputs(key)
  }

  fn run(&self, key: &SessionKey, reporter: RunReporter) -> SyncRun {
    self.runs.fetch_add(1, Ordering::SeqCst);
    let (run, mut released) = (self.host.run(key, reporter), self.released.clone());
    Box::pin(async move {
      let _ = released.wait_for(|released| *released).await;
      run.await
    })
  }

  fn syncs_on_focus(&self, key: &SessionKey) -> bool {
    self.host.syncs_on_focus(key)
  }

  fn derive_patronus(&self, key: &SessionKey, of: &PatronusOf<'_>, memo: &mut ContentMemo) -> Result<PatronusView, ApiError> {
    self.host.derive_patronus(key, of, memo)
  }

  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> Result<Needs, ApiError> {
    self.host.landing_needs(key, landed)
  }

  fn frame(&self, _key: &SessionKey) -> Option<CachedSession> {
    Some(self.frame.clone())
  }

  fn synced_before(&self, _key: &SessionKey) -> bool {
    true
  }
}

/// The syncs of a seeded start, held until `release`
struct HeldSyncs {
  release: watch::Sender<bool>,
  runs: Arc<AtomicUsize>,
}

impl HeldSyncs {
  fn release(&self) {
    self.release.send_replace(true);
  }

  /// The syncs the sessions ran so far
  fn runs(&self) -> usize {
    self.runs.load(Ordering::SeqCst)
  }
}

/// A window of the app, which shows the table of the session it subscribed to
struct Window;

impl Subscriber for Window {
  fn send(&self, _patch: &ViewPatch) -> Result<(), SubscriberGone> {
    Ok(())
  }
}

/// A start after the one of `fixture(true)`: the frame its session saved, as the object cache keeps it (through JSON),
/// then `meanwhile` while the app was closed (another process moves HEAD), then the registry of the new start and a
/// window on its session, which seeded from that frame as it opened; its syncs held. What `meanwhile` answered
async fn seeded<T>(meanwhile: impl FnOnce(&Fixture) -> T) -> (Fixture, HeldSyncs, T) {
  let earlier = fixture(true).await;
  let key = earlier.key.clone();
  let frame = earlier
    .registry
    .sessions()
    .read(&earlier.key, move |state, facts| frame_of(&key, state, facts))
    .await
    .unwrap();
  let frame: CachedSession = serde_json::from_value(serde_json::to_value(&frame).unwrap()).unwrap();
  let answered = meanwhile(&earlier);
  let Fixture { repo, registry, .. } = earlier;
  drop(registry);
  let (release, released) = watch::channel(false);
  let runs = Arc::new(AtomicUsize::new(0));
  let host = Seeded {
    host: test_host(),
    frame,
    released,
    runs: Arc::clone(&runs),
  };
  let (registry, key) = registry_on(&repo, Arc::new(host), false).await;
  let (_, view) = registry.sessions().subscribe(&key, Box::new(Window)).await.unwrap();
  assert!(view.cached, "the window shows the frame");
  (Fixture { repo, registry, key }, HeldSyncs { release, runs }, answered)
}

impl Fixture {
  /// The table of the session is the frame of an earlier start still
  async fn cached(&self) -> bool {
    self.registry.sessions().read(&self.key, |state, _| state.cached()).await.unwrap()
  }

  /// The operation of the view once it waits for the first sync of the session
  async fn until_waiting_for_sync(&self) -> RunningOperation {
    let operations = self.until(|operations| operations.len() == 1 && operations[0].step == OperationStep::WaitingForSync).await;
    operations[0].clone()
  }
}

/// Unstack pressed on the table a start seeded from the frame of the start before waits for the first sync of the
/// session, which the session started as it opened: the step `WaitingForSync`, HEAD held, nothing written; once that
/// sync committed, it runs on the table it read. One sync served both, then the one the rewrite asked
#[tokio::test(flavor = "multi_thread")]
async fn an_unstack_on_a_seeded_table_waits_for_the_first_sync_and_runs_once_it_committed() {
  let (fixture, syncs, ()) = seeded(|_| ()).await;
  let (head, witness) = (fixture.repo.rev_parse("HEAD").unwrap(), fixture.witness().await);
  assert!(fixture.cached().await, "the session seeded from the frame");
  let first_sync = async {
    let waiting = fixture.until_waiting_for_sync().await;
    assert_eq!(waiting.resources, [Resource::Head]);
    assert_eq!(fixture.repo.rev_parse("HEAD").unwrap(), head, "nothing is written while it waits");
    syncs.release();
  };
  assert_moved(fixture.rewrite_while(&unstack(&witness), first_sync).await);
  assert_eq!(fixture.subjects(), ["(beta) Beta", "(alpha) Alpha"]);
  assert!(!fixture.cached().await);
  assert_eq!(syncs.runs(), 2, "the sync of the open, which the wait joined, and the one after the rewrite");
}

/// A move pressed on a seeded table whose HEAD another process moved after the frame was saved (`bd reword` of the
/// commit under it) finds what it names once the first sync committed: the commit of the same key (`CommitKey`, of the
/// identity the frame kept), the copy the reword left
#[tokio::test(flavor = "multi_thread")]
async fn a_move_on_a_seeded_table_whose_head_moved_since_moves_the_commit_of_its_key() {
  let reword_alpha = |earlier: &Fixture| {
    let beta = earlier.commit("(alpha/beta) Beta");
    let request = RewordRequest {
      repository_path: earlier.repo.path_str().to_owned(),
      rewords: vec![CommitReword {
        commit: earlier.commit("(alpha) Alpha"),
        text: RewordText::Message("(alpha) Alpha, reworded".to_owned()),
      }],
    };
    reword_request_core(&GitCommandExecutor::new(), &request, &RegroupOptions::default()).unwrap();
    beta
  };
  let (fixture, syncs, beta) = seeded(reword_alpha).await;
  let witness = fixture.witness().await;
  assert_ne!(fixture.commit("(alpha/beta) Beta"), beta, "the frame names the commit the reword replaced");
  let first_sync = async {
    fixture.until_waiting_for_sync().await;
    syncs.release();
  };
  assert_moved(fixture.rewrite_while(&move_to(std::slice::from_ref(&beta), "gamma", &witness), first_sync).await);
  assert_eq!(fixture.subjects(), ["(gamma) Beta", "(alpha) Alpha, reworded"]);
}

/// Stop Waiting on a rewrite that waits for the first sync of a seeded session ends the wait at once: `canceled`,
/// nothing written, the operation gone; the sync goes on and commits the table, no other sync runs
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_waiting_for_the_first_sync_is_canceled_and_the_sync_goes_on() {
  let (fixture, syncs, ()) = seeded(|_| ()).await;
  let (head, witness) = (fixture.repo.rev_parse("HEAD").unwrap(), fixture.witness().await);
  let stop = async {
    let waiting = fixture.until_waiting_for_sync().await;
    assert!(fixture.registry.cancel(&waiting.id));
  };
  let refused = fixture.rewrite_while(&unstack(&witness), stop).await;
  fixture.assert_stopped(refused, &head).await;
  assert!(fixture.cached().await, "the first sync is still held");

  syncs.release();
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Open).wait().await.unwrap();
  assert!(!fixture.cached().await);
  assert_eq!(syncs.runs(), 1, "the sync the session started as it opened, joined by the open");
  assert_eq!(fixture.subjects(), ["(alpha/beta) Beta", "(alpha) Alpha"]);
}
