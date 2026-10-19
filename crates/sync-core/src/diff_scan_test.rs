use crate::dependencies::compute_dependencies;
use crate::diff_scan::{CommitDiff, parse_commit_diffs, read_commit_diffs};
use object_cache::cache::ObjectCache;
use object_cache::commit_diff::{FileChange, Hunk};
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

fn change(status: char, path: &str) -> FileChange {
  FileChange {
    status,
    path: path.to_string(),
    old_path: None,
    additions: 0,
    deletions: 0,
    binary: false,
    old_blob: None,
    new_blob: None,
    hunks: Vec::new(),
  }
}

fn hunk(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> Hunk {
  Hunk {
    old_start,
    old_lines,
    new_start,
    new_lines,
  }
}

#[test]
fn parses_raw_and_patch_parts() {
  let output = b"\x1eaaaa\0\n\
:100644 100644 1111111 2222222 M\0text file.txt\0\
:000000 100644 0000000 3333333 A\0bin\0\
:100644 100644 4444444 5555555 R090\0old\0new\0\
:100644 120000 6666666 7777777 T\0link\0\
:160000 160000 8888888 9999999 M\0sub\0\0\
diff --git a/text file.txt b/text file.txt\n\
index 1111111..2222222 100644\n\
--- a/text file.txt\t\n\
+++ b/text file.txt\t\n\
@@ -2 +2 @@ context\n\
-b\n\
+B\n\
@@ -5,0 +6,2 @@\n\
+x\n\
+y\n\
\\ No newline at end of file\n\
diff --git a/bin b/bin\n\
new file mode 100644\n\
index 0000000..3333333\n\
Binary files /dev/null and b/bin differ\n\
diff --git a/old b/new\n\
similarity index 90%\n\
rename from old\n\
rename to new\n\
--- a/old\n\
+++ b/new\n\
@@ -3,2 +3 @@\n\
-- a removed line that starts with a dash\n\
-+ and one with a plus\n\
+--- an added line that looks like a header\n\
diff --git a/link b/link\n\
deleted file mode 100644\n\
--- a/link\n\
+++ /dev/null\n\
@@ -1 +0,0 @@\n\
-target\n\
diff --git a/link b/link\n\
new file mode 120000\n\
--- /dev/null\n\
+++ b/link\n\
@@ -0,0 +1 @@\n\
+target\n\
\\ No newline at end of file\n\
diff --git a/sub b/sub\n\
index 8888888..9999999 160000\n\
--- a/sub\n\
+++ b/sub\n\
@@ -1 +1 @@\n\
-Subproject commit 8888888\n\
+Subproject commit 9999999\n\
\x1ebbbb\0\n";

  let diffs = parse_commit_diffs(output).unwrap();
  let blob = |id: &str| Some(id.to_string());
  let mut renamed = change('R', "new");
  renamed.old_path = Some("old".to_string());
  (renamed.old_blob, renamed.new_blob) = (blob("4444444"), blob("5555555"));
  renamed.additions = 1;
  renamed.deletions = 2;
  renamed.hunks = vec![hunk(3, 2, 3, 1)];
  assert_eq!(
    diffs,
    [
      CommitDiff {
        id: "aaaa".to_string(),
        files: vec![
          FileChange {
            additions: 3,
            deletions: 1,
            hunks: vec![hunk(2, 1, 2, 1), hunk(5, 0, 6, 2)],
            old_blob: blob("1111111"),
            new_blob: blob("2222222"),
            ..change('M', "text file.txt")
          },
          // the zero id of the side without a file
          FileChange {
            binary: true,
            new_blob: blob("3333333"),
            ..change('A', "bin")
          },
          renamed,
          // a symlink has no source to read, nor has a submodule
          FileChange {
            additions: 1,
            deletions: 1,
            hunks: vec![hunk(1, 1, 0, 0), hunk(0, 0, 1, 1)],
            old_blob: blob("6666666"),
            ..change('T', "link")
          },
          FileChange {
            additions: 1,
            deletions: 1,
            hunks: vec![hunk(1, 1, 1, 1)],
            ..change('M', "sub")
          },
        ],
      },
      CommitDiff {
        id: "bbbb".to_string(),
        files: Vec::new(),
      },
    ]
  );
}

/// The raw part gives the full ids of the blobs before and after: a modified file both, a renamed one the blob of its
/// old path, an added one the new blob only
#[test]
fn read_commit_diffs_gives_the_full_blob_ids() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("base", &[("kept.txt", "one\n"), ("moved.txt", "a\nb\nc\nd\ne\n")]);
  let base = repo.head();
  let git = |args: &[&str]| repo.git_executor().execute_command(args, repo.path_str()).unwrap();
  git(&["mv", "moved.txt", "renamed.txt"]);
  let commit = repo.create_commit_with_files("change", &[("kept.txt", "two\n"), ("added.txt", "new\n")]);
  let blob = |commit: &str, path: &str| Some(git(&["rev-parse", &format!("{commit}:{path}")]));

  let diffs = read_commit_diffs(repo.git_executor(), repo.path_str(), &ObjectCache::disabled(), std::slice::from_ref(&commit)).unwrap();
  let blobs: Vec<(&str, Option<String>, Option<String>)> = diffs[0]
    .files
    .iter()
    .map(|file| (file.path.as_str(), file.old_blob.clone(), file.new_blob.clone()))
    .collect();
  assert_eq!(
    blobs,
    [
      ("added.txt", None, blob(&commit, "added.txt")),
      ("kept.txt", blob(&base, "kept.txt"), blob(&commit, "kept.txt")),
      ("renamed.txt", blob(&base, "moved.txt"), blob(&commit, "renamed.txt")),
    ]
  );
  assert_eq!(blob(&commit, "kept.txt").map(|id| id.len()), Some(40), "a full id");
}

