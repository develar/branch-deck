//! Find Usages in a review: the names of a local in its file, the names of a word in the files of the review from the
//! texts the diffs read, then those outside the review that the index knows

use crate::accio::{DeclarationAt, ReferenceHit, References, SearchCanceled, SymbolHit, SymbolIndex, TestReach};
use crate::declarations_test::{params, review, single_file_params};
use crate::fixture_test::{commit_all, process_counts, repo_path, unhurried, write};
use crate::model::FoundUsages;
use diff_marks::index::{Columns, Resolved};
use pretty_assertions::assert_eq;
use std::sync::Mutex;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The path, line, column and access of each usage
fn places(found: &FoundUsages) -> Vec<(&str, u32, u32, &str)> {
  found
    .usages
    .iter()
    .map(|it| (it.target.path.as_str(), it.target.line, it.target.column, it.target.kind.as_str()))
    .collect()
}

#[test]
fn the_accio_column_counts_bytes_after_a_utf16_surrogate_pair() {
  use diff_marks::index::Columns;
  // `😀` is two UTF-16 units, one character and four bytes: `count` starts at UTF-16 column 6 from 0.
  assert_eq!(Columns::Bytes.at("val 😀count = 1", 0, 6), Some(9));
  assert_eq!(Columns::Characters.at("val 😀count = 1", 0, 6), Some(6));
  // inside the surrogate pair, or past the line: no column
  assert_eq!(Columns::Bytes.at("val 😀count = 1", 0, 5), None);
  assert_eq!(Columns::Characters.at("val 😀count = 1", 1, 0), None);
  // a hit read back: accio before 0.21 answers the byte from 1, from 0.21 the UTF-16 unit from 0
  assert_eq!(Columns::Bytes.hit("val 😀count = 1", 0, 9), Some(6));
  assert_eq!(Columns::Characters.hit("val 😀count = 1", 0, 6), Some(6));
  assert_eq!(Columns::Bytes.hit("val 😀count = 1", 0, 7), None);
  assert_eq!(Columns::Characters.hit("val 😀count = 1", 0, 5), None);
}

#[test]
fn the_column_units_follow_the_version_of_accio() {
  use diff_marks::index::Columns;
  assert_eq!(Columns::of_version("accio 0.20.0\n"), Columns::Bytes);
  assert_eq!(Columns::of_version("accio 0.21.0 (source)\n"), Columns::Characters);
  assert_eq!(Columns::of_version("accio 1.0.0"), Columns::Characters);
  assert_eq!(Columns::of_version("something else"), Columns::Bytes);
}

#[test]
fn finds_a_word_in_the_files_of_the_review_without_git_and_a_local_in_its_file_only() {
  let (repo, diffs, files) = review();
  let counting = repo.git_executor().with_process_stats("test");
  let add = diffs.find_usages(&counting, None, &params(&repo, &files, "src/Uses.kt", 4, "add")).unwrap();
  assert_eq!(process_counts(&counting), []);
  // the call of the name first, then the files of the review in its order
  assert_eq!(
    (add.name.as_str(), places(&add)),
    (
      "add",
      vec![("src/Uses.kt", 4, 10, "call"), ("lib/Queue.kt", 3, 6, "declaration"), ("src/Store.kt", 3, 6, "declaration")]
    )
  );
  assert_eq!((add.usages[0].text.as_str(), add.usages[0].text_column), ("items.add(Box.LIMIT)", 6));

  let left = diffs.find_usages(repo.git_executor(), None, &params(&repo, &files, "src/Uses.kt", 6, "left")).unwrap();
  assert_eq!(places(&left), [("src/Uses.kt", 5, 8, "declaration"), ("src/Uses.kt", 6, 11, "read")]);
}

/// An index that answers the references of each name from a fixed list, cut short, and records the queries
struct FakeReferences {
  /// The units of the columns it takes and answers
  columns: Columns,
  hits: Vec<ReferenceHit>,
  at_hits: Vec<ReferenceHit>,
  by_name: usize,
  queries: Mutex<Vec<(String, Option<String>)>>,
  places: Mutex<Vec<(Vec<DeclarationAt>, Option<String>)>>,
}

