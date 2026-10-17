use super::commit_utils::{blob_id, empty_tree, find_commit, get_commits_info};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::fmt::Write;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

#[test]
fn test_get_commits_info_batch() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let repo_path = test_repo.path().to_str().unwrap();

  let commit1 = test_repo.create_commit("First commit", "file1.txt", "content1");
  let commit2 = test_repo.create_commit("Second commit\n\nBody line one\nBody line two", "file2.txt", "content2");
  let commit3 = test_repo.create_commit("Third commit with special chars: 'quotes' and \"double quotes\"", "file3.txt", "content3");

  // Duplicates are allowed and collapse into one entry
  let result = get_commits_info(git_executor, repo_path, &[&commit3, &commit1, &commit2, &commit1]).unwrap();
  assert_eq!(result.len(), 3);

  let info1 = &result[&commit1];
  assert_eq!(info1.id, commit1);
  assert_eq!(info1.subject, "First commit");
  assert_eq!(info1.message, "First commit");
  assert_eq!(info1.author_name, "Test User");
  assert!(info1.author_timestamp > 0);
  assert!(info1.committer_timestamp > 0);
  assert_eq!(info1.tree_id.len(), 40, "tree id: {}", info1.tree_id);

  let info2 = &result[&commit2];
  assert_eq!(info2.subject, "Second commit");
  assert_eq!(info2.message, "Second commit\n\nBody line one\nBody line two");
  assert_eq!(info2.parent_id.as_deref(), Some(commit1.as_str()));

  assert_eq!(result[&commit3].subject, "Third commit with special chars: 'quotes' and \"double quotes\"");
}

#[test]
fn test_get_commits_info_empty_input() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  let result = get_commits_info(git_executor, test_repo.path().to_str().unwrap(), &[]).unwrap();
  assert!(result.is_empty());
}

#[test]
fn test_get_commits_info_invalid_id() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let repo_path = test_repo.path().to_str().unwrap();

  let valid_commit = test_repo.create_commit("Valid commit", "file.txt", "content");

  get_commits_info(git_executor, repo_path, &[&valid_commit, "0000000000000000000000000000000000000000"]).unwrap_err();
  get_commits_info(git_executor, repo_path, &["invalid_hash"]).unwrap_err();
}

#[test]
fn test_get_commits_info_resolves_revision() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let repo_path = test_repo.path().to_str().unwrap();

  let commit = test_repo.create_commit("Only commit", "file.txt", "content");

  let commits = get_commits_info(git_executor, repo_path, &["HEAD"]).unwrap();
  // keyed by the full id the revision resolves to; an abbreviated id finds it too
  assert_eq!(find_commit(&commits, &commit[..7]).map(|info| info.id.as_str()), Some(commit.as_str()));
  let info = find_commit(&commits, &commit).unwrap();
  assert_eq!(info.id, commit);
  assert_eq!(info.subject, "Only commit");
  assert_eq!(info.parent_id, None);
}

#[test]
fn test_get_commits_info_many_ids() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let repo_path = test_repo.path().to_str().unwrap();

  // more ids than fit on a Windows command line; fast-import writes the commits in one go
  let count = 1500;
  let mut stream = String::new();
  for i in 1..=count {
    let message = format!("Commit {i}\n");
    let content = format!("{i}\n");
    let from = if i > 1 { format!("from :{}\n", i - 1) } else { String::new() };
    writeln!(
      stream,
      "commit refs/heads/many\nmark :{i}\ncommitter Test User <test@example.com> {} +0000\ndata {}\n{message}{from}M 644 inline file.txt\ndata {}\n{content}",
      1_700_000_000 + i,
      message.len(),
      content.len()
    )
    .unwrap();
  }
  git_executor.execute_command_with_input(&["fast-import", "--quiet"], repo_path, &stream).unwrap();
  let ids = git_executor.execute_command_lines(&["rev-list", "many"], repo_path).unwrap();
  assert_eq!(ids.len(), count);

  let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
  let result = get_commits_info(git_executor, repo_path, &id_refs).unwrap();

  assert_eq!(result.len(), count);
  let newest = &result[&ids[0]];
  assert_eq!(newest.subject, format!("Commit {count}"));
  assert_eq!(newest.parent_id.as_deref(), Some(ids[1].as_str()));
  assert_eq!(result[&ids[count - 1]].subject, "Commit 1");
}

#[test]
fn test_get_commits_info_refuses_options() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let repo_path = test_repo.path().to_str().unwrap();
  test_repo.create_commit("Only commit", "file.txt", "content");

  for id in ["--all", "^HEAD", "HEAD HEAD~1", ""] {
    assert!(get_commits_info(git_executor, repo_path, &[id]).is_err(), "{id:?}");
  }
}

/// The empty tree comes from the length of an id, without a process, in both hash algorithms
#[test]
fn the_empty_tree_of_each_hash_algorithm() {
  for format in ["sha1", "sha256"] {
    let repo = TestRepo::new_empty();
    repo.git(&["init", "--quiet", &format!("--object-format={format}")]);
    // the tree of the empty index
    let tree = repo.git(&["write-tree"]);
    assert_eq!(empty_tree(&tree), tree, "{format}");
  }
}

/// A blob id comes from the length of an id, without a process, as `hash-object --stdin` computes it, in both hash
/// algorithms
#[test]
fn a_blob_id_is_the_one_of_hash_object() {
  for format in ["sha1", "sha256"] {
    let repo = TestRepo::new_empty();
    repo.git(&["init", "--quiet", &format!("--object-format={format}")]);
    let tree = repo.git(&["write-tree"]);
    for content in ["", "target/of/a/link", "line\n\u{e9}\0binary"] {
      let expected = repo
        .git_executor()
        .execute_command_with_input(&["hash-object", "--stdin"], repo.path_str(), content)
        .unwrap();
      assert_eq!(blob_id(&tree, content.as_bytes()), expected, "{format} {content:?}");
    }
  }
}
