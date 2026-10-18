use crate::commit_files::{
  BlobContents, CommitFiles, DiffSubject, FilePatch, MAX_BLOB_BYTES, MAX_BLOB_IDS, MAX_CACHED_PATCH_BYTES, MAX_WARM_FILES, MAX_WARM_LINES, PATCH_CACHE_VERSION, PATCH_LIMITS,
  PatchLimits, PatchSource, PatchSplitter, PatchWarm, parse_commit_files, patch_bytes, patch_warm_ids, read_blob_contents, read_commit_files, read_commit_files_cached,
  read_commit_patch, read_diff_files, read_files_from_base, read_patch, read_patch_and_files, read_patch_cached, read_patches, warm_patches,
};
use object_cache::cache::{CacheConfig, ObjectCache};
use object_cache::commit_files::{CommitFile, FileStatus};
use pretty_assertions::assert_eq;
use std::fmt::Write as _;
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
    assert!(read_commit_patch(repo.git_executor(), repo_path(&repo), id, &PATCH_LIMITS).is_err(), "{id}");
    assert!(read_blob_contents(repo.git_executor(), repo_path(&repo), &[id.to_string()]).is_err(), "{id}");
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
  std::fs::write(repo.path().join("c.txt"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
  run(&repo, &["commit", "-q", "-a", "-m", "Add five"]);
  let head = repo.head();
  (repo, root, head)
}

#[test]
fn range_files_are_the_changes_of_all_its_commits() {
  let (repo, root, head) = repo_with_a_range();
  let files = read_diff_files(repo.git_executor(), repo_path(&repo), &range(&root, &head)).unwrap();
  assert_eq!(files.commit_id, format!("{root}..{head}"));
  assert_eq!(
    files.files,
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
fn range_patch_has_a_section_per_file_of_the_range() {
  let (repo, root, head) = repo_with_a_range();
  let patch = read_patch(repo.git_executor(), repo_path(&repo), &range(&root, &head), &PATCH_LIMITS).unwrap();
  assert_eq!(patch.commit_id, format!("{root}..{head}"));
  let starts: Vec<&str> = patch.files.iter().map(|file| file.patch.lines().next().unwrap()).collect();
  assert_eq!(
    starts,
    [
      "diff --git a/b.txt b/b.txt",
      "diff --git a/bin.dat b/bin.dat",
      "diff --git a/a.txt b/c.txt",
      "diff --git \"a/dir name/ü\\tx.txt\" \"b/dir name/ü\\tx.txt\"",
    ]
  );
  let renamed = &patch.files[2].patch;
  assert!(renamed.contains("rename from a.txt\nrename to c.txt\n"), "{renamed}");
  assert!(renamed.ends_with(" three\n+four\n+five\n"), "{renamed}");
  let (old, new) = index_ids(renamed);
  assert_eq!((old.len(), new.len()), (40, 40));
}

#[test]
fn a_range_refuses_what_is_not_an_object_id() {
  let repo = TestRepo::new();
  let head = repo.create_commit("Initial", "README.md", "# Test\n");
  for (base, tip) in [("HEAD", head.as_str()), (head.as_str(), "--output=x"), (&head[..7], head.as_str())] {
    assert!(read_diff_files(repo.git_executor(), repo_path(&repo), &range(base, tip)).is_err(), "{base}..{tip}");
    assert!(
      read_patch(repo.git_executor(), repo_path(&repo), &range(base, tip), &PATCH_LIMITS).is_err(),
      "{base}..{tip}"
    );
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
  parse_commit_files(b"not an id\0").unwrap_err();
  parse_commit_files(b"1\t2\ta.txt\0").unwrap_err();
  assert!(parse_commit_files(&[]).unwrap().is_empty());
}

/// The full blob ids of the `index <old>..<new>` line of a patch section
fn index_ids(patch: &str) -> (String, String) {
  let range = patch.lines().find_map(|line| line.strip_prefix("index ")).unwrap();
  let range = range.split(' ').next().unwrap();
  let (old, new) = range.split_once("..").unwrap();
  (old.to_string(), new.to_string())
}

#[test]
fn commit_patch_has_a_section_per_file_in_git_order() {
  let (repo, root, second) = repo_with_all_statuses();
  let patch = read_commit_patch(repo.git_executor(), repo_path(&repo), &second, &PATCH_LIMITS).unwrap();
  assert_eq!(patch.commit_id, second);
  let starts: Vec<&str> = patch.files.iter().map(|file| file.patch.lines().next().unwrap()).collect();
  // git sorts by path; a path with a tab is quoted
  assert_eq!(
    starts,
    [
      "diff --git a/b.txt b/b.txt",
      "diff --git a/bin.dat b/bin.dat",
      "diff --git a/a.txt b/c.txt",
      "diff --git \"a/dir name/ü\\tx.txt\" \"b/dir name/ü\\tx.txt\"",
    ]
  );
  assert!(patch.files.iter().all(|file| file.omitted_lines.is_none()));
  assert!(patch.files[1].patch.contains("Binary files /dev/null and b/bin.dat differ"), "{}", patch.files[1].patch);

  // the root commit adds every file
  let root_patch = read_commit_patch(repo.git_executor(), repo_path(&repo), &root, &PATCH_LIMITS).unwrap();
  assert_eq!(root_patch.files.len(), 2);
  assert!(root_patch.files[0].patch.contains("new file mode"), "{}", root_patch.files[0].patch);
}

#[test]
fn commit_patch_of_a_rename_shows_the_changed_lines_with_full_blob_ids() {
  let (repo, _, second) = repo_with_all_statuses();
  let patch = read_commit_patch(repo.git_executor(), repo_path(&repo), &second, &PATCH_LIMITS).unwrap();
  let renamed = &patch.files[2].patch;
  assert!(renamed.contains("rename from a.txt\nrename to c.txt\n"), "{renamed}");
  // untrimmed: the section ends with its last line and its newline
  assert!(renamed.ends_with(" three\n+four\n"), "{renamed}");

  let (old, new) = index_ids(renamed);
  assert_eq!((old.len(), new.len()), (40, 40));
  let blobs = read_blob_contents(repo.git_executor(), repo_path(&repo), &[old.clone(), new.clone()]).unwrap();
  assert_eq!(
    blobs,
    [
      BlobContents::Text {
        object_id: old,
        content: "one\ntwo\nthree\n".to_string()
      },
      BlobContents::Text {
        object_id: new,
        content: "one\ntwo\nthree\nfour\n".to_string()
      },
    ]
  );
}

#[test]
fn commit_patch_keeps_only_the_header_of_a_file_over_a_limit() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("big.txt", "a\n"), ("small.txt", "s\n")]);
  let big = (0..50).fold(String::new(), |mut text, i| {
    let _ = writeln!(text, "line {i}");
    text
  });
  let head = repo.create_commit_with_files("Grow", &[("big.txt", &big), ("small.txt", "s 2\n")]);
  let limits = PatchLimits { file_lines: 20, ..PATCH_LIMITS };
  let patch = read_commit_patch(repo.git_executor(), repo_path(&repo), &head, &limits).unwrap();
  // 1 deleted and 50 added lines
  assert_eq!(patch.files[0].omitted_lines, Some(51));
  assert!(patch.files[0].patch.ends_with("+++ b/big.txt\n"), "{}", patch.files[0].patch);
  assert!(!patch.files[0].patch.contains("@@"), "{}", patch.files[0].patch);
  assert_eq!(patch.files[1].omitted_lines, None);
  assert!(patch.files[1].patch.contains("+s 2\n"), "{}", patch.files[1].patch);
}

#[test]
fn commit_patch_leaves_out_the_hunks_after_the_total_limit() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("a.txt", "a\n"), ("b.txt", "b\n")]);
  let head = repo.create_commit_with_files("Change both", &[("a.txt", "a 2\n"), ("b.txt", "b 2\n")]);
  let full = read_commit_patch(repo.git_executor(), repo_path(&repo), &head, &PATCH_LIMITS).unwrap();
  let limits = PatchLimits {
    total_bytes: full.files[0].patch.len() + 10,
    ..PATCH_LIMITS
  };
  let patch = read_commit_patch(repo.git_executor(), repo_path(&repo), &head, &limits).unwrap();
  assert_eq!(patch.files[0], full.files[0]);
  assert_eq!(patch.files[1].omitted_lines, Some(2));
  assert!(patch.files[1].patch.starts_with("diff --git a/b.txt b/b.txt\n"), "{}", patch.files[1].patch);
  assert!(!patch.files[1].patch.contains("@@"), "{}", patch.files[1].patch);
}

