//! A `NotIntegrated` detection note holds for the baseline it was checked against: when the baseline moves on, the
//! branch is compared with the baseline commits since then only; a note without a baseline is detected again in full,
//! within the retention period

use super::stack_tests::{land_auth_and_sync_login, stacked_clone};
use super::test_helpers::{detected_branch_info, detection_note, setup_test_repos, sync_user_branches};
use branch_integration::cache::{CacheOps, CachedDetection, Recheck};
use branch_integration::detector::{DetectConfig, detect_integrated_branches};
use branch_integration::strategy::DetectionStrategy;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus};
use sync_utils::timestamp::format_utc_date;
use test_log::test;
use test_utils::env_vars::EnvVars;
use test_utils::git_test_utils::TestRepo;

const RETENTION_DAYS: u64 = 7;

fn path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

/// The full name of the archived branch `name`, archived `days_ago`
fn archive_name(days_ago: i64, name: &str) -> String {
  let date = format_utc_date(sync_utils::timestamp::days_ago(days_ago));
  format!("user/archived/{date}/{name}")
}

fn not_integrated(total: u32, orphaned: u32) -> BranchIntegrationStatus {
  BranchIntegrationStatus::NotIntegrated {
    total_commit_count: total,
    integrated_count: total - orphaned,
    orphaned_count: orphaned,
    integrated_at: None,
  }
}

/// Writes a `NotIntegrated` note on `tip`: `total` own commits, `orphaned` of them without an equivalent; `check` is
/// what it was checked against (`None`: a note of version 1)
fn write_not_integrated(repo: &TestRepo, tip: &str, total: u32, orphaned: u32, check: Option<Recheck>) {
  let detection = CachedDetection {
    info: BranchIntegrationInfo {
      name: String::new(),
      summary: String::new(),
      status: not_integrated(total, orphaned),
    },
    check,
  };
  CacheOps::new(repo.git_executor(), path(repo)).write(tip, &detection).unwrap();
}

/// Writes the `NotIntegrated` note of a one-commit branch at `tip`, checked against `baseline` with the tip orphaned
fn write_checked_against(repo: &TestRepo, tip: &str, baseline: &str) {
  let check = Recheck {
    baseline: baseline.to_string(),
    orphaned: vec![tip.to_string()],
  };
  write_not_integrated(repo, tip, 1, 1, Some(check));
}

/// The parent `auth` of a stacked clone has a `NotIntegrated` note checked against the current baseline; then upstream
/// takes it: the baseline moved on from the note, so the parent is checked again before the placement and is landed
async fn stale_parent_lands(upstream_repo: &TestRepo, local_repo: &TestRepo, git_executor: &GitCommandExecutor, auth: &str) {
  write_checked_against(local_repo, auth, &local_repo.rev_parse("origin/master").unwrap());
  land_auth_and_sync_login(upstream_repo, local_repo, git_executor).await;
  assert!(matches!(detection_note(local_repo, auth).info.status, BranchIntegrationStatus::Integrated { .. }));
}

/// Detects the archived branches against `origin/master` as a sync does; returns the events and the cherry-mark walks
/// that ran (their trace2 start events)
fn detect(repo: &TestRepo) -> (Vec<SyncEvent>, Vec<String>) {
  let trace = repo.path().join(".git").join("trace2.json");
  let _ = std::fs::remove_file(&trace);
  let progress = TestReporter::new();
  let active_branches = HashSet::new();
  let config = DetectConfig {
    active_branches: &active_branches,
    progress: &progress,
    strategy: DetectionStrategy::Rebase,
    retention_days: RETENTION_DAYS,
  };
  let env = EnvVars::set(&[("GIT_TRACE2_EVENT", &trace)]);
  let result = detect_integrated_branches(repo.git_executor(), path(repo), "user", "origin/master", config);
  drop(env);
  result.unwrap();
  let scans = std::fs::read_to_string(&trace)
    .unwrap_or_default()
    .lines()
    .filter(|line| line.contains("\"event\":\"start\"") && line.contains("\"--cherry-mark\""))
    .map(str::to_string)
    .collect();
  (progress.get_events(), scans)
}

fn status<'a>(events: &'a [SyncEvent], branch: &str) -> &'a BranchIntegrationStatus {
  &detected_branch_info(events, branch).status
}

