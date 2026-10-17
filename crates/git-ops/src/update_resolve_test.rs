use crate::merge_tree_output::parse_merge_tree_output;
use crate::update_resolve::{ConflictFile, ConflictKind, ResolveError, SessionState, SessionStatus, conflict_files, follow_head, is_valid_tree_path};
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

/// An id of no object
const MISSING: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn tree_paths_that_could_leave_a_directory_are_invalid() {
  for path in ["a.txt", "dir/a.txt", ".github/x", "a..b", "...", "x.gitignore", "git~2", ".gitx"] {
    assert!(is_valid_tree_path(path), "{path}");
  }
  for path in [
    "",
    "../secret",
    "a/../../b",
    "./a",
    "a//b",
    "a/",
    "/abs",
    ".git/hooks/post-checkout",
    "x/.GIT/config",
    "nul\0",
    "GIT~1/config",
    "x/.git./config",
    "x/.git /config",
    ".g\u{200c}it/hooks/pre-commit",
  ] {
    assert!(!is_valid_tree_path(path), "{path:?}");
  }
}

fn file(key: usize, path: &str) -> ConflictFile {
  ConflictFile {
    key,
    path: path.to_string(),
    kind: ConflictKind::Content,
    before: None,
    onto: None,
    commit: None,
    mode: String::new(),
    renamed_from: None,
  }
}

#[test]
fn a_long_basename_is_cut_and_keeps_its_extension() {
  let long = format!("dir/{}.txt", "é".repeat(200));
  let name = file(3, &long).copy_name();
  assert!(name.len() <= 3 + 1 + 100, "{}", name.len());
  assert!(name.starts_with("3-é"), "{name}");
  assert_eq!(std::path::Path::new(&name).extension().and_then(|e| e.to_str()), Some("txt"));
}

#[test]
fn copies_have_flat_names_that_never_collide() {
  // the same name in two directories, and two names that differ only in case (one file on macOS and Windows)
  let names: Vec<String> = [file(1, "a/File.txt"), file(2, "b/File.txt"), file(3, "file.txt"), file(4, "c/we:ird?.txt")]
    .iter()
    .map(ConflictFile::copy_name)
    .collect();
  assert_eq!(names, ["1-File.txt", "2-File.txt", "3-file.txt", "4-we_ird_.txt"]);
  let lowercase: std::collections::HashSet<String> = names.iter().map(|name| name.to_lowercase()).collect();
  assert_eq!(lowercase.len(), names.len());
}

