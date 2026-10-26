use super::*;
use crate::repo_template::templates;
use pretty_assertions::assert_eq;

fn rev_parse(repo_path: &Path, revision: &str) -> String {
  run(repo_path, &["rev-parse", "--verify", revision]).unwrap().trim().to_string()
}

/// A template has the remote a clone left, its `master` at `origin/master`; a push in a terminal moves the remote branch
/// and the remote-tracking ref, a commit someone else pushed only the remote branch
#[test]
fn a_template_has_its_own_remote_which_a_terminal_and_someone_else_push_to() {
  let directory = tempfile::tempdir().unwrap();
  let repo = directory.path();
  templates::simple().build(repo).unwrap();
  let baseline = rev_parse(repo, "origin/master");
  assert_eq!(remote_branches(repo).unwrap(), BTreeMap::from([("master".to_string(), baseline)]));
  assert_eq!(run(repo, &["config", "remote.origin.url"]).unwrap().trim(), BARE_ORIGIN);

  let head = rev_parse(repo, "HEAD");
  assert_eq!(push_in_terminal(repo, "user-name/feature", "HEAD").unwrap(), head);
  assert_eq!(remote_branches(repo).unwrap()["user-name/feature"], head);
  assert_eq!(rev_parse(repo, "origin/user-name/feature"), head);

  let commit = RemoteCommit {
    subject: "Fix the pool".to_string(),
    author: "Ann <ann@example.com>".to_string(),
    timestamp: 1_704_178_800,
    files: vec![("pool.txt".to_string(), "pool\n".to_string())],
  };
  let theirs = commit_on_remote(repo, "user-name/feature", &commit).unwrap();
  assert_eq!(remote_branches(repo).unwrap()["user-name/feature"], theirs);
  assert_eq!(rev_parse(repo, "origin/user-name/feature"), head, "the remote-tracking ref stays until a fetch");
  let bare = bare_origin(repo);
  assert_eq!(
    run(&bare, &["log", "-1", "--format=%an <%ae> %at %s %P", &theirs]).unwrap().trim(),
    format!("Ann <ann@example.com> 1704178800 Fix the pool {head}")
  );
}

/// An earlier build copies the commits after the base with their trees, authors and messages, committed a day earlier
#[test]
fn an_earlier_build_is_the_same_commits_under_other_ids() {
  let directory = tempfile::tempdir().unwrap();
  let repo = directory.path();
  templates::simple().build(repo).unwrap();
  let copy = earlier_build(repo, "origin/master", "HEAD").unwrap();
  let format = "--format=%T %an <%ae> %at %B";
  let log = |revision: &str| run(repo, &["log", format, &format!("origin/master..{revision}")]).unwrap();
  assert_ne!(copy, rev_parse(repo, "HEAD"));
  assert_eq!(log(&copy), log("HEAD"));
  let committer_time = |revision: &str| run(repo, &["log", "-1", "--format=%ct", revision]).unwrap().trim().parse::<i64>().unwrap();
  assert_eq!(committer_time(&copy), committer_time("HEAD") - 86_400);
  earlier_build(repo, "HEAD", "HEAD").unwrap_err();
}