#[test(tokio::test)]
async fn a_not_integrated_note_is_rechecked_when_the_baseline_moves() {
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();
  local_repo.create_commit("(feature) Add feature", "feature.rs", "// feature");
  sync_user_branches(&git_executor, &local_repo, DetectionStrategy::Rebase).await.unwrap();
  let tip = local_repo.rev_parse("user/virtual/feature").unwrap();
  let archived = archive_name(0, "feature");

  // the commit leaves the current branch: the sync archives `feature`, not integrated
  local_repo.reset_hard("origin/master").unwrap();
  let events = sync_user_branches(&git_executor, &local_repo, DetectionStrategy::Rebase).await.unwrap().get_events();
  assert!(matches!(status(&events, &archived), BranchIntegrationStatus::NotIntegrated { orphaned_count: 1, .. }));
  let check = detection_note(&local_repo, &tip).check.expect("a NotIntegrated note records its baseline");
  assert_eq!(check.baseline, local_repo.rev_parse("origin/master").unwrap());

  // upstream takes the change: the first sync after the fetch finds it
  upstream_repo.create_commit("Add feature", "feature.rs", "// feature");
  local_repo.fetch_prune("origin").unwrap();
  let events = sync_user_branches(&git_executor, &local_repo, DetectionStrategy::Rebase).await.unwrap().get_events();
  let integrated = status(&events, &archived);
  assert!(matches!(integrated, BranchIntegrationStatus::Integrated { commit_count: 1, .. }), "{integrated:?}");
  let note = detection_note(&local_repo, &tip);
  assert!(matches!(note.info.status, BranchIntegrationStatus::Integrated { .. }), "{:?}", note.info.status);
  assert_eq!(note.check, None);
}

/// The user's case: the parent's tip got a `NotIntegrated` note before its merge, then upstream took the parent
#[test(tokio::test)]
async fn a_stale_note_on_the_virtual_tip_does_not_hide_an_integrated_parent() {
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();
  let (auth, _) = stacked_clone(&local_repo, &git_executor).await;

  stale_parent_lands(&upstream_repo, &local_repo, &git_executor, &auth).await;
}

/// The parent was archived a sync ago, with a note that is stale now: the archived branch is checked again before the
/// placement too, so the parent is "landed" on the first sync after its merge, also in `bd status`
#[test(tokio::test)]
async fn a_stale_note_on_the_archived_parent_does_not_hide_an_integrated_parent() {
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();
  let (auth, _) = stacked_clone(&local_repo, &git_executor).await;
  let archived = archive_name(0, "auth");
  local_repo.create_branch_at(&archived, &auth).unwrap();
  local_repo
    .git_executor()
    .execute_command(&["update-ref", "-d", "refs/heads/user/virtual/auth"], path(&local_repo))
    .unwrap();

  stale_parent_lands(&upstream_repo, &local_repo, &git_executor, &auth).await;
}

/// A branch fast-forwarded into the baseline has no equivalent commit, its commits are the baseline's: the tip is the
/// commit that brought it in
#[test]
fn a_fast_forwarded_branch_names_its_tip_as_the_integration_commit() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  let base = local_repo.head();
  let tip = local_repo.create_commit("Add feature", "feature.rs", "// feature");
  let archived = archive_name(0, "feature");
  local_repo.create_branch_at(&archived, &tip).unwrap();
  local_repo.reset_hard(&base).unwrap();
  write_checked_against(&local_repo, &tip, &base);
  // the baseline takes the very commit
  local_repo
    .git_executor()
    .execute_command(&["update-ref", "refs/remotes/origin/master", &tip], path(&local_repo))
    .unwrap();

  let (events, _) = detect(&local_repo);

  let integrated = status(&events, &archived);
  let BranchIntegrationStatus::Integrated {
    commit_count: 1,
    commit: Some(commit),
    ..
  } = integrated
  else {
    panic!("{integrated:?}");
  };
  assert_eq!(commit.id, tip);

  // a full detection of such a branch names the tip too
  let other = local_repo.create_commit("Add other", "other.rs", "// other");
  let archived_other = archive_name(0, "other");
  local_repo.create_branch_at(&archived_other, &other).unwrap();
  local_repo
    .git_executor()
    .execute_command(&["update-ref", "refs/remotes/origin/master", &other], path(&local_repo))
    .unwrap();
  let (events, _) = detect(&local_repo);
  let integrated = status(&events, &archived_other);
  let BranchIntegrationStatus::Integrated { commit: Some(commit), .. } = integrated else {
    panic!("{integrated:?}");
  };
  assert_eq!(commit.id, other);
}