#[test]
fn patch_splitter_cuts_at_diff_headers_across_chunk_boundaries() {
  let input = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-diff --git a/y b/y\n+x\n\\ No newline at end of file\ndiff --git a/z b/z\n@@ -0,0 +1 @@\n+z";
  for size in 1..=7 {
    let mut splitter = PatchSplitter::new(PATCH_LIMITS);
    for chunk in input.as_bytes().chunks(size) {
      splitter.push(chunk);
    }
    let files = splitter.finish();
    let patches: Vec<&str> = files.iter().map(|file| file.patch.as_str()).collect();
    // a deleted line that reads like a header stays in its hunk
    assert_eq!(
      patches,
      [
        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-diff --git a/y b/y\n+x\n\\ No newline at end of file\n",
        "diff --git a/z b/z\n@@ -0,0 +1 @@\n+z",
      ],
      "chunks of {size}"
    );
  }
}

#[test]
fn blob_contents_tells_text_from_binary_missing_and_too_many() {
  let (repo, _, second) = repo_with_all_statuses();
  let patch = read_commit_patch(repo.git_executor(), repo_path(&repo), &second, &PATCH_LIMITS).unwrap();
  let (_, binary) = index_ids(&patch.files[1].patch);
  let absent = "0123456789012345678901234567890123456789".to_string();
  let blobs = read_blob_contents(repo.git_executor(), repo_path(&repo), &[binary.clone(), absent.clone(), second.clone()]).unwrap();
  assert_eq!(
    blobs,
    [
      BlobContents::Binary { object_id: binary },
      BlobContents::Missing { object_id: absent },
      // a commit is not a blob
      BlobContents::Missing { object_id: second.clone() },
    ]
  );
  read_blob_contents(repo.git_executor(), repo_path(&repo), &vec![second; MAX_BLOB_IDS + 1]).unwrap_err();
}