#[test]
fn only_text_and_modify_delete_conflicts_are_supported() {
  let output = b"tree\0\
100644 b1 1\tf.txt\x00100644 b2 2\tf.txt\x00100644 b3 3\tf.txt\0\
100644 m1 1\tmd.txt\x00100644 m3 3\tmd.txt\0\
100644 a2 2\tadded.txt\x00100644 a3 3\tadded.txt\0\0\
1\0f.txt\0CONFLICT (contents)\0x\n\0\
1\0md.txt\0CONFLICT (modify/delete)\0x\n\0\
1\0added.txt\0CONFLICT (add/add)\0x\n\0";
  let files = conflict_files(&parse_merge_tree_output(output).unwrap()).unwrap();
  let kinds: Vec<(&str, ConflictKind, bool)> = files.iter().map(|file| (file.path.as_str(), file.kind, file.before.is_some())).collect();
  assert_eq!(
    kinds,
    [
      ("f.txt", ConflictKind::Content, true),
      ("md.txt", ConflictKind::ModifyDelete, true),
      ("added.txt", ConflictKind::Content, false)
    ]
  );
  // a rename on one side and a delete on the other: the stages are at the new path
  let renamed =
    conflict_files(&parse_merge_tree_output(b"tree\x00100644 r1 1\ty.txt\x00100644 r1 2\ty.txt\0\x002\0y.txt\0x.txt\0CONFLICT (rename/delete)\0x\n\0").unwrap()).unwrap();
  assert_eq!(
    (renamed[0].kind, renamed[0].renamed_from.as_deref(), renamed[0].is_named("x.txt")),
    (ConflictKind::RenameDelete, Some("x.txt"), true)
  );

  let refused = |output: &[u8]| {
    let error = conflict_files(&parse_merge_tree_output(output).unwrap()).unwrap_err();
    match error.downcast::<ResolveError>().unwrap() {
      ResolveError::Unsupported { paths, .. } => paths,
      other => panic!("{other:?}"),
    }
  };
  assert_eq!(
    refused(b"tree\x00100644 b1 1\tbin\x00100644 b2 2\tbin\x00100644 b3 3\tbin\0\x00001\0bin\0CONFLICT (binary)\0x\n\0"),
    ["bin"]
  );
  assert_eq!(refused(b"tree\x00120000 l1 1\tlink\x00120000 l2 2\tlink\x00120000 l3 3\tlink\0\0"), ["link"]);
  // a mode that one side changed is merged as git does; added on both sides with two modes, it can't be
  let merged = conflict_files(&parse_merge_tree_output(b"tree\x00100644 x1 1\tx\x00100644 x2 2\tx\x00100755 x3 3\tx\0\0").unwrap()).unwrap();
  assert_eq!(merged[0].mode, "100755");
  assert_eq!(refused(b"tree\x00100644 x2 2\tx\x00100755 x3 3\tx\0\0"), ["x"]);
  assert_eq!(
    refused(b"tree\x00100644 t1 1\t../escape\x00100644 t2 2\t../escape\x00100644 t3 3\t../escape\0\0"),
    ["../escape"]
  );
  assert_eq!(
    refused(b"tree\x00100644 r1 1\tr~x\x00100644 r3 3\tr~x\0\x00002\0r~x\0r\0CONFLICT (file/directory)\0x\n\x00001\0r~x\0CONFLICT (modify/delete)\0x\n\0"),
    ["r~x", "r"]
  );
}

/// A session of `master` in `repo` that knows HEAD as `head`, before its first round
fn session_state(repo: &TestRepo, head: &str) -> SessionState {
  SessionState {
    version: 1,
    worktree: "main".to_string(),
    git_dir: repo.path().join(".git").to_str().unwrap().to_string(),
    branch: "master".to_string(),
    upstream: "origin/master".to_string(),
    base: head.to_string(),
    upstream_head: head.to_string(),
    excluded: Vec::new(),
    committer: "Test User <test@example.com> 0 +0000".to_string(),
    dir: None,
    dir_created: false,
    started_head: head.to_string(),
    head: head.to_string(),
    local: Vec::new(),
    likely_landed: Vec::new(),
    replayed: 0,
    tip: head.to_string(),
    round: 0,
    status: SessionStatus::Resolving,
    conflict: None,
    resolutions: Vec::new(),
    notes_before: None,
    landed_dropped: Vec::new(),
    run_starts: Vec::new(),
  }
}

#[test]
fn a_session_head_git_cannot_read_is_an_error_not_a_rewrite() {
  let repo = TestRepo::new();
  repo.create_commit("one", "a.txt", "a\n");
  // `merge-base --is-ancestor` exits with 1 for "no"; 128 here, for an object that is not there
  let error = follow_head(repo.git_executor(), repo.path_str(), &mut session_state(&repo, MISSING), None).unwrap_err();
  assert!(error.downcast_ref::<ResolveError>().is_none(), "{error:#}");
  assert!(format!("{error:#}").contains(MISSING), "{error:#}");

  let head = repo.head();
  let mut state = session_state(&repo, &head);
  let rewritten = repo.create_commit("two", "a.txt", "b\n");
  repo.git(&["commit", "--amend", "-m", "two, amended"]);
  state.head = rewritten;
  assert!(matches!(
    follow_head(repo.git_executor(), repo.path_str(), &mut state, None).unwrap_err().downcast::<ResolveError>(),
    Ok(ResolveError::Stale { .. })
  ));
}
