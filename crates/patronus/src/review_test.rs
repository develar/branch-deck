use crate::client::PreMergeCommitDto;
use crate::fake_patronus::{FakePatronus, start_fake};
use crate::review::ReviewSubject;
use crate::service::PatronusService;
use crate::service_test::{service, token};
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::Ordering;

fn commits() -> Vec<PreMergeCommitDto> {
  vec![PreMergeCommitDto {
    hash: "tip".to_string(),
    is_merge_commit: false,
    message: "AIR-1 fix".to_string(),
    author_name: "Me".to_string(),
    author_email: "me@jetbrains.com".to_string(),
    commiter_name: "Me".to_string(),
    commiter_email: "me@jetbrains.com".to_string(),
    author_date: "2026-10-14T09:30:00Z".to_string(),
    committer_date: "2026-10-14T09:30:00Z".to_string(),
    changed_paths: vec!["plugins/air/src/A.kt".to_string()],
  }]
}

fn subject(commits: &[PreMergeCommitDto]) -> ReviewSubject<'_> {
  ReviewSubject {
    project_key: "ij",
    repository: "ultimate",
    target_branch: "master",
    source_branch: "me/virtual/AIR-1",
    commits,
    base: "base",
    reviewed_on_github: false,
  }
}

/// A Patronus whose pre-check wants approvals, and the service asking it
async fn approvals_required() -> (Arc<FakePatronus>, PatronusService, Vec<PreMergeCommitDto>) {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.review_required.store(true, Ordering::SeqCst);
  (fake, service, commits())
}

#[tokio::test]
async fn no_merge_request_is_looked_up_when_no_approval_is_needed() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let commits = commits();
  let requirement = service.review_requirement(&subject(&commits), false).await.unwrap();
  assert!(!requirement.required);
  assert_eq!((fake.check_calls.load(Ordering::SeqCst), fake.merge_request_calls.load(Ordering::SeqCst)), (1, 0));

  // the same commits: cached
  service.review_requirement(&subject(&commits), false).await.unwrap();
  assert_eq!(fake.check_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn names_the_missing_approvals_and_checks_again_with_the_merge_request() {
  let (fake, service, commits) = approvals_required().await;

  let without = service.review_requirement(&subject(&commits), false).await.unwrap();
  assert!(without.required);
  assert_eq!(without.merge_request, None);
  assert_eq!(without.groups[0].name, "Air Plugin Release Team");
  assert!(without.groups[0].reviewers[0].suggested);

  // a merge request opened since and approved: a recheck finds it, and the check with its key passes
  *fake.merge_request.lock().unwrap() = Some(serde_json::json!({"projectKey": "IJ", "reviewKey": "IJ-CR-2", "url": "https://jetbrains.team/p/ij/reviews/2"}));
  fake.review_approved.store(true, Ordering::SeqCst);
  let with = service.review_requirement(&subject(&commits), true).await.unwrap();
  assert!(!with.required);
  assert_eq!(with.merge_request.map(|merge_request| merge_request.review_key).as_deref(), Some("IJ-CR-2"));
  assert_eq!(*fake.checked_review_keys.lock().unwrap(), [None, None, Some("IJ-CR-2".to_string())]);
  assert_eq!(fake.merge_request_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_branch_reviewed_on_github_looks_up_no_merge_request() {
  let (fake, service, commits) = approvals_required().await;
  let subject = ReviewSubject {
    reviewed_on_github: true,
    ..subject(&commits)
  };
  let requirement = service.review_requirement(&subject, false).await.unwrap();
  assert!(requirement.required);
  assert_eq!(requirement.merge_request, None);
  assert_eq!(requirement.groups[0].name, "Air Plugin Release Team");
  assert_eq!((fake.check_calls.load(Ordering::SeqCst), fake.merge_request_calls.load(Ordering::SeqCst)), (1, 0));
}

#[tokio::test]
async fn a_required_review_is_asked_again_on_a_recheck() {
  let (fake, service, commits) = approvals_required().await;
  service.review_requirement(&subject(&commits), false).await.unwrap();
  service.review_requirement(&subject(&commits), false).await.unwrap();
  assert_eq!(fake.check_calls.load(Ordering::SeqCst), 1, "cached for two minutes");

  // the merge request was opened in the web UI meanwhile
  *fake.merge_request.lock().unwrap() = Some(serde_json::json!({"projectKey": "IJ", "reviewKey": "IJ-CR-1", "url": "u"}));
  let rechecked = service.review_requirement(&subject(&commits), true).await.unwrap();
  assert_eq!(rechecked.merge_request.map(|merge_request| merge_request.review_key).as_deref(), Some("IJ-CR-1"));
  assert_eq!(fake.merge_request_calls.load(Ordering::SeqCst), 2, "no merge request is asked again on a recheck");
}

#[tokio::test]
async fn an_unavailable_review_check_is_not_asked_again_by_itself() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.set_status(503);
  let commits = commits();
  let error = service.review_requirement(&subject(&commits), false).await.unwrap_err();
  assert_eq!(error, "The review check of Patronus is unavailable right now");
  assert_eq!(fake.check_calls.load(Ordering::SeqCst), 1);
}
