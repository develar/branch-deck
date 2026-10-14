use crate::push_lease::{LeaseSource, PUSHED_NAMESPACE, PushedRefStatus, parse_push_porcelain, prepare_push, prepare_push_expecting, record_push};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const BRANCH: &str = "prefix/virtual/feature";

fn update_ref(repo: &TestRepo, reference: &str, id: &str) {
  repo.git_executor().execute_command(&["update-ref", reference, id], repo.path().to_str().unwrap()).unwrap();
}

/// A repository with two commits, `BRANCH` at the second
fn repo_with_branch() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let first = repo.create_commit("One", "one.txt", "1");
  let second = repo.create_commit("Two", "two.txt", "2");
  update_ref(&repo, &format!("refs/heads/{BRANCH}"), &second);
  (repo, first, second)
}

#[test]
fn the_lease_is_the_record_else_the_tracking_ref_else_absent() {
  let (repo, first, second) = repo_with_branch();
  let path = repo.path().to_str().unwrap();
  let prepare = |force| prepare_push(repo.git_executor(), path, "origin", &[BRANCH.to_string()], force).unwrap().remove(0);

  let push = prepare(false);
  assert_eq!((push.tip.as_str(), push.lease.as_deref(), push.source), (second.as_str(), None, LeaseSource::Absent));
  assert_eq!(push.lease_arg(), format!("--force-with-lease=refs/heads/{BRANCH}:"));
  assert_eq!(push.refspec(), format!("{second}:refs/heads/{BRANCH}"));

  update_ref(&repo, &format!("refs/remotes/origin/{BRANCH}"), &first);
  assert_eq!((prepare(false).lease, prepare(false).source), (Some(first.clone()), LeaseSource::Tracking));

  // a recorded push wins over a fetched remote branch, unless the caller forces
  record_push(repo.git_executor(), path, &[push]).unwrap();
  assert_eq!(repo.rev_parse(&format!("{PUSHED_NAMESPACE}{BRANCH}")).unwrap(), second);
  assert_eq!((prepare(false).lease, prepare(false).source), (Some(second.clone()), LeaseSource::Recorded));
  assert_eq!((prepare(true).lease, prepare(true).source), (Some(first), LeaseSource::Tracking));

  let error = prepare_push(repo.git_executor(), path, "origin", &["prefix/virtual/missing".to_string()], false).unwrap_err();
  assert!(error.to_string().contains("does not exist"), "{error}");
}

#[test]
fn an_expected_lease_is_the_remote_tip_the_caller_saw_whatever_was_recorded() {
  let (repo, first, second) = repo_with_branch();
  update_ref(&repo, &format!("{PUSHED_NAMESPACE}{BRANCH}"), &second);
  let path = repo.path().to_str().unwrap();

  let refs = prepare_push_expecting(
    repo.git_executor(),
    path,
    &[(BRANCH.to_string(), Some(first.clone())), ("prefix/virtual/new".to_string(), None)],
  );
  let error = refs.unwrap_err();
  assert!(error.to_string().contains("prefix/virtual/new does not exist"), "{error}");

  update_ref(&repo, "refs/heads/prefix/virtual/new", &first);
  let refs = prepare_push_expecting(
    repo.git_executor(),
    path,
    &[(BRANCH.to_string(), Some(first.clone())), ("prefix/virtual/new".to_string(), None)],
  )
  .unwrap();
  let leases: Vec<_> = refs.iter().map(|push| (push.tip.as_str(), push.lease.as_deref(), push.source)).collect();
  assert_eq!(
    leases,
    [(second.as_str(), Some(first.as_str()), LeaseSource::Expected), (first.as_str(), None, LeaseSource::Absent)]
  );
  assert_eq!(refs[0].lease_arg(), format!("--force-with-lease=refs/heads/{BRANCH}:{first}"));
}

#[test]
fn parses_porcelain_lines() {
  let output = "To /tmp/remote\n*\trefs/heads/a:refs/heads/a\t[new branch]\n+\trefs/heads/b:refs/heads/b\t1111...2222 (forced update)\n!\trefs/heads/c:refs/heads/c\t[rejected] (stale info)\nDone";
  let status = |flag, summary: &str| PushedRefStatus {
    flag,
    summary: summary.to_string(),
  };
  let statuses = parse_push_porcelain(output);
  assert_eq!(
    statuses,
    HashMap::from([
      ("refs/heads/a".to_string(), status('*', "[new branch]")),
      ("refs/heads/b".to_string(), status('+', "1111...2222 (forced update)")),
      ("refs/heads/c".to_string(), status('!', "[rejected] (stale info)")),
    ])
  );
  assert!(statuses["refs/heads/b"].accepted());
  assert!(!statuses["refs/heads/c"].accepted());
}