/// A blob over `MAX_BLOB_BYTES` is too large, with its size; its bytes are skipped, and the blob after it is read
#[test]
fn a_blob_over_the_limit_is_too_large_with_its_size() {
  let repo = TestRepo::new();
  let large = "x".repeat(MAX_BLOB_BYTES + 1);
  let write = |content: &str| {
    repo
      .git_executor()
      .cmd(&["hash-object", "-w", "--stdin"], repo_path(&repo))
      .stdin(content.as_bytes())
      .text()
      .unwrap()
  };
  let (large_id, small_id) = (write(&large), write("small\n"));

  let blobs = read_blob_contents(repo.git_executor(), repo_path(&repo), &[large_id.clone(), small_id.clone()]).unwrap();

  assert_eq!(
    blobs,
    [
      BlobContents::TooLarge {
        object_id: large_id,
        size: u32::try_from(MAX_BLOB_BYTES + 1).unwrap(),
      },
      BlobContents::Text {
        object_id: small_id,
        content: "small\n".to_string(),
      },
    ]
  );
}

/// `repo_with_all_statuses` (a root commit, then a rename, a deletion and a binary file), then a side branch merged
/// with a file whose context line is the id of the root commit, and an empty commit; the ids from the root up
fn repo_with_every_kind_of_commit() -> (TestRepo, Vec<String>) {
  let (repo, root, second) = repo_with_all_statuses();
  repo.create_commit("Id line", "ids.txt", &format!("{root}\nfirst\n"));
  let id_line = repo.head();
  repo.create_branch("side").unwrap();
  repo.checkout("side").unwrap();
  let side = repo.create_commit("Side", "side.txt", "side\n");
  repo.checkout("master").unwrap();
  let main = repo.create_commit("Main", "ids.txt", &format!("{root}\nsecond\n"));
  let merge = repo.merge_no_ff("side", "Merge side").unwrap();
  run(&repo, &["commit", "-q", "--allow-empty", "-m", "Nothing"]);
  let empty = repo.head();
  (repo, vec![root, second, id_line, side, main, merge, empty])
}

fn batch_patches(repo: &TestRepo, ids: &[String], limits: &PatchLimits) -> Vec<(String, Vec<FilePatch>)> {
  let mut patches = Vec::new();
  read_patches(repo.git_executor(), repo_path(repo), ids, limits, |id, files| patches.push((id, files))).unwrap();
  patches
}

