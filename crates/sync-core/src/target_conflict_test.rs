use crate::sync::{SyncOptions, sync_branches};
use crate::target_conflict::{TargetCheck, TargetCommit, TargetConflict, TargetConflictParams, TargetOutcome, check_branch_target, target_conflict_core};
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

const PREFIX: &str = "test";

/// An upstream with `shared.txt`, and a clone with the commit `(auth) Change shared` on it, synced: the virtual branch
/// `test/virtual/auth` is built on the target as the clone fetched it. Returns its copy of the commit
async fn synced_clone() -> (TestRepo, TestRepo, String) {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", "one\n"), ("other.txt", "one\n")]);
  clone.create_commit("(auth) Change shared", "shared.txt", "mine\n");
  sync_branches(clone.git_executor(), clone.path_str(), PREFIX, TestReporter::new(), SyncOptions::default())
    .await
    .unwrap();
  let copy = clone.rev_parse("test/virtual/auth").unwrap();
  (upstream, clone, copy)
}

/// The author time of `commit`
fn author_time(repo: &TestRepo, commit: &str) -> u32 {
  repo.git(&["log", "-1", "--format=%at", commit]).trim().parse().unwrap()
}

/// A commit of another developer on the upstream's `master`
fn land_on_target(upstream: &TestRepo, subject: &str, file: &str, content: &str) -> String {
  upstream.git(&["config", "user.name", "Alice"]);
  upstream.create_commit(subject, file, content)
}

fn params(clone: &TestRepo) -> TargetConflictParams {
  TargetConflictParams {
    repository_path: clone.path_str().to_string(),
    branch_prefix: PREFIX.to_string(),
    baseline_branch: "origin/master".to_string(),
    branch_name: "auth".to_string(),
    stack: false,
  }
}

/// The conflict of the copy `commit` of the clone with `target_commit` of the upstream
fn conflict(clone: &TestRepo, upstream: &TestRepo, commit: &str, target_commit: &str) -> TargetOutcome {
  TargetOutcome::Conflict(TargetConflict {
    target_tip: target_commit.to_string(),
    commit: TargetCommit {
      id: commit.to_string(),
      author: "Test User".to_string(),
      author_time: author_time(clone, commit),
      subject: "Change shared".to_string(),
    },
    files: vec!["shared.txt".to_string()],
    file_count: 1,
    target_commits: vec![TargetCommit {
      id: target_commit.to_string(),
      author: "Alice".to_string(),
      author_time: author_time(upstream, target_commit),
      subject: "Change shared upstream".to_string(),
    }],
  })
}

#[test_log::test(tokio::test)]
async fn a_target_that_moved_with_a_conflicting_change_after_the_last_sync_conflicts() {
  let (upstream, clone, commit) = synced_clone().await;
  land_on_target(&upstream, "Change other upstream", "other.txt", "two\n");
  let target_commit = land_on_target(&upstream, "Change shared upstream", "shared.txt", "theirs\n");
  let head = clone.head();

  let check = target_conflict_core(clone.git_executor(), &params(&clone));
  // the fetch moved the target: the commit of another branch on it is named, the one that changes other files is not
  assert_eq!(
    check,
    TargetCheck {
      baseline: "origin/master".to_string(),
      fetch_error: None,
      outcome: conflict(&clone, &upstream, &commit, &target_commit),
    }
  );
  assert_eq!(clone.rev_parse("origin/master").unwrap(), target_commit);
  // the check writes objects only: the virtual branch, HEAD and the files stay
  assert_eq!(clone.rev_parse("test/virtual/auth").unwrap(), commit);
  assert_eq!(clone.head(), head);
  assert_eq!(clone.git(&["status", "--porcelain"]), "");
}

#[test_log::test(tokio::test)]
async fn a_target_that_moved_without_a_conflict_is_clean() {
  let (upstream, clone, _) = synced_clone().await;
  let target_commit = land_on_target(&upstream, "Change other upstream", "other.txt", "two\n");

  let check = target_conflict_core(clone.git_executor(), &params(&clone));
  assert_eq!((check.fetch_error, check.outcome), (None, TargetOutcome::Clean));
  assert_eq!(clone.rev_parse("origin/master").unwrap(), target_commit);
}

#[test_log::test(tokio::test)]
async fn a_failed_fetch_checks_the_last_fetched_target_and_says_so() {
  let (upstream, clone, commit) = synced_clone().await;
  let target_commit = land_on_target(&upstream, "Change shared upstream", "shared.txt", "theirs\n");
  // another session fetched the target, then the remote went away
  clone.git(&["fetch", "--quiet", "origin", "master"]);
  clone.git(&["remote", "set-url", "origin", &format!("{}/missing", upstream.path_str())]);

  let check = target_conflict_core(clone.git_executor(), &params(&clone));
  assert!(
    check.fetch_error.as_deref().is_some_and(|error| error.contains("Failed to fetch origin/master")),
    "{check:?}"
  );
  assert_eq!(check.outcome, conflict(&clone, &upstream, &commit, &target_commit));
}

#[test_log::test(tokio::test)]
async fn a_branch_that_does_not_resolve_is_not_checked() {
  let (_upstream, clone, _) = synced_clone().await;
  let params = TargetConflictParams {
    branch_name: "missing".to_string(),
    ..params(&clone)
  };
  let check = check_branch_target(clone.git_executor(), &params, None);
  assert!(matches!(check.outcome, TargetOutcome::NotRun { .. }), "{check:?}");
}
