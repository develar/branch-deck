use crate::commit_files::{CommitFiles, parse_commit_files, read_commit_file_diff, read_commit_files, read_commit_files_cached};
use object_cache::cache::{CacheConfig, ObjectCache};
use object_cache::commit_files::{CommitFile, FileStatus};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const ID_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn repo_path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

/// Run a porcelain command that builds the fixture
fn run(repo: &TestRepo, args: &[&str]) {
  repo.git_executor().execute_command(args, repo_path(repo)).unwrap();
}

fn file(path: &str, status: FileStatus, additions: u32, deletions: u32) -> CommitFile {
  CommitFile {
    path: path.to_string(),
    status,
    old_path: None,
    additions,
    deletions,
    binary: false,
  }
}

/// A root commit with `a.txt` (3 lines) and `b.txt`, then a commit that renames `a.txt` to `c.txt` and adds a line,
/// deletes `b.txt`, adds a binary file and a file whose name has a space, a tab and a non-ASCII letter
fn repo_with_all_statuses() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let root = repo.create_commit_with_files("Initial", &[("a.txt", "one\ntwo\nthree\n"), ("b.txt", "gone\n")]);
  run(&repo, &["mv", "a.txt", "c.txt"]);
  std::fs::write(repo.path().join("c.txt"), "one\ntwo\nthree\nfour\n").unwrap();
  run(&repo, &["rm", "-q", "b.txt"]);
  std::fs::write(repo.path().join("bin.dat"), [0u8, 1, 2, 0, 255]).unwrap();
  std::fs::create_dir_all(repo.path().join("dir name")).unwrap();
  std::fs::write(repo.path().join("dir name/ü\tx.txt"), "q\n").unwrap();
  run(&repo, &["add", "-A"]);
  run(&repo, &["commit", "-q", "-m", "Change everything"]);
  let second = repo.head();
  (repo, root, second)
}

#[test]
fn lists_the_status_paths_and_line_counts_of_each_file() {
  let (repo, root, second) = repo_with_all_statuses();
  let files = read_commit_files(repo.git_executor(), repo_path(&repo), &[root.clone(), second.clone()]).unwrap();

  // the root commit adds everything
  assert_eq!(files[&root], [file("a.txt", FileStatus::Added, 3, 0), file("b.txt", FileStatus::Added, 1, 0)]);
  assert_eq!(
    files[&second],
    [
      file("b.txt", FileStatus::Deleted, 0, 1),
      CommitFile {
        binary: true,
        ..file("bin.dat", FileStatus::Added, 0, 0)
      },
      CommitFile {
        old_path: Some("a.txt".to_string()),
        ..file("c.txt", FileStatus::Renamed, 1, 0)
      },
      file("dir name/ü\tx.txt", FileStatus::Added, 1, 0),
    ]
  );
}

#[test]
fn lists_a_merge_against_its_first_parent_and_nothing_for_an_empty_commit() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "# Test\n");
  repo.create_branch("side").unwrap();
  repo.checkout("side").unwrap();
  repo.create_commit("Side", "side.txt", "side\n");
  repo.checkout("master").unwrap();
  repo.create_commit("Main", "main.txt", "main\n");
  let merge = repo.merge_no_ff("side", "Merge side").unwrap();
  run(&repo, &["commit", "-q", "--allow-empty", "-m", "Nothing"]);
  let empty = repo.head();

  let files = read_commit_files(repo.git_executor(), repo_path(&repo), &[merge.clone(), empty.clone()]).unwrap();
  assert_eq!(files[&merge], [file("side.txt", FileStatus::Added, 1, 0)]);
  assert!(!files.contains_key(&empty));
}

#[test]
fn reads_through_the_cache_once_per_commit() {
  let (repo, root, second) = repo_with_all_statuses();
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(cache_dir.path(), &CacheConfig::default());
  run(&repo, &["commit", "-q", "--allow-empty", "-m", "Nothing"]);
  let empty = repo.head();

  let ids = [second.clone(), root.clone(), second.clone(), empty.clone()];
  let first_read = read_commit_files_cached(repo.git_executor(), repo_path(&repo), &cache, &ids).unwrap();
  assert_eq!(first_read.iter().map(|files| files.commit_id.as_str()).collect::<Vec<_>>(), ids);
  assert_eq!((first_read[0].files.len(), first_read[0].additions, first_read[0].deletions), (4, 2, 1));
  assert_eq!(first_read[0], first_read[2]);
  assert_eq!(first_read[3], CommitFiles::new(empty.clone(), Vec::new()));

  // what git listed is cached; the commit git listed nothing for is not
  let cached = cache.get_commit_files(&ids);
  assert_eq!(cached[&second], first_read[0].files);
  assert_eq!(cached[&root], first_read[1].files);
  assert!(!cached.contains_key(&empty));

  // a hit runs no git: a repository path that doesn't exist would fail it
  let missing_repository = repo.path().join("missing");
  let second_read = read_commit_files_cached(repo.git_executor(), missing_repository.to_str().unwrap(), &cache, &[root.clone(), second.clone()]).unwrap();
  assert_eq!(second_read, [first_read[1].clone(), first_read[0].clone()]);
}

