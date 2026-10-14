use crate::diff_scan::{CommitDiff, FileChange, Hunk, parse_commit_diffs};
use pretty_assertions::assert_eq;

fn change(status: char, path: &str) -> FileChange {
  FileChange {
    status,
    path: path.to_string(),
    old_path: None,
    additions: 0,
    deletions: 0,
    binary: false,
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
:100644 120000 6666666 7777777 T\0link\0\0\
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
\x1ebbbb\0\n";

  let diffs = parse_commit_diffs(output).unwrap();
  let mut renamed = change('R', "new");
  renamed.old_path = Some("old".to_string());
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
            ..change('M', "text file.txt")
          },
          FileChange {
            binary: true,
            ..change('A', "bin")
          },
          renamed,
          FileChange {
            additions: 1,
            deletions: 1,
            hunks: vec![hunk(1, 1, 0, 0), hunk(0, 0, 1, 1)],
            ..change('T', "link")
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