#[test]
fn batch_patches_are_the_patches_of_each_commit() {
  let (repo, ids) = repo_with_every_kind_of_commit();
  let empty = ids.last().unwrap().clone();
  // newest first, as a list asks
  let ids: Vec<String> = ids.into_iter().rev().collect();
  let limits = PatchLimits { file_lines: 1, ..PATCH_LIMITS };
  for limits in [PATCH_LIMITS, limits] {
    let patches = batch_patches(&repo, &ids, &limits);
    // git prints nothing for the empty commit
    let expected: Vec<String> = ids.iter().filter(|id| **id != empty).cloned().collect();
    assert_eq!(patches.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(), expected);
    for (id, files) in patches {
      assert_eq!(files, read_commit_patch(repo.git_executor(), repo_path(&repo), &id, &limits).unwrap().files, "{id}");
    }
  }
  // the batch has every kind of section: a rename, a binary file, the first-parent side of the merge
  let all: String = batch_patches(&repo, &ids, &PATCH_LIMITS)
    .iter()
    .flat_map(|(_, files)| files.iter().map(|file| file.patch.clone()))
    .collect();
  for text in ["rename from a.txt", "Binary files", "+side", "new file mode"] {
    assert!(all.contains(text), "{text}");
  }
}

#[test]
fn one_read_gives_the_patch_and_the_files_of_a_range_as_the_two_reads_do() {
  let (repo, ids) = repo_with_every_kind_of_commit();
  let git = repo.git_executor();
  let limits = PatchLimits { file_lines: 1, ..PATCH_LIMITS };
  // ranges with a rename, a deletion, a binary file and a merge; a commit; and nothing between a commit and itself
  let subjects = [
    range(&ids[0], &ids[5]),
    range(&ids[1], &ids[6]),
    DiffSubject::Commit(ids[1].clone()),
    range(&ids[6], &ids[6]),
  ];
  for limits in [PATCH_LIMITS, limits] {
    for subject in &subjects {
      let (patch, files) = read_patch_and_files(git, repo_path(&repo), subject, &limits).unwrap();
      assert_eq!(patch, read_patch(git, repo_path(&repo), subject, &limits).unwrap(), "{subject:?}");
      assert_eq!(files, read_diff_files(git, repo_path(&repo), subject).unwrap(), "{subject:?}");
    }
  }
  let (patch, files) = read_patch_and_files(git, repo_path(&repo), &subjects[0], &PATCH_LIMITS).unwrap();
  assert!(files.files.iter().any(|file| file.status == FileStatus::Renamed) && files.files.iter().any(|file| file.binary));
  assert_eq!(patch.files.len(), files.files.len());
  assert_eq!(read_patch_and_files(git, repo_path(&repo), &subjects[3], &PATCH_LIMITS).unwrap().1.files, []);
}

#[test]
fn the_warm_reads_the_commits_of_a_list_within_its_limits() {
  let lines = |id: &str, additions: u32, deletions: u32| CommitFiles::new(id.to_string(), vec![file("a.txt", FileStatus::Modified, additions, deletions)]);
  let many_files = CommitFiles::new(
    ID_B.to_string(),
    (0..=MAX_WARM_FILES).map(|index| file(&format!("{index}.txt"), FileStatus::Added, 1, 0)).collect(),
  );
  let id_c = "c".repeat(40);
  let id_d = "d".repeat(40);
  let files = [
    lines(ID_A, MAX_WARM_LINES - 1, 1),
    many_files,
    lines(&id_c, MAX_WARM_LINES, 1),
    CommitFiles::new(id_d, Vec::new()),
    lines(&format!("{ID_A}..{ID_B}"), 1, 1),
    lines(ID_A, 1, 1),
  ];
  // over the lines or the files, without changes, a range, a repeat: left out
  assert_eq!(patch_warm_ids(&files), [ID_A.to_string()]);
}

/// `repo_with_every_kind_of_commit` with an empty object cache (and the directory that holds it)
fn repo_with_a_patch_cache() -> (TestRepo, Vec<String>, tempfile::TempDir, ObjectCache) {
  let (repo, ids) = repo_with_every_kind_of_commit();
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(cache_dir.path(), &CacheConfig::default());
  (repo, ids, cache_dir, cache)
}