#[test]
fn refuses_what_is_not_an_object_id() {
  let repo = TestRepo::new();
  let head = repo.create_commit("Initial", "README.md", "# Test\n");
  for id in ["HEAD", "--output=x", &head[..7]] {
    assert!(read_commit_files(repo.git_executor(), repo_path(&repo), &[id.to_string()]).is_err(), "{id}");
    assert!(read_commit_file_diff(repo.git_executor(), repo_path(&repo), id, "README.md", None).is_err(), "{id}");
  }
}

#[test]
fn parses_numstat_entries_in_another_order_than_the_raw_entries() {
  let raw_a = b":100644 100644 1111111111111111111111111111111111111111 2222222222222222222222222222222222222222 M\0a.txt\0";
  let raw_b = b":000000 100644 0000000000000000000000000000000000000000 3333333333333333333333333333333333333333 A\0b.txt\0";
  let rename = b":100644 100644 4444444444444444444444444444444444444444 5555555555555555555555555555555555555555 R090\0old.txt\0new.txt\0";
  let mut output = Vec::new();
  output.extend_from_slice(format!("{ID_A}\0").as_bytes());
  output.extend_from_slice(raw_a);
  output.extend_from_slice(raw_b);
  output.extend_from_slice(b"7\t0\tb.txt\0");
  output.extend_from_slice(b"2\t1\ta.txt\0");
  output.extend_from_slice(format!("{ID_B}\0").as_bytes());
  output.extend_from_slice(rename);
  output.extend_from_slice(b"1\t1\t\0old.txt\0new.txt\0");

  let files = parse_commit_files(&output).unwrap();
  assert_eq!(files[ID_A], [file("a.txt", FileStatus::Modified, 2, 1), file("b.txt", FileStatus::Added, 7, 0)]);
  assert_eq!(
    files[ID_B],
    [CommitFile {
      old_path: Some("old.txt".to_string()),
      ..file("new.txt", FileStatus::Renamed, 1, 1)
    }]
  );
}

#[test]
fn rejects_output_it_does_not_understand() {
  assert!(parse_commit_files(b"not an id\0").is_err());
  assert!(parse_commit_files(b"1\t2\ta.txt\0").is_err());
  assert!(parse_commit_files(&[]).unwrap().is_empty());
}

#[test]
fn file_diff_of_a_rename_shows_the_changed_lines() {
  let (repo, _, second) = repo_with_all_statuses();
  let renamed = read_commit_file_diff(repo.git_executor(), repo_path(&repo), &second, "c.txt", Some("a.txt")).unwrap();
  assert_eq!((renamed.old_file.file_name.as_str(), renamed.new_file.file_name.as_str()), ("a.txt", "c.txt"));
  assert_eq!(renamed.hunks.len(), 1);
  let patch = &renamed.hunks[0];
  assert!(patch.starts_with("diff --git a/a.txt b/c.txt\n"), "{patch}");
  assert!(patch.contains("rename from a.txt\nrename to c.txt\n"), "{patch}");
  assert!(patch.ends_with(" three\n+four\n"), "{patch}");

  // without the old path, the file is new
  let added = read_commit_file_diff(repo.git_executor(), repo_path(&repo), &second, "c.txt", None).unwrap();
  assert!(added.hunks[0].contains("new file mode"), "{}", added.hunks[0]);
}

#[test]
fn file_diff_takes_the_path_literally() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("*.txt", "star\n"), ("a.txt", "a\n")]);
  let head = repo.create_commit_with_files("Change both", &[("*.txt", "star 2\n"), ("a.txt", "a 2\n")]);
  let diff = read_commit_file_diff(repo.git_executor(), repo_path(&repo), &head, "*.txt", None).unwrap();
  assert_eq!(diff.new_file.file_lang, "txt");
  assert!(diff.hunks[0].contains("+star 2\n"), "{}", diff.hunks[0]);
  assert!(!diff.hunks[0].contains("a.txt"), "{}", diff.hunks[0]);

  // a path the commit didn't change has no patch
  let unchanged = read_commit_file_diff(repo.git_executor(), repo_path(&repo), &head, "missing.txt", None).unwrap();
  assert!(unchanged.hunks.is_empty());
}
