use crate::events::NoEvents;
use crate::push::push_test::{Fixture, OFFLINE, fixture, waits_for_the_agent};
use crate::rewrite::execute_rewrite;
use domain_types::commit::CommitInfo;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, Intent, OperationResult, OperationStep, Outcome, RunningOperation};
use domain_types::rewrite::{CommitReword, ParkBranchLine, RewordText, RewriteIntent, RewriteResult, TableWitness, UndoKind};
use domain_types::session::SyncReason;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::RewriteOptions;
use history_rewrite::squash::{SquashCommitsParams, squash_commits_core};
use pretty_assertions::assert_eq;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

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
  let (refused, ()) = within_a_minute(Box::pin(async { tokio::join!(fixture.rewrite(&unstack), stop) })).await;
  assert_eq!(refused.unwrap_err().code, ErrorCode::Canceled);
  assert_eq!(fixture.repo.rev_parse("HEAD").unwrap(), head, "nothing was written");
  assert_eq!(fixture.operations().await, Vec::new());
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
