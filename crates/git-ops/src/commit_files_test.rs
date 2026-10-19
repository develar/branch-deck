use crate::commit_files::{CommitFiles, DiffSubject, parse_commit_files, read_commit_files, read_commit_files_cached, read_diff_files, read_files_from_base};
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
    old_id: None,
    new_id: None,
    submodule: false,
  }
}

/// The files without their blob ids, which a test of the paths and counts does not name
fn without_ids(files: &[CommitFile]) -> Vec<CommitFile> {
  files
    .iter()
    .map(|file| CommitFile {
      old_id: None,
      new_id: None,
      ..file.clone()
    })
    .collect()
}

fn ids(old_id: Option<&str>, new_id: Option<&str>, file: CommitFile) -> CommitFile {
  CommitFile {
    old_id: old_id.map(str::to_string),
    new_id: new_id.map(str::to_string),
    ..file
  }
}

/// A root commit with `a.txt` (3 lines) and `b.txt`, then a commit that renames `a.txt` to `c.txt` and adds a line,
/// deletes `b.txt`, adds a binary file and a file whose name has a space, a tab and a non-ASCII letter. That file is
/// in the index only: Windows has no file name with a tab
fn repo_with_all_statuses() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let root = repo.create_commit_with_files("Initial", &[("a.txt", "one\ntwo\nthree\n"), ("b.txt", "gone\n")]);
  run(&repo, &["mv", "a.txt", "c.txt"]);
  std::fs::write(repo.path().join("c.txt"), "one\ntwo\nthree\nfour\n").unwrap();
  run(&repo, &["rm", "-q", "b.txt"]);
  std::fs::write(repo.path().join("bin.dat"), [0u8, 1, 2, 0, 255]).unwrap();
  run(&repo, &["add", "-A"]);
  let second = repo.create_commit_in_index("Change everything", &[("dir name/ü\tx.txt", "q\n")]);
  (repo, root, second)
}

