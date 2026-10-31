use crate::push_lease::{LeaseSource, PushedRefStatus, fetch_remote_branches, parse_push_porcelain, prepare_push_expecting};
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
fn an_expected_lease_is_the_remote_tip_the_caller_saw() {
  let (repo, first, second) = repo_with_branch();
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
  assert_eq!(refs[0].refspec(), format!("{second}:refs/heads/{BRANCH}"));
  assert_eq!(refs[1].lease_arg(), "--force-with-lease=refs/heads/prefix/virtual/new:");
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

/// A clone whose remote has `BRANCH` at a commit someone else pushed, which its remote-tracking ref lacks (at the first
/// commit): the remote-tracking ref, that commit and the clone's newer commit a push of the branch would set
fn clone_behind_its_remote() -> (TestRepo, TestRepo, String, String) {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("a.txt", "a\n")]);
  let first = clone.head();
  let theirs = upstream.create_commit("Their change", "b.txt", "b\n");
  update_ref(&upstream, &format!("refs/heads/{BRANCH}"), &theirs);
  update_ref(&clone, &format!("refs/remotes/origin/{BRANCH}"), &first);
  let pushed = clone.create_commit("Pushed", "c.txt", "c\n");
  (upstream, clone, theirs, pushed)
}

/// The fetch moves the remote-tracking ref from the tip it read before: alone, to the remote's head; when a push of
/// the branch set its own head meanwhile (here between the read and the fetch, as git's push writes the
/// remote-tracking ref when it ends), that head stays, the fetched one does not write over it, and no temporary ref
/// is left
#[cfg(unix)]
#[test]
fn a_fetch_that_ends_after_a_push_of_its_ref_leaves_the_head_of_the_push() {
  let tracking = format!("refs/remotes/origin/{BRANCH}");
  let (_upstream, clone, theirs, _) = clone_behind_its_remote();
  fetch_remote_branches(clone.git_executor(), clone.path_str(), "origin", &[BRANCH.to_string()]).unwrap();
  assert_eq!(clone.rev_parse(&tracking).unwrap(), theirs);

  let (_upstream, clone, theirs, pushed) = clone_behind_its_remote();
  let wrapped = test_utils::wrapped_git::WrappedGit::new(&format!("*\" fetch \"*) git update-ref {tracking} {pushed}; touch '{{dir}}/pushed';;"));
  fetch_remote_branches(wrapped.executor(), clone.path_str(), "origin", &[BRANCH.to_string()]).unwrap();
  assert!(wrapped.marked("pushed"), "the push ran during the fetch");
  assert_eq!(clone.rev_parse(&tracking).unwrap(), pushed, "not {theirs}, which the fetch brought");
  assert_eq!(clone.git(&["for-each-ref", "refs/branch-deck/tmp/"]), "");
}