#[test(tokio::test)]
async fn partial_integration_is_tracked_incrementally() {
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();
  local_repo.create_commit("(feature) Add one", "one.rs", "// one");
  local_repo.create_commit("(feature) Add two", "two.rs", "// two");
  sync_user_branches(&git_executor, &local_repo, DetectionStrategy::Rebase).await.unwrap();
  let tip = local_repo.rev_parse("user/virtual/feature").unwrap();
  let archived = archive_name(0, "feature");
  local_repo.reset_hard("origin/master").unwrap();
  let sync = || async { sync_user_branches(&git_executor, &local_repo, DetectionStrategy::Rebase).await.unwrap().get_events() };

  let events = sync().await;
  assert!(matches!(status(&events, &archived), BranchIntegrationStatus::NotIntegrated { orphaned_count: 2, .. }));
  // both commits of the branch side are listed, newest first
  let one = local_repo.rev_parse(&format!("{tip}^")).unwrap();
  assert_eq!(detection_note(&local_repo, &tip).check.unwrap().orphaned, vec![tip.clone(), one]);

  upstream_repo.create_commit("Add one", "one.rs", "// one");
  local_repo.fetch_prune("origin").unwrap();
  let events = sync().await;
  let partial = status(&events, &archived);
  assert!(
    matches!(
      partial,
      BranchIntegrationStatus::NotIntegrated {
        total_commit_count: 2,
        integrated_count: 1,
        orphaned_count: 1,
        integrated_at: Some(_),
      }
    ),
    "{partial:?}"
  );
  // `two`, the tip, is left
  assert_eq!(detection_note(&local_repo, &tip).check.unwrap().orphaned, vec![tip.clone()]);

  upstream_repo.create_commit("Add two", "two.rs", "// two");
  local_repo.fetch_prune("origin").unwrap();
  let events = sync().await;
  let integrated = status(&events, &archived);
  assert!(matches!(integrated, BranchIntegrationStatus::Integrated { commit_count: 2, .. }), "{integrated:?}");
}

#[test]
fn a_recheck_compares_only_the_baseline_commits_since_the_note() {
  let (upstream_repo, local_repo, _) = setup_test_repos();
  let base = local_repo.head();
  local_repo.create_commit("Add one", "one.rs", "// one");
  let two = local_repo.create_commit("Add two", "two.rs", "// two");
  let archived = archive_name(0, "feature");
  local_repo.create_branch_at(&archived, &two).unwrap();
  local_repo.reset_hard(&base).unwrap();
  // the note says `one` had an equivalent in `base`: a full detection would find none, the recheck keeps it
  let check = Recheck {
    baseline: base.clone(),
    orphaned: vec![two.clone()],
  };
  write_not_integrated(&local_repo, &two, 2, 1, Some(check));
  upstream_repo.create_commit("Add other", "other.rs", "// other");
  local_repo.fetch_prune("origin").unwrap();

  let (events, scans) = detect(&local_repo);

  let rechecked = status(&events, &archived);
  assert!(
    matches!(
      rechecked,
      BranchIntegrationStatus::NotIntegrated {
        total_commit_count: 2,
        integrated_count: 1,
        orphaned_count: 1,
        ..
      }
    ),
    "{rechecked:?}"
  );
  assert_eq!(scans.len(), 1, "{scans:#?}");
  assert!(scans[0].contains(&format!("\"^{base}\"")), "{scans:#?}");
  let baseline = local_repo.rev_parse("origin/master").unwrap();
  assert_eq!(
    detection_note(&local_repo, &two).check,
    Some(Recheck {
      baseline,
      orphaned: vec![two.clone()],
    })
  );

  // checked against the current baseline: reported from the note, nothing scanned
  let (events, scans) = detect(&local_repo);
  assert!(matches!(status(&events, &archived), BranchIntegrationStatus::NotIntegrated { orphaned_count: 1, .. }));
  assert_eq!(scans, Vec::<String>::new());
}

