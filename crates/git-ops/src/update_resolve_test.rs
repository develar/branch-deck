use crate::merge_tree_output::parse_merge_tree_output;
use crate::update_resolve::{ConflictFile, ConflictKind, ResolveError, conflict_files, is_valid_tree_path};
use pretty_assertions::assert_eq;

#[test]
fn tree_paths_that_could_leave_a_directory_are_invalid() {
  for path in ["a.txt", "dir/a.txt", ".github/x", "a..b", "...", "x.gitignore"] {
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
  }
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
  assert_eq!(refused(b"tree\x00100644 x1 1\tx\x00100644 x2 2\tx\x00100755 x3 3\tx\0\0"), ["x"]);
  assert_eq!(
    refused(b"tree\x00100644 t1 1\t../escape\x00100644 t2 2\t../escape\x00100644 t3 3\t../escape\0\0"),
    ["../escape"]
  );
  assert_eq!(
    refused(b"tree\x00100644 r1 1\tr~x\x00100644 r3 3\tr~x\0\x00002\0r~x\0r\0CONFLICT (file/directory)\0x\n\x00001\0r~x\0CONFLICT (modify/delete)\0x\n\0"),
    ["r~x", "r"]
  );
}