impl SymbolIndex for FakeReferences {
  fn columns(&self) -> Columns {
    self.columns
  }

  fn symbols(&self, _repository_root: &str, _query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Ok(None)
  }

  fn references(&self, _repository_root: &str, name: &str, revision: Option<&str>) -> anyhow::Result<Option<References>> {
    self
      .queries
      .lock()
      .map_err(|error| anyhow::anyhow!("{error}"))?
      .push((name.to_string(), revision.map(str::to_string)));
    Ok(Some(References {
      hits: self.hits.clone(),
      by_name: 0,
      complete: false,
      unavailable: false,
    }))
  }
  /// Find Usages asks no place
  fn references_at(&self, _repository_root: &str, _declarations: &[DeclarationAt], _revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    anyhow::bail!("Find Usages asked a place")
  }

  fn references_at_resolved(&self, _repository_root: &str, declarations: &[DeclarationAt], revision: Option<&str>) -> anyhow::Result<Option<Vec<References>>> {
    self
      .places
      .lock()
      .map_err(|error| anyhow::anyhow!("{error}"))?
      .push((declarations.to_vec(), revision.map(str::to_string)));
    Ok(Some(
      declarations
        .iter()
        .map(|_| References {
          hits: self.at_hits.clone(),
          by_name: self.by_name,
          complete: true,
          unavailable: false,
        })
        .collect(),
    ))
  }

  /// Find Usages asks no tests
  fn tests(&self, _repository_root: &str, _declarations: &[DeclarationAt], _given: &[String], _revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    anyhow::bail!("Find Usages asked for tests")
  }
}

fn reference(path: &str, line: u32, column: u32, kind: &str) -> ReferenceHit {
  ReferenceHit {
    path: path.to_string(),
    line,
    column,
    snippet: String::new(),
    kind: serde_json::from_value(serde_json::Value::String(kind.to_string())).unwrap(),
    resolved: Resolved::Typed,
    end_line: None,
    arguments: None,
    fits: None,
  }
}

fn find_store_add_usages(index: &FakeReferences) -> FoundUsages {
  let (repo, diffs, files) = review();
  write(&repo, "lib/Outside.kt", b"package lib\n\nfun run(s: app.Store) = s.add(1)\n");
  write(&repo, "lib/Namesake.kt", b"package lib\n\nfun run() = add()\n");
  let params = params(&repo, &files, "src/Uses.kt", 4, "add");
  diffs.find_usages(repo.git_executor(), Some(index), &params).unwrap()
}

/// A repository whose `lib/Outside.kt` (with `outside`) and `more` files the review of its last commit lacks, the review
/// adding `Uses.kt`, which calls `outsider` at column 13 of its first line: the params of that call and the commit
fn outside_review(outside: &[u8], more: &[(&str, &[u8])]) -> (TestRepo, crate::service::FileDiffs, crate::model::FindDeclarationParams, String) {
  let repo = TestRepo::new();
  std::fs::create_dir_all(repo.path().join("lib")).unwrap();
  write(&repo, "lib/Outside.kt", outside);
  for (path, text) in more {
    write(&repo, path, text);
  }
  commit_all(&repo, "Initial");
  write(&repo, "Uses.kt", b"fun run() = outsider()\n");
  let commit = commit_all(&repo, "Use it");
  let diffs = unhurried();
  let files = diffs
    .read(
      repo.git_executor(),
      repo_path(&repo),
      &object_cache::cache::ObjectCache::disabled(),
      &crate::model::DiffSource::Commit { id: commit.clone() },
      line_diff::policy::WhitespacePolicy::Default,
    )
    .unwrap();
  let mut params = single_file_params(&repo, &files, &commit);
  params.site.column = 13;
  (repo, diffs, params, commit)
}

