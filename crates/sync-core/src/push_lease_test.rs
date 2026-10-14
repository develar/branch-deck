use crate::push_lease::{LeaseSource, PUSHED_NAMESPACE, prepare_push, record_push};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const BRANCH: &str = "prefix/virtual/feature";

fn update_ref(repo: &TestRepo, reference: &str, id: &str) {
  repo.git_executor().execute_command(&["update-ref", reference, id], repo.path().to_str().unwrap()).unwrap();
}

#[test]
fn the_lease_is_the_record_else_the_tracking_ref_else_absent() {
  let repo = TestRepo::new();
  let first = repo.create_commit("One", "one.txt", "1");
  let second = repo.create_commit("Two", "two.txt", "2");
  update_ref(&repo, &format!("refs/heads/{BRANCH}"), &second);
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