/// The hunks and the counts are the blocks of the diff views: a final line end added is a block on the empty last line,
/// not a changed line `c`; a deleted file counts its lines; a change of only the line ends, which the views show
/// without a block, is the whole file; a binary file has no hunks
#[test]
fn read_commit_diffs_takes_the_blocks_of_the_views() {
  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "base",
    &[("lines.txt", "a\nb\nc"), ("crlf.txt", "a\r\nb\r\n"), ("gone.txt", "x\ny\n"), ("data.bin", "\0one")],
  );
  repo.git(&["rm", "-q", "gone.txt"]);
  let commit = repo.create_commit_with_files("change", &[("lines.txt", "a\nB\nc\n"), ("crlf.txt", "a\nb\n"), ("data.bin", "\0two")]);

  let diffs = read_commit_diffs(repo.git_executor(), repo.path_str(), &ObjectCache::disabled(), std::slice::from_ref(&commit)).unwrap();
  let files: Vec<(&str, u32, u32, bool, &[Hunk])> = diffs[0]
    .files
    .iter()
    .map(|file| (file.path.as_str(), file.additions, file.deletions, file.binary, file.hunks.as_slice()))
    .collect();
  assert_eq!(
    files,
    [
      ("crlf.txt", 0, 0, false, [hunk(1, 3, 1, 3)].as_slice()),
      ("data.bin", 0, 0, true, [].as_slice()),
      ("gone.txt", 0, 2, false, [hunk(1, 2, 0, 0)].as_slice()),
      ("lines.txt", 1, 1, false, [hunk(2, 1, 2, 1), hunk(3, 0, 4, 1)].as_slice()),
    ]
  );
}

/// The hunks, and so the dependency edges, are the same whatever the user's diff config: with `diff.interHunkContext`
/// the two changes of `B`, ten lines apart, would fuse into one hunk over the line `A` changed, an edge that isn't there
#[test]
fn the_diff_config_of_the_user_changes_neither_the_hunks_nor_the_edges() {
  let repo = TestRepo::new();
  let lines = |changed: &[usize]| -> String {
    (1..=20)
      .map(|n| if changed.contains(&n) { format!("changed {n}\n") } else { format!("line {n}\n") })
      .collect()
  };
  repo.create_commit("Initial commit", "file.txt", &lines(&[]));
  let ids = vec![
    repo.create_commit("A", "file.txt", &lines(&[10])),
    repo.create_commit("B", "file.txt", &lines(&[5, 10, 15])),
  ];
  let path = repo.path().to_str().unwrap();
  let plain = read_commit_diffs(repo.git_executor(), path, &ObjectCache::disabled(), &ids).unwrap();

  for (key, value) in [("diff.interHunkContext", "10"), ("diff.algorithm", "histogram"), ("diff.indentHeuristic", "false")] {
    repo.set_config(key, value).unwrap();
  }
  let configured = read_commit_diffs(repo.git_executor(), path, &ObjectCache::disabled(), &ids).unwrap();

  assert_eq!(configured, plain);
  assert_eq!(configured[1].files[0].hunks, [hunk(5, 1, 5, 1), hunk(15, 1, 15, 1)]);
  assert_eq!(compute_dependencies(&configured), Vec::new());
}