/// An archived branch whose tip has an equivalent upstream, with a version 1 `NotIntegrated` note, detected after the
/// fetch; returns the repositories, the archived branch, its tip and the events
fn archived_with_a_version_1_note(days_ago: i64) -> (TestRepo, TestRepo, String, String, Vec<SyncEvent>) {
  let (upstream_repo, local_repo, _) = setup_test_repos();
  let tip = local_repo.create_commit("Add feature", "feature.rs", "// feature");
  let archived = archive_name(days_ago, "feature");
  local_repo.create_branch_at(&archived, &tip).unwrap();
  local_repo.reset_hard("origin/master").unwrap();
  write_not_integrated(&local_repo, &tip, 1, 1, None);
  // another subject, else it is the very same commit
  upstream_repo.create_commit("Add feature upstream", "feature.rs", "// feature");
  local_repo.fetch_prune("origin").unwrap();
  let (events, _) = detect(&local_repo);
  (upstream_repo, local_repo, archived, tip, events)
}

#[test]
fn a_v1_note_within_retention_is_detected_afresh() {
  let (_upstream_repo, local_repo, archived, tip, events) = archived_with_a_version_1_note(2);

  assert!(matches!(status(&events, &archived), BranchIntegrationStatus::Integrated { commit_count: 1, .. }));
  assert!(matches!(detection_note(&local_repo, &tip).info.status, BranchIntegrationStatus::Integrated { .. }));
}

#[test]
fn a_v1_note_beyond_retention_is_reported_as_is() {
  let (_upstream_repo, local_repo, archived, tip, events) = archived_with_a_version_1_note(10);

  // a full scan of an old archive is slow: the note stays
  assert!(matches!(status(&events, &archived), BranchIntegrationStatus::NotIntegrated { orphaned_count: 1, .. }));
  assert_eq!(detection_note(&local_repo, &tip).check, None);
}

#[test]
fn a_rewound_baseline_falls_back_to_a_full_detection() {
  let (upstream_repo, local_repo, _) = setup_test_repos();
  let tip = local_repo.create_commit("Add feature", "feature.rs", "// feature");
  let archived = archive_name(0, "feature");
  local_repo.create_branch_at(&archived, &tip).unwrap();
  let feature = upstream_repo.create_commit("Add feature upstream", "feature.rs", "// feature");
  let later = upstream_repo.create_commit("Add other", "other.rs", "// other");
  local_repo.fetch_prune("origin").unwrap();
  // checked against `later`; a recheck from there would leave out the equivalent, which is older
  write_checked_against(&local_repo, &tip, &later);
  // the baseline is rewound to before `later`
  local_repo
    .git_executor()
    .execute_command(&["update-ref", "refs/remotes/origin/master", &feature], path(&local_repo))
    .unwrap();
  local_repo.reset_hard("origin/master").unwrap();

  let (events, scans) = detect(&local_repo);

  let detected = status(&events, &archived);
  assert!(matches!(detected, BranchIntegrationStatus::Integrated { commit_count: 1, .. }), "{detected:?}");
  assert_eq!(scans.len(), 1, "{scans:#?}");
  assert!(!scans[0].contains(&format!("^{later}")), "{scans:#?}");
}

#[test]
fn two_archived_names_with_one_tip_are_rechecked_once() {
  let (upstream_repo, local_repo, _) = setup_test_repos();
  let base = local_repo.head();
  let tip = local_repo.create_commit("IJAI-1 Add feature", "feature.rs", "// feature");
  // archived as the issue branch, then again under its new name after a rename
  let names = [archive_name(1, "IJAI-1"), archive_name(0, "feature")];
  for name in &names {
    local_repo.create_branch_at(name, &tip).unwrap();
  }
  local_repo.reset_hard(&base).unwrap();
  write_checked_against(&local_repo, &tip, &base);
  upstream_repo.create_commit("Add feature", "feature.rs", "// feature");
  local_repo.fetch_prune("origin").unwrap();

  let (events, scans) = detect(&local_repo);

  for name in &names {
    assert!(matches!(status(&events, name), BranchIntegrationStatus::Integrated { commit_count: 1, .. }), "{name}");
  }
  assert_eq!(scans.len(), 1, "{scans:#?}");
  // each name has its summary: the issue branch the subject without the key
  assert_eq!(detected_branch_info(&events, &names[0]).summary, "Add feature");
  assert_eq!(detected_branch_info(&events, &names[1]).summary, "");
}
