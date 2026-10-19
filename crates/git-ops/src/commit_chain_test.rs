use crate::commit_chain::{ChainCommit, CommitChain};
use crate::merge_memo::MergeOutcome;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// What `commit-tree` wrote for `commit` before the chain did
fn commit_tree(repo: &TestRepo, commit: &ChainCommit<'_>) -> String {
  let date = |timestamp: u32, timezone: &str| {
    if timezone.is_empty() {
      timestamp.to_string()
    } else {
      format!("{timestamp} {timezone}")
    }
  };
  let author_date = date(commit.author_timestamp, commit.author_timezone);
  let committer_date = date(commit.committer_timestamp, commit.committer_timezone);
  repo
    .git_executor()
    .cmd(&["commit-tree", commit.tree, "-p", commit.parent, "-m", commit.message], repo.path_str())
    .env(&[
      ("GIT_AUTHOR_NAME", commit.author_name),
      ("GIT_AUTHOR_EMAIL", commit.author_email),
      ("GIT_AUTHOR_DATE", &author_date),
      ("GIT_COMMITTER_NAME", commit.committer_name),
      ("GIT_COMMITTER_EMAIL", commit.committer_email),
      ("GIT_COMMITTER_DATE", &committer_date),
      // a date without a timezone is in the local one; the chain writes UTC
      ("TZ", "UTC"),
    ])
    .text()
    .unwrap()
}

fn commit<'a>(tree: &'a str, parent: &'a str, message: &'a str) -> ChainCommit<'a> {
  ChainCommit {
    tree,
    parent,
    author_name: "Ann Author",
    author_email: "ann@example.com",
    author_timestamp: 1_700_000_000,
    author_timezone: "+0200",
    committer_name: "branch-deck",
    committer_email: "ann@example.com",
    committer_timestamp: 1_700_000_100,
    committer_timezone: "-0130",
    message,
    resolution: None,
  }
}

#[test]
fn a_commit_has_the_id_commit_tree_gives_it() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  let tree = repo.git(&["rev-parse", &format!("{base}^{{tree}}")]);
  let messages = ["subject", "subject\n\nbody", "subject\n\nbody\n", "subject\n\n\n", "ünïcode — subject"];
  let mut chain = CommitChain::new(repo.git_executor(), repo.path_str());
  for message in messages {
    let commit = commit(&tree, &base, message);
    assert_eq!(chain.write_commit(&commit).unwrap(), commit_tree(&repo, &commit), "message {message:?}");
  }
  // git's crud around and inside the identity, and a date without a timezone
  let crud = ChainCommit {
    author_name: " .Ann <Author>; ",
    author_email: "<ann@example.com>",
    author_timezone: "",
    committer_timezone: "",
    ..commit(&tree, &base, "crud")
  };
  assert_eq!(chain.write_commit(&crud).unwrap(), commit_tree(&repo, &crud));
  chain.finish().unwrap();
}

#[test]
fn a_chain_writes_commits_on_its_own_commits_and_flushes_them_for_other_processes() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  let tree = repo.git(&["rev-parse", &format!("{base}^{{tree}}")]);
  let mut chain = CommitChain::new(repo.git_executor(), repo.path_str());
  let first = chain.write_commit(&commit(&tree, &base, "first")).unwrap();
  let second = chain.write_commit(&commit(&tree, &first, "second")).unwrap();
  chain.flush().unwrap();
  assert_eq!(repo.git(&["rev-parse", &format!("{second}^")]), first);
  // a commit after the flush is written by a new writer, on a parent of the old one
  let third = chain.write_commit(&commit(&tree, &second, "third")).unwrap();
  chain.finish().unwrap();
  assert_eq!(repo.git(&["log", "--format=%s", &format!("{base}..{third}")]), "third\nsecond\nfirst");
  assert_eq!(repo.git(&["for-each-ref", "refs/branch-deck/"]), "", "the writer leaves no ref");
}

#[test]
fn a_merge_of_trees_is_the_merge_of_their_commits() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("base", &[("a.txt", "1\n2\n3\n"), ("b.txt", "b\n")]);
  let target = repo.create_commit("target", "a.txt", "one\n2\n3\n");
  repo.git(&["checkout", "-q", "-b", "side", &base]);
  let clean = repo.create_commit("clean", "a.txt", "1\n2\nthree\n");
  repo.git(&["checkout", "-q", "-b", "other", &base]);
  let conflict = repo.create_commit("conflict", "a.txt", "uno\n2\n3\n");
  let tree = |commit: &str| repo.git(&["rev-parse", &format!("{commit}^{{tree}}")]);

  let mut chain = CommitChain::new(repo.git_executor(), repo.path_str());
  let merged = chain.merge(&tree(&base), &tree(&target), &tree(&clean)).unwrap();
  let expected = repo.git(&["merge-tree", "--write-tree", "--merge-base", &base, &target, &clean]);
  assert_eq!(merged, MergeOutcome::Clean { tree: expected });

  let merged = chain.merge(&tree(&base), &tree(&target), &tree(&conflict)).unwrap();
  assert_eq!(merged, MergeOutcome::Conflict { files: vec!["a.txt".to_string()] });
  // the process answers the merge after a conflict too
  let merged = chain.merge(&tree(&base), &tree(&base), &tree(&clean)).unwrap();
  assert_eq!(merged, MergeOutcome::Clean { tree: tree(&clean) });
  chain.finish().unwrap();
}

#[test]
fn a_chain_dropped_on_an_error_ends_its_writer_cleanly() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  let tree = repo.git(&["rev-parse", &format!("{base}^{{tree}}")]);
  let id = {
    let mut chain = CommitChain::new(repo.git_executor(), repo.path_str());
    chain.write_commit(&commit(&tree, &base, "dropped")).unwrap()
  };
  assert_eq!(repo.git(&["cat-file", "-t", &id]), "commit");
  let git_dir = repo.git(&["rev-parse", "--absolute-git-dir"]);
  let crash_reports = std::fs::read_dir(&git_dir)
    .unwrap()
    .filter(|entry| entry.as_ref().unwrap().file_name().to_string_lossy().starts_with("fast_import_crash"))
    .count();
  assert_eq!(crash_reports, 0);
}
