use crate::dependencies::compute_dependencies;
use crate::diff_scan::{CommitDiff, parse_commit_diffs, read_commit_diffs};
use file_diffs::service::FileDiffs;
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

/// The changes of one commit, without caches
pub(crate) fn commit_diffs(repo: &TestRepo, commit: &str) -> Vec<CommitDiff> {
  read_commit_diffs(repo.git_executor(), repo.path_str(), &ObjectCache::disabled(), &FileDiffs::default(), &[commit.to_string()]).unwrap()
}

fn hunk(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> Hunk {
  Hunk {
    old_start,
    old_lines,
    new_start,
    new_lines,
  }
}

/// The raw part: the status, the paths (a rename names the old one first) and the blob ids of each file; the zero id
/// of a side without a file, a symlink and a submodule give no blob. The hunks and the counts come from the blocks
#[test]
fn parses_the_raw_part() {
  let output = b"\x1eaaaa\0\n\
:100644 100644 1111111 2222222 M\0text file.txt\0\
:000000 100644 0000000 3333333 A\0bin\0\
:100644 100644 4444444 5555555 R090\0old\0new\0\
:100644 120000 6666666 7777777 T\0link\0\
:160000 160000 8888888 9999999 M\0sub\0\0\
\x1ebbbb\0\n";

  let diffs = parse_commit_diffs(output).unwrap();
  let blob = |id: &str| Some(id.to_string());
  let mut renamed = change('R', "new");
  renamed.old_path = Some("old".to_string());
  (renamed.old_blob, renamed.new_blob) = (blob("4444444"), blob("5555555"));
  assert_eq!(
    diffs,
    [
      CommitDiff {
        id: "aaaa".to_string(),
        files: vec![
          FileChange {
            old_blob: blob("1111111"),
            new_blob: blob("2222222"),
            ..change('M', "text file.txt")
          },
          FileChange {
            new_blob: blob("3333333"),
            ..change('A', "bin")
          },
          renamed,
          FileChange {
            old_blob: blob("6666666"),
            ..change('T', "link")
          },
          change('M', "sub"),
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

  let diffs = commit_diffs(&repo, &commit);
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

  let diffs = commit_diffs(&repo, &commit);
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
  let plain = read_commit_diffs(repo.git_executor(), path, &ObjectCache::disabled(), &FileDiffs::default(), &ids).unwrap();

  for (key, value) in [("diff.interHunkContext", "10"), ("diff.algorithm", "histogram"), ("diff.indentHeuristic", "false")] {
    repo.set_config(key, value).unwrap();
  }
  let configured = read_commit_diffs(repo.git_executor(), path, &ObjectCache::disabled(), &FileDiffs::default(), &ids).unwrap();

  assert_eq!(configured, plain);
  assert_eq!(configured[1].files[0].hunks, [hunk(5, 1, 5, 1), hunk(15, 1, 15, 1)]);
  assert_eq!(compute_dependencies(&configured), Vec::new());
}
