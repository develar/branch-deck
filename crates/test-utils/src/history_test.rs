use crate::git_test_utils::TestRepo;
use crate::history::{HistoryCommit, HistoryStream, cleaned_message};
use crate::process_budget::git_processes_of;

#[test]
fn a_message_is_cleaned_as_git_commit_m_cleans_it() {
  assert_eq!(cleaned_message("Subject"), "Subject\n");
  assert_eq!(cleaned_message("\n\nSubject  \n\n\n\nBody line\t\n\n"), "Subject\n\nBody line\n");
  assert_eq!(cleaned_message(""), "");
}

/// The history of a fixture is one `fast-import` and one `read-tree`, whatever the number of its commits, and its
/// commits are those `add` and `commit` write: the same ids at the same times, the files of HEAD in the working tree
/// with a clean status, a commit that changes nothing left out
#[test]
fn a_history_is_two_processes_and_the_commits_of_git_commit() {
  let commits = [
    HistoryCommit {
      timestamp: Some(1_700_000_000),
      ..HistoryCommit::new("Initial commit", &[("README.md", "# Test\n"), ("src/a.txt", "a\n")])
    },
    HistoryCommit {
      timestamp: Some(1_700_000_100),
      ..HistoryCommit::new("(auth) Add login\n\nThe body  \n", &[("src/login.txt", "login\n")])
    },
    HistoryCommit {
      timestamp: Some(1_700_000_200),
      ..HistoryCommit::new("Nothing changes", &[("src/a.txt", "a\n")])
    },
    HistoryCommit {
      deleted: vec!["src/a.txt".to_string()],
      timestamp: Some(1_700_000_300),
      ..HistoryCommit::new("Remove a", &[])
    },
  ];

  let written = TestRepo::new();
  let (ids, processes) = git_processes_of(|| {
    let mut history = HistoryStream::new("refs/heads/master");
    for commit in &commits {
      history.commit(commit);
    }
    history.reset_to_root("refs/remotes/origin/master");
    history.write(written.path()).unwrap()
  });
  assert_eq!(processes, ["fast-import", "read-tree"]);
  assert!(ids[2].is_none(), "a commit that changes nothing is left out: {ids:?}");

  let committed = TestRepo::new();
  let mut expected = Vec::new();
  for commit in &commits {
    if commit.message == "Nothing changes" {
      expected.push(None);
      continue;
    }
    for (path, content) in &commit.files {
      let file = committed.path().join(path);
      std::fs::create_dir_all(file.parent().unwrap()).unwrap();
      std::fs::write(file, content).unwrap();
    }
    for path in &commit.deleted {
      std::fs::remove_file(committed.path().join(path)).unwrap();
    }
    committed.git(&["add", "-A"]);
    let date = format!("{} +0000", commit.timestamp.unwrap());
    expected.push(Some(
      committed.commit_with(&["-m", &commit.message], &[("GIT_AUTHOR_DATE", &date), ("GIT_COMMITTER_DATE", &date)]),
    ));
  }
  assert_eq!(ids, expected);
  assert_eq!(written.git(&["rev-parse", "refs/remotes/origin/master"]), ids[0].clone().unwrap());
  assert_eq!(written.git(&["status", "--porcelain"]), "");
  assert_eq!(std::fs::read_to_string(written.path().join("src/login.txt")).unwrap(), "login\n");
  assert!(!written.path().join("src/a.txt").exists());
}
