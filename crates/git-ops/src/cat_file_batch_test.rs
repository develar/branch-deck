use super::{BatchEntry, BatchParser, NO_LIMIT, for_each_object, object_types, read_objects};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// An entry as the tests compare it: `(id, kind, content)`, `(id, kind, size)` for a too large one, `None` for a
/// missing one
#[derive(Debug, PartialEq, Eq)]
enum Seen {
  Object(String, String, Vec<u8>),
  TooLarge(String, String, usize),
  Missing,
}

fn seen(entry: BatchEntry<'_>) -> Seen {
  match entry {
    BatchEntry::Object { id, kind, content } => Seen::Object(id.to_string(), kind.to_string(), content),
    BatchEntry::TooLarge { id, kind, size } => Seen::TooLarge(id.to_string(), kind.to_string(), size),
    BatchEntry::Missing => Seen::Missing,
  }
}

fn blob(content: &[u8]) -> Seen {
  Seen::Object(String::new(), "blob".to_string(), content.to_vec())
}

/// The entries of `names`, the ids left out
fn read(repo: &TestRepo, names: &[&str], max_bytes: usize) -> Vec<Seen> {
  read_objects(repo.git_executor(), repo.path_str(), names, max_bytes, seen)
    .unwrap()
    .into_iter()
    .map(|entry| match entry {
      Seen::Object(_, kind, content) => Seen::Object(String::new(), kind, content),
      Seen::TooLarge(_, kind, size) => Seen::TooLarge(String::new(), kind, size),
      Seen::Missing => Seen::Missing,
    })
    .collect()
}

#[test]
fn names_with_a_space_or_a_newline_and_missing_names() {
  let repo = TestRepo::new();
  repo.create_commit_in_index("files", &[("a b.txt", "space\n"), ("line\nbreak.txt", "newline\n")]);

  let entries = read(&repo, &["HEAD:a b.txt", "HEAD:line\nbreak.txt", "HEAD:no such file", "no such ref"], NO_LIMIT);

  assert_eq!(entries, [blob(b"space\n"), blob(b"newline\n"), Seen::Missing, Seen::Missing]);
}

#[test]
fn the_types_of_paths_with_a_newline_a_directory_a_gitlink_and_missing_names() {
  let repo = TestRepo::new();
  let head = repo.create_commit_in_index("files", &[("line\nbreak.txt", "newline\n"), ("src/main.rs", "fn main() {}\n")]);
  // a gitlink to a commit of another repository, and one to a commit that is here as well
  repo.git(&["update-index", "--add", "--cacheinfo", "160000,0123456789abcdef0123456789abcdef01234567,vendor"]);
  repo.git(&["update-index", "--add", "--cacheinfo", &format!("160000,{head},self")]);
  repo.git(&["commit", "-q", "-m", "gitlinks"]);

  let names = ["HEAD:line\nbreak.txt", "HEAD:src", "HEAD:vendor", "HEAD:self", "HEAD", "HEAD:no such file", "no such ref"];
  let types = object_types(repo.git_executor(), repo.path_str(), &names).unwrap();
  assert_eq!(types, [Some("blob"), Some("tree"), Some("submodule"), Some("commit"), Some("commit"), None, None]);
  assert_eq!(object_types::<&str>(repo.git_executor(), repo.path_str(), &[]).unwrap(), []);
  object_types(repo.git_executor(), repo.path_str(), &["HEAD:a\0b"]).unwrap_err();
}

#[test]
fn content_holding_the_delimiter_is_read_whole() {
  let repo = TestRepo::new();
  let id = repo
    .git_executor()
    .cmd(&["hash-object", "-w", "--stdin"], repo.path_str())
    .stdin(b"a\0b\0\0c\n")
    .text()
    .unwrap();
  let commit = repo.create_commit("text", "text.txt", "text\n");

  let entries = read_objects(repo.git_executor(), repo.path_str(), &[id.as_str(), "HEAD:text.txt", &commit], NO_LIMIT, seen).unwrap();

  assert_eq!(entries[0], Seen::Object(id, "blob".to_string(), b"a\0b\0\0c\n".to_vec()));
  assert_eq!(entries[1], Seen::Object(repo.rev_parse("HEAD:text.txt").unwrap(), "blob".to_string(), b"text\n".to_vec()));
  assert!(matches!(&entries[2], Seen::Object(id, kind, _) if *id == commit && kind == "commit"), "{:?}", entries[2]);
}

#[test]
fn an_object_over_the_limit_is_too_large_and_the_next_ones_are_read() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("files", &[("large.txt", "0123456789\n"), ("small.txt", "abc\n"), ("exact.txt", "abcd")]);

  let entries = read(&repo, &["HEAD:large.txt", "HEAD:small.txt", "HEAD:exact.txt", "HEAD:large.txt"], 4);

  assert_eq!(
    entries,
    [
      Seen::TooLarge(String::new(), "blob".to_string(), 11),
      blob(b"abc\n"),
      blob(b"abcd"),
      Seen::TooLarge(String::new(), "blob".to_string(), 11),
    ]
  );
}

/// The bytes of an object over the limit are skipped as they arrive: whatever its size, the parser holds no more than
/// a header
#[test]
fn the_content_of_an_object_over_the_limit_is_not_buffered() {
  let mut entries = Vec::new();
  let mut parser = BatchParser::new(3, 8, |index, entry| {
    entries.push((index, seen(entry)));
    Ok(())
  });
  let id = "0123456789abcdef0123456789abcdef01234567";
  let size = 10 << 20;
  parser.push(format!("{id} blob {size}\0").as_bytes()).unwrap();
  let chunk = vec![b'x'; 64 << 10];
  let mut most = 0;
  for _ in 0..size / chunk.len() {
    parser.push(&chunk).unwrap();
    most = most.max(parser.content_held());
  }
  // the rest arrives in pieces cut anywhere: within the NUL, the header and the content
  for piece in [&b"\0"[..], b"fedc", b"ba98765432 blob 3", b"\0xy", b"z\0nope mis", b"sing\0"] {
    parser.push(piece).unwrap();
  }
  parser.finish().unwrap();

  assert_eq!(most, 0, "the skipped content was held");
  assert_eq!(
    entries,
    [
      (0, Seen::TooLarge(id.to_string(), "blob".to_string(), size)),
      (1, Seen::Object("fedcba98765432".to_string(), "blob".to_string(), b"xyz".to_vec())),
      (2, Seen::Missing),
    ]
  );
}

#[test]
fn a_truncated_output_and_an_error_of_the_handler_fail() {
  let mut parser = BatchParser::new(1, NO_LIMIT, |_, _| Ok(()));
  parser.push(b"0123 blob 5\0abc").unwrap();
  assert!(parser.finish().is_err(), "the content ended early");

  let repo = TestRepo::new();
  repo.create_commit("file", "file.txt", "text\n");
  let result = for_each_object(repo.git_executor(), repo.path_str(), &["HEAD:file.txt", "HEAD"], NO_LIMIT, |index, _| {
    if index == 1 { Err(anyhow::anyhow!("refused")) } else { Ok(()) }
  });
  assert_eq!(result.unwrap_err().to_string(), "refused");
}
