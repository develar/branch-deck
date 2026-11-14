//! The safer autocrlf of git, held to what `git add` stages.

use crate::crlf::{crlf_kept_paths, is_crlf_text};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A repository whose `win.txt` was committed with CRLF, `attributes` written after it (none for an empty line), then
/// an edit with CRLF staged by `git add`: whether `git add` kept the CR, and whether `crlf_kept_paths` names the path
fn kept(attributes: &str, autocrlf: bool) -> (bool, bool) {
  let repo = TestRepo::new();
  repo.create_commit("Add win.txt", "win.txt", "one\r\ntwo\r\n");
  if !attributes.is_empty() {
    repo.create_commit("Add the attributes", ".gitattributes", &format!("{attributes}\n"));
  }
  if autocrlf {
    repo.git(&["config", "core.autocrlf", "true"]);
  }
  std::fs::write(repo.path().join("win.txt"), "one\r\ntwo\r\nthree\r\n").unwrap();
  repo.git(&["add", "win.txt"]);
  let staged = repo.git_executor().cmd(&["cat-file", "blob", ":win.txt"], repo.path_str()).bytes().unwrap();
  let by_git = staged.contains(&b'\r');
  let by_rule = crlf_kept_paths(repo.git_executor(), repo.path_str(), ["win.txt"]).unwrap().contains("win.txt");
  (by_git, by_rule)
}

/// The rule says what `git add` does: the bytes kept under `text=auto`, `-text`, without `text` and `eol` (also with
/// `core.autocrlf`), converted under `text` and an `eol` alone
#[test]
fn a_blob_with_crlf_is_kept_where_git_add_keeps_it() {
  for (attributes, autocrlf, expected) in [
    ("*.txt text=auto eol=lf", false, true),
    ("*.txt text=auto", false, true),
    ("*.txt -text", false, true),
    ("", false, true),
    ("", true, true),
    ("*.txt text eol=lf", false, false),
    ("*.txt text eol=crlf", false, false),
    ("*.txt eol=lf", false, false),
  ] {
    assert_eq!(kept(attributes, autocrlf), (expected, expected), "{attributes:?} autocrlf={autocrlf}");
  }
}

/// `ident`, a `filter` and a `working-tree-encoding` change the bytes besides the line ends, which the rule does not
/// model: it calls none of them kept, though `git add` keeps the CRLF beside `ident`
#[test]
fn ident_a_filter_and_an_encoding_are_never_kept() {
  assert_eq!(kept("*.txt text=auto eol=lf ident", false), (true, false));
  for attributes in ["*.txt filter=lfs", "*.txt working-tree-encoding=UTF-16"] {
    let repo = TestRepo::new();
    repo.create_commit("Add the attributes", ".gitattributes", &format!("{attributes}\n"));
    assert!(crlf_kept_paths(repo.git_executor(), repo.path_str(), ["win.txt"]).unwrap().is_empty(), "{attributes}");
  }
}

/// Text with CRLF, by git's statistics: a NUL or a lone CR makes it binary
#[test]
fn crlf_text_is_text_with_a_crlf() {
  assert!(is_crlf_text(b"one\r\ntwo\r\n"));
  assert!(is_crlf_text(b"one\r\ntwo\n\x1a"));
  assert!(!is_crlf_text(b"one\ntwo\n"));
  assert!(!is_crlf_text(b"one\r\ntwo\0"));
  assert!(!is_crlf_text(b"one\r\ntwo\rthree"));
  assert!(!is_crlf_text(b""));
}