#[test]
fn patches_are_read_through_the_cache() {
  let (repo, ids, _cache_dir, cache) = repo_with_a_patch_cache();
  let git = repo.git_executor();
  let missing_repository = repo.path().join("missing");
  let missing_repository = missing_repository.to_str().unwrap();

  let subject = DiffSubject::Commit(ids[1].clone());
  let (patch, source) = read_patch_cached(git, repo_path(&repo), &cache, &subject).unwrap();
  assert_eq!(source, PatchSource::Git);
  assert_eq!(patch, read_patch(git, repo_path(&repo), &subject, &PATCH_LIMITS).unwrap());
  // a hit runs no git: a repository path that doesn't exist would fail it
  assert_eq!(read_patch_cached(git, missing_repository, &cache, &subject).unwrap(), (patch, PatchSource::Disk));

  // a range is read, never written to the store: the store is keyed by commits
  let subject = range(&ids[0], &ids[5]);
  let (patch, source) = read_patch_cached(git, repo_path(&repo), &cache, &subject).unwrap();
  assert_eq!((patch.commit_id.as_str(), source), (format!("{}..{}", ids[0], ids[5]).as_str(), PatchSource::Git));
  read_patch_cached(git, missing_repository, &cache, &subject).unwrap_err();

  // the store is read before git, whatever git would say
  let planted = vec![FilePatch {
    patch: "diff --git a/x b/x\n".to_string(),
    omitted_lines: None,
  }];
  cache.put_commit_patches(PATCH_CACHE_VERSION, &[(ids[2].as_str(), &planted)]);
  let (patch, source) = read_patch_cached(git, missing_repository, &cache, &DiffSubject::Commit(ids[2].clone())).unwrap();
  assert_eq!((patch.files, source), (planted, PatchSource::Disk));
}

#[test]
fn the_warm_writes_the_patches_the_cache_misses() {
  let (repo, ids, _cache_dir, cache) = repo_with_a_patch_cache();
  let git = repo.git_executor();
  let commits = &ids[..ids.len() - 1];
  read_patch_cached(git, repo_path(&repo), &cache, &DiffSubject::Commit(ids[0].clone())).unwrap();

  let warm = warm_patches(git, repo_path(&repo), &cache, commits).unwrap();
  assert_eq!(
    warm,
    PatchWarm {
      cached: 1,
      read: commits.len() - 1,
      written: commits.len() - 1
    }
  );
  assert!(cache.has_commit_patches(PATCH_CACHE_VERSION, commits).iter().all(|cached| *cached));
  for id in commits {
    let (patch, source) = read_patch_cached(git, repo_path(&repo), &cache, &DiffSubject::Commit(id.clone())).unwrap();
    assert_eq!(source, PatchSource::Disk, "{id}");
    assert_eq!(patch, read_commit_patch(git, repo_path(&repo), id, &PATCH_LIMITS).unwrap(), "{id}");
  }
  // all cached: no git at all
  assert_eq!(
    warm_patches(git, "/nonexistent", &cache, commits).unwrap(),
    PatchWarm {
      cached: commits.len(),
      read: 0,
      written: 0
    }
  );
  // a disabled cache warms nothing
  assert_eq!(warm_patches(git, repo_path(&repo), &ObjectCache::disabled(), commits).unwrap(), PatchWarm::default());
}

#[test]
fn a_patch_over_the_cache_limit_stays_out_of_the_store() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "# Test\n");
  let line = format!("{}\n", "x".repeat(99));
  // three files under the file limit, together over the cache limit
  let text = line.repeat(9000);
  let files: Vec<(&str, &str)> = ["a.txt", "b.txt", "c.txt"].into_iter().map(|path| (path, text.as_str())).collect();
  let big = repo.create_commit_with_files("Big", &files);
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(cache_dir.path(), &CacheConfig::default());

  let (patch, source) = read_patch_cached(repo.git_executor(), repo_path(&repo), &cache, &DiffSubject::Commit(big.clone())).unwrap();
  assert_eq!(source, PatchSource::Git);
  assert!(patch_bytes(&patch.files) > MAX_CACHED_PATCH_BYTES);
  assert!(patch.files.iter().all(|file| file.omitted_lines.is_none()));
  let warm = warm_patches(repo.git_executor(), repo_path(&repo), &cache, std::slice::from_ref(&big)).unwrap();
  assert_eq!(warm, PatchWarm { cached: 0, read: 1, written: 0 });
  assert_eq!(cache.has_commit_patches(PATCH_CACHE_VERSION, &[big]), [false]);
}
