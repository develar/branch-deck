use crate::diff_scan::{CommitDiff, FilePatch, file_hunks, file_patch, parse_commit_diffs, read_commit_diffs};
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

  let diffs = read_commit_diffs(repo.git_executor(), repo.path_str(), std::slice::from_ref(&commit)).unwrap();
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

/// A commit patch as `read_commit_patches` reads it: a changed file with two hunks (the last line without a newline, an
/// empty context line as `diff.suppressBlankEmpty` prints it), a rename, a binary file, a new file, a mode change and
/// a quoted path
const PATCH: &str = "diff --git a/text file.txt b/text file.txt
index 1111111..2222222 100644
--- a/text file.txt\t
+++ b/text file.txt\t
@@ -1,4 +1,4 @@
 one
-two
+TWO

 four
@@ -9,3 +9,3 @@ fn context()
 nine
 ten
-eleven
\\ No newline at end of file
+ELEVEN
\\ No newline at end of file
diff --git a/old.txt b/new.txt
similarity index 90%
rename from old.txt
rename to new.txt
index 3333333..4444444 100644
--- a/old.txt
+++ b/new.txt
@@ -1 +1 @@
-old
+new
diff --git a/image.png b/image.png
index 5555555..6666666 100644
Binary files a/image.png and b/image.png differ
diff --git a/added.txt b/added.txt
new file mode 100644
index 0000000..7777777
--- /dev/null
+++ b/added.txt
@@ -0,0 +1 @@
+added
diff --git a/run.sh b/run.sh
old mode 100644
new mode 100755
diff --git \"a/tab\\there\" \"b/tab\\there\"
index 8888888..9999999 100644
--- \"a/tab\\there\"
+++ \"b/tab\\there\"
@@ -1 +1 @@
-x
+y
";

#[test]
fn file_patch_numbers_the_hunks_of_one_file() {
  let hunks = file_hunks(PATCH, "text file.txt");
  assert_eq!(
    hunks.iter().map(|hunk| hunk.header.as_str()).collect::<Vec<_>>(),
    ["@@ -1,4 +1,4 @@", "@@ -9,3 +9,3 @@ fn context()"]
  );
  assert_eq!((hunks[0].range, hunks[1].range), (hunk(1, 4, 1, 4), hunk(9, 3, 9, 3)));
  assert_eq!(hunks[0].text(), " one\n-two\n+TWO\n\n four\n");
  assert_eq!(
    hunks[1].lines,
    [" nine", " ten", "-eleven", "\\ No newline at end of file", "+ELEVEN", "\\ No newline at end of file"]
  );
  assert_eq!(file_hunks(PATCH, "missing.txt"), []);
}

#[test]
fn file_patch_tells_renames_binaries_added_files_and_modes() {
  let file = |path| file_patch(PATCH, path).unwrap_or_else(|| panic!("no section of {path}"));
  let renamed = file("new.txt");
  assert_eq!((renamed.renamed, renamed.old_path.as_deref(), renamed.hunks.len()), (true, Some("old.txt"), 1));
  assert_eq!(file("old.txt"), renamed, "found by its old path too");
  let binary = file("image.png");
  assert_eq!((binary.binary, binary.hunks.len(), binary.old_path.as_deref()), (true, 0, Some("image.png")));
  assert_eq!(
    file("added.txt"),
    FilePatch {
      old_path: None,
      new_path: Some("added.txt".to_string()),
      old_mode: None,
      new_mode: Some("100644".to_string()),
      renamed: false,
      copied: false,
      binary: false,
      hunks: file_hunks(PATCH, "added.txt"),
    }
  );
  let mode = file("run.sh");
  assert_eq!((mode.old_mode.as_deref(), mode.new_mode.as_deref(), mode.hunks.len()), (Some("100644"), Some("100755"), 0));
  assert_eq!(file("tab\there").hunks.len(), 1);
}
