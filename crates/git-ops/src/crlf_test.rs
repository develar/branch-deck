//! The safer autocrlf of git, held to what `git add` stages.

use crate::crlf::{Conversion, LineEnds, conversions, is_crlf_text};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The conversion of `win.txt` under `attributes` in `repo`
fn conversion_of(repo: &TestRepo) -> Conversion {
  conversions(repo.git_executor(), repo.path_str(), ["win.txt"]).unwrap()["win.txt"]
}

/// A repository whose `win.txt` was committed with CRLF, `attributes` written after it (none for an empty line), then
/// an edit with CRLF staged by `git add`: whether `git add` kept the CR, and whether the rule says git keeps such a blob
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
  (by_git, conversion_of(&repo).keeps_a_crlf_blob())
}

/// The rule says what `git add` does: the bytes kept under `text=auto`, `-text`, without `text` and `eol` (also with
/// `core.autocrlf`), converted under `text`, `text=input` and an `eol` alone; the old `crlf` where `text` is unspecified
#[test]
fn a_blob_with_crlf_is_kept_where_git_add_keeps_it() {
  for (attributes, autocrlf, expected) in [
    ("*.txt text=auto eol=lf", false, true),
    ("*.txt text=auto eol=crlf", false, true),
    ("*.txt text=auto", false, true),
    ("*.txt -text", false, true),
    ("*.txt -text eol=lf", false, true),
    ("", false, true),
    ("", true, true),
    ("*.txt text eol=lf", false, false),
    ("*.txt text eol=crlf", false, false),
    ("*.txt text=input", false, false),
    ("*.txt eol=lf", false, false),
    ("*.txt crlf", false, false),
    ("*.txt crlf=input", false, false),
    ("*.txt -crlf", false, true),
    ("*.txt crlf=auto", false, true),
    ("*.txt text=auto crlf", false, true),
  ] {
    assert_eq!(kept(attributes, autocrlf), (expected, expected), "{attributes:?} autocrlf={autocrlf}");
  }
}

/// `ident`, a `filter` and a `working-tree-encoding` change the bytes besides the line ends, which the rule does not
/// model: their conversion says so, and no such blob counts as kept, though `git add` keeps the CRLF beside `ident`
#[test]
fn ident_a_filter_and_an_encoding_are_told_apart() {
  assert_eq!(kept("*.txt text=auto eol=lf ident", false), (true, false));
  for (attributes, line_ends) in [
    ("*.txt text=auto ident", LineEnds::Auto),
    ("*.txt filter=lfs", LineEnds::Auto),
    ("*.txt -text working-tree-encoding=UTF-16", LineEnds::AsTheyAre),
    ("*.txt text ident", LineEnds::Converted),
  ] {
    let repo = TestRepo::new();
    repo.create_commit("Add the attributes", ".gitattributes", &format!("{attributes}\n"));
    assert_eq!(conversion_of(&repo), Conversion { line_ends, other: true }, "{attributes}");
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