#[test]
fn adds_the_usages_outside_the_review_that_the_index_knows_at_the_revision_of_the_side() {
  let (repo, diffs, params, commit) = outside_review(
    b"package lib\n\nfun outsider() = 1\n\nval twice = outsider() + outsider()\n",
    &[("README.md", b"outsider\n")],
  );
  // the file of the review and a text file are not usages the review takes from the index
  let mut name_only = reference("lib/Outside.kt", 5, 13, "call");
  name_only.resolved = Resolved::Name;
  let index = FakeReferences {
    columns: Columns::Bytes,
    hits: vec![
      name_only,
      reference("Uses.kt", 1, 13, "call"),
      reference("README.md", 1, 1, "read"),
      reference("lib/Outside.kt", 3, 5, "declaration"),
      reference("lib/Outside.kt", 5, 26, "argument"),
    ],
    at_hits: Vec::new(),
    by_name: 0,
    queries: Mutex::default(),
    places: Mutex::default(),
  };
  let counting = repo.git_executor().with_process_stats("test");
  let found = diffs.find_usages(&counting, Some(&index), &params).unwrap();
  // the blob at the revision and its text
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 2)]);
  assert_eq!(
    places(&found),
    [
      ("Uses.kt", 0, 12, "call"),
      ("lib/Outside.kt", 2, 4, "declaration"),
      ("lib/Outside.kt", 4, 25, "read"),
      ("lib/Outside.kt", 4, 12, "call")
    ]
  );
  // the second `outsider` of its line, at the column the index answers
  assert_eq!((found.usages[2].text.as_str(), found.usages[2].text_column), ("val twice = outsider() + outsider()", 25));
  assert!(found.more_outside);
  assert_eq!(found.filtered_count, 0);
  let outside = found.usages[1].target.outside.as_ref().unwrap();
  assert_eq!((outside.revision.as_deref(), outside.line_count), (Some(commit.as_str()), 6));
  assert_eq!(*index.queries.lock().unwrap(), [("outsider".to_string(), Some(commit))]);
  assert!(index.places.lock().unwrap().is_empty());
}

/// An accio from 0.21 answers a usage's column in UTF-16 units from 0, as the review counts: no word is looked for on its
/// line, so a namesake before it (`"outsider"` of a string after an emoji) is not taken for it
#[test]
fn an_outside_usage_is_at_the_column_the_index_answers_in_its_units() {
  let (repo, diffs, params, _) = outside_review("package lib\n\nfun outsider() = 1\n\nval x = \"😀outsider\" + outsider()\n".as_bytes(), &[]);
  // `val x = "😀outsider" + ` is 22 UTF-16 units: the call, not the string
  let index = FakeReferences {
    columns: Columns::Characters,
    hits: vec![reference("lib/Outside.kt", 5, 22, "call")],
    at_hits: Vec::new(),
    by_name: 0,
    queries: Mutex::default(),
    places: Mutex::default(),
  };
  let found = diffs.find_usages(repo.git_executor(), Some(&index), &params).unwrap();
  let outside: Vec<(u32, u32, u32)> = found
    .usages
    .iter()
    .filter(|it| it.target.path == "lib/Outside.kt")
    .map(|it| (it.target.line, it.target.column, it.target.end_column))
    .collect();
  assert_eq!(outside, [(4, 22, 30)]);
}

#[test]
fn does_not_list_a_name_only_namesake_when_the_review_has_the_declaration() {
  let mut namesake = reference("lib/Namesake.kt", 3, 13, "call");
  namesake.resolved = Resolved::Name;
  let actual = reference("lib/Outside.kt", 3, 27, "call");
  let index = FakeReferences {
    columns: Columns::Bytes,
    hits: vec![actual.clone(), namesake.clone()],
    at_hits: vec![actual, namesake],
    by_name: 1,
    queries: Mutex::default(),
    places: Mutex::default(),
  };

  let found = find_store_add_usages(&index);
  assert!(places(&found).contains(&("lib/Outside.kt", 2, 26, "call")));
  assert!(!places(&found).contains(&("lib/Namesake.kt", 2, 12, "call")), "{found:?}");
  assert_eq!(found.filtered_count, 1);
  assert_eq!(
    *index.places.lock().unwrap(),
    [(
      vec![DeclarationAt {
        revision: None,
        path: "src/Store.kt".to_string(),
        line: 4,
        column: Some(7),
      }],
      None,
    )]
  );
  assert!(index.queries.lock().unwrap().is_empty());
}

