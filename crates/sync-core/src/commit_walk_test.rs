use crate::commit_key::CommitKey;
use crate::commit_walk::walk_commits;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// `(mark, id, author, key, subject)` of each commit
fn fields(repo: &TestRepo, options: &[&str], revs: &[&str]) -> Vec<(char, String, String, CommitKey, String)> {
  walk_commits(repo.git_executor(), repo.path_str(), options, revs)
    .unwrap()
    .into_iter()
    .map(|commit| (commit.mark, commit.id, commit.author, commit.key, commit.subject))
    .collect()
}

#[test]
fn a_walk_reads_the_mark_the_author_and_the_identity_of_each_commit() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_timestamp("Initial commit", "a.txt", "a\n", Some(1_700_000_000));
  repo.checkout_new_branch("feature").unwrap();
  let login = repo.create_commit_with_timestamp("(auth) Add login", "login.txt", "login\n", Some(1_700_000_100));
  repo.checkout("master").unwrap();
  repo.git(&["cherry-pick", &login]);
  let copy = repo.head();
  let key = CommitKey::from_subject("test@example.com", 1_700_000_100, "Add login");

  // without `--left-right` every commit is `>`
  assert_eq!(
    fields(&repo, &[], &[&format!("^{base}"), "feature"]),
    [('>', login.clone(), "Test User".to_string(), key, "(auth) Add login".to_string())]
  );
  // a copy on the other side is `=`
  let mut marked: Vec<(char, String)> = fields(&repo, &["--left-right", "--cherry-mark"], &["master...feature"])
    .into_iter()
    .map(|(mark, id, ..)| (mark, id))
    .collect();
  marked.sort();
  let mut expected = [('=', login), ('=', copy)];
  expected.sort();
  assert_eq!(marked, expected);

  // a rev that names a file too is a rev: the walk ends its revs with `--`
  repo.create_commit("Add a file named feature", "feature", "feature\n");
  assert_eq!(fields(&repo, &[], &[&format!("^{base}"), "feature"]).len(), 1);
}