#[test]
fn lists_the_status_paths_and_line_counts_of_each_file() {
  let (repo, root, second) = repo_with_all_statuses();
  let files = read_commit_files(repo.git_executor(), repo_path(&repo), &[root.clone(), second.clone()]).unwrap();

  // the root commit adds everything
  assert_eq!(without_ids(&files[&root]), [file("a.txt", FileStatus::Added, 3, 0), file("b.txt", FileStatus::Added, 1, 0)]);
  assert_eq!(
    without_ids(&files[&second]),
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
fn each_file_has_the_blob_ids_of_its_sides() {
  let (repo, root, second) = repo_with_all_statuses();
  let files = read_commit_files(repo.git_executor(), repo_path(&repo), std::slice::from_ref(&second)).unwrap();
  let blob = |commit: &str, path: &str| Some(repo.rev_parse(&format!("{commit}:{path}")).unwrap());
  let sides: Vec<(&str, Option<String>, Option<String>)> = files[&second].iter().map(|file| (file.path.as_str(), file.old_id.clone(), file.new_id.clone())).collect();
  assert_eq!(
    sides,
    [
      ("b.txt", blob(&root, "b.txt"), None),
      ("bin.dat", None, blob(&second, "bin.dat")),
      ("c.txt", blob(&root, "a.txt"), blob(&second, "c.txt")),
      ("dir name/ü\tx.txt", None, blob(&second, "dir name/ü\tx.txt")),
    ]
  );
}

#[test]
fn a_submodule_has_the_ids_of_its_commits() {
  let repo = TestRepo::new();
  let root = repo.create_commit_with_files("Initial", &[("a.txt", "a\n")]);
  run(&repo, &["update-index", "--add", "--cacheinfo", &format!("160000,{root},sub")]);
  let second = repo.create_commit_in_index("Add a submodule", &[]);
  let files = read_commit_files(repo.git_executor(), repo_path(&repo), std::slice::from_ref(&second)).unwrap();
  assert_eq!(
    files[&second],
    [CommitFile {
      new_id: Some(root),
      submodule: true,
      ..file("sub", FileStatus::Added, 1, 0)
    }]
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
  assert_eq!(without_ids(&files[&merge]), [file("side.txt", FileStatus::Added, 1, 0)]);
  assert!(!files.contains_key(&empty));
}

/// The tips against one base in one process, keyed by the tip: the second commit of the fixture against its root, the
/// root against itself (no changes, no entry)
#[test]
fn lists_the_files_of_each_tip_against_a_base() {
  let (repo, root, second) = repo_with_all_statuses();
  let files = read_files_from_base(repo.git_executor(), repo_path(&repo), &root, &[second.as_str(), root.as_str()]).unwrap();
  assert_eq!(files.keys().collect::<Vec<_>>(), [&second]);
  assert_eq!(
    files[&second],
    read_commit_files(repo.git_executor(), repo_path(&repo), std::slice::from_ref(&second)).unwrap()[&second]
  );
  read_files_from_base(repo.git_executor(), repo_path(&repo), "HEAD", &[second.as_str()]).unwrap_err();
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
  let second_read = read_commit_files_cached(repo.git_executor(), missing_repository.to_str().unwrap(), &cache, &[root, second]).unwrap();
  assert_eq!(second_read, [first_read[1].clone(), first_read[0].clone()]);
}

#[test]
fn refuses_what_is_not_an_object_id() {
  let repo = TestRepo::new();
  let head = repo.create_commit("Initial", "README.md", "# Test\n");
  for id in ["HEAD", "--output=x", &head[..7]] {
    assert!(read_commit_files(repo.git_executor(), repo_path(&repo), &[id.to_string()]).is_err(), "{id}");
  }
}

fn range(base: &str, head: &str) -> DiffSubject {
  DiffSubject::Range {
    base: base.to_string(),
    head: head.to_string(),
  }
}

/// `repo_with_all_statuses`, then a commit that adds a line to the renamed `c.txt`
fn repo_with_a_range() -> (TestRepo, String, String) {
  let (repo, root, _) = repo_with_all_statuses();
  // `c.txt` alone: `commit -a` would also stage the deletion of the file that is in the index only
  let head = repo.create_commit("Add five", "c.txt", "one\ntwo\nthree\nfour\nfive\n");
  (repo, root, head)
}

#[test]
fn range_files_are_the_changes_of_all_its_commits() {
  let (repo, root, head) = repo_with_a_range();
  let files = read_diff_files(repo.git_executor(), repo_path(&repo), &range(&root, &head)).unwrap();
  assert_eq!(files.commit_id, format!("{root}..{head}"));
  assert_eq!(
    without_ids(&files.files),
    [
      file("b.txt", FileStatus::Deleted, 0, 1),
      CommitFile {
        binary: true,
        ..file("bin.dat", FileStatus::Added, 0, 0)
      },
      // renamed over both commits, with the lines of both
      CommitFile {
        old_path: Some("a.txt".to_string()),
        ..file("c.txt", FileStatus::Renamed, 2, 0)
      },
      file("dir name/ü\tx.txt", FileStatus::Added, 1, 0),
    ]
  );
  assert_eq!((files.additions, files.deletions), (3, 1));
}

#[test]
fn the_files_of_a_commit_subject_are_those_of_the_commit() {
  let (repo, root, second) = repo_with_all_statuses();
  let listed = read_commit_files(repo.git_executor(), repo_path(&repo), std::slice::from_ref(&second)).unwrap();
  let read = read_diff_files(repo.git_executor(), repo_path(&repo), &DiffSubject::Commit(second.clone())).unwrap();
  assert_eq!(read, CommitFiles::new(second.clone(), listed[&second].clone()));
  // a range of one commit is the commit
  let one = read_diff_files(repo.git_executor(), repo_path(&repo), &range(&root, &second)).unwrap();
  assert_eq!(one.files, read.files);
}

#[test]
fn a_range_refuses_what_is_not_an_object_id() {
  let repo = TestRepo::new();
  let head = repo.create_commit("Initial", "README.md", "# Test\n");
  for (base, tip) in [("HEAD", head.as_str()), (head.as_str(), "--output=x"), (&head[..7], head.as_str())] {
    assert!(read_diff_files(repo.git_executor(), repo_path(&repo), &range(base, tip)).is_err(), "{base}..{tip}");
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
  assert_eq!(
    files[ID_A],
    [
      ids(Some(&"1".repeat(40)), Some(&"2".repeat(40)), file("a.txt", FileStatus::Modified, 2, 1)),
      ids(None, Some(&"3".repeat(40)), file("b.txt", FileStatus::Added, 7, 0)),
    ]
  );
  assert_eq!(
    files[ID_B],
    [ids(
      Some(&"4".repeat(40)),
      Some(&"5".repeat(40)),
      CommitFile {
        old_path: Some("old.txt".to_string()),
        ..file("new.txt", FileStatus::Renamed, 1, 1)
      }
    )]
  );
}

#[test]
fn rejects_output_it_does_not_understand() {
  parse_commit_files(b"not an id\0").unwrap_err();
  parse_commit_files(b"1\t2\ta.txt\0").unwrap_err();
  assert!(parse_commit_files(&[]).unwrap().is_empty());
}