#[test]
fn an_unknown_accio_access_kind_is_shown_as_read() {
  let index = FakeReferences {
    columns: Columns::Bytes,
    hits: Vec::new(),
    at_hits: vec![reference("lib/Outside.kt", 3, 27, "config")],
    by_name: 0,
    queries: Mutex::default(),
    places: Mutex::default(),
  };

  let found = find_store_add_usages(&index);
  assert!(places(&found).contains(&("lib/Outside.kt", 2, 26, "read")));
}

/// An index that fails every query with `error`: accio that exits with an error, or a search the window canceled
struct FailingIndex(fn() -> anyhow::Error);

impl SymbolIndex for FailingIndex {
  fn symbols(&self, _repository_root: &str, _query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Err(self.0())
  }

  fn references(&self, _repository_root: &str, _name: &str, _revision: Option<&str>) -> anyhow::Result<Option<References>> {
    Err(self.0())
  }
  fn references_at(&self, _repository_root: &str, _declarations: &[DeclarationAt], _revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    Err(self.0())
  }

  fn tests(&self, _repository_root: &str, _declarations: &[DeclarationAt], _given: &[String], _revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    Err(self.0())
  }
}

const EXITED: FailingIndex = FailingIndex(|| anyhow::anyhow!("accio exited with code 2"));

#[test]
fn an_index_that_fails_takes_nothing_from_the_review_and_says_why() {
  let (repo, diffs, files) = review();
  let add = diffs
    .find_usages(repo.git_executor(), Some(&EXITED), &params(&repo, &files, "src/Uses.kt", 4, "add"))
    .unwrap();
  assert_eq!((add.usages.len(), add.index_error.as_deref()), (3, Some("accio did not answer: accio exited with code 2")));
  // a declaration the review lacks: none, and why
  let any = diffs
    .find_declaration(
      repo.git_executor(),
      &object_cache::cache::ObjectCache::disabled(),
      Some(&EXITED),
      &params(&repo, &files, "src/Uses.kt", 2, "Store"),
    )
    .unwrap();
  assert_eq!(any.index_error, None, "Store is declared in the review: the index is not asked");
  let int = diffs
    .find_declaration(
      repo.git_executor(),
      &object_cache::cache::ObjectCache::disabled(),
      Some(&EXITED),
      &params(&repo, &files, "src/Uses.kt", 3, "Int"),
    )
    .unwrap();
  assert_eq!((int.targets.len(), int.index_error.as_deref()), (0, Some("accio did not answer: accio exited with code 2")));
}

#[test]
fn a_canceled_search_answers_nothing() {
  let (repo, diffs, files) = review();
  let canceled = FailingIndex(|| anyhow::Error::new(SearchCanceled));
  // the usages of the review are not answered alone, as if the index had none: the window asked for no answer
  let add = diffs
    .find_usages(repo.git_executor(), Some(&canceled), &params(&repo, &files, "src/Uses.kt", 4, "add"))
    .unwrap_err();
  assert!(add.is::<SearchCanceled>(), "{add:#}");
  let int = diffs
    .find_declaration(
      repo.git_executor(),
      &object_cache::cache::ObjectCache::disabled(),
      Some(&canceled),
      &params(&repo, &files, "src/Uses.kt", 3, "Int"),
    )
    .unwrap_err();
  assert!(int.is::<SearchCanceled>(), "{int:#}");
}
