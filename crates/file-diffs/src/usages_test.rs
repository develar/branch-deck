//! Find Usages in a review: the names of a local in its file, the names of a word in the files of the review from the
//! texts the diffs read, then those outside the review that the index knows

use crate::accio::{ReferenceHit, References, SymbolHit, SymbolIndex};
use crate::declarations_test::{params, review, single_file_params};
use crate::fixture_test::{commit_all, process_counts, repo_path, write};
use crate::model::FoundUsages;
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
  hits: Vec<ReferenceHit>,
  queries: Mutex<Vec<(String, Option<String>)>>,
}

impl SymbolIndex for FakeReferences {
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
      complete: false,
    }))
  }
}

fn reference(path: &str, line: u32, column: u32, kind: &str) -> ReferenceHit {
  ReferenceHit {
    path: path.to_string(),
    line,
    column,
    snippet: String::new(),
    kind: kind.to_string(),
  }
}

#[test]
fn adds_the_usages_outside_the_review_that_the_index_knows_at_the_revision_of_the_side() {
  let repo = TestRepo::new();
  std::fs::create_dir_all(repo.path().join("lib")).unwrap();
  write(&repo, "lib/Outside.kt", b"package lib\n\nfun outsider() = 1\n\nval twice = outsider() + outsider()\n");
  write(&repo, "README.md", b"outsider\n");
  commit_all(&repo, "Initial");
  write(&repo, "Uses.kt", b"fun run() = outsider()\n");
  let commit = commit_all(&repo, "Use it");
  let diffs = crate::service::FileDiffs::default();
  let files = diffs
    .read(
      repo.git_executor(),
      repo_path(&repo),
      &object_cache::cache::ObjectCache::disabled(),
      &crate::model::DiffSource::Commit { id: commit.clone() },
      text_diff::policy::WhitespacePolicy::Default,
    )
    .unwrap();
  let mut params = single_file_params(&repo, &files, &commit);
  params.site.column = 13;
  // the file of the review and a text file are not usages the review takes from the index
  let index = FakeReferences {
    hits: vec![
      reference("Uses.kt", 1, 13, "call"),
      reference("README.md", 1, 1, "read"),
      reference("lib/Outside.kt", 3, 5, "declaration"),
      reference("lib/Outside.kt", 5, 26, "argument"),
    ],
    queries: Mutex::default(),
  };
  let counting = repo.git_executor().with_process_stats("test");
  let found = diffs.find_usages(&counting, Some(&index), &params).unwrap();
  // the blob at the revision and its text
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 2)]);
  assert_eq!(
    places(&found),
    [("Uses.kt", 0, 12, "call"), ("lib/Outside.kt", 2, 4, "declaration"), ("lib/Outside.kt", 4, 25, "read")]
  );
  // the second `outsider` of its line, nearest to the column of the index
  assert_eq!((found.usages[2].text.as_str(), found.usages[2].text_column), ("val twice = outsider() + outsider()", 25));
  assert!(found.more_outside);
  let outside = found.usages[1].target.outside.as_ref().unwrap();
  assert_eq!((outside.revision.as_deref(), outside.line_count), (Some(commit.as_str()), 6));
  assert_eq!(*index.queries.lock().unwrap(), [("outsider".to_string(), Some(commit))]);
}

/// An index that fails every query, as accio that does not answer in time
struct FailingIndex;

impl SymbolIndex for FailingIndex {
  fn symbols(&self, _repository_root: &str, _query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    anyhow::bail!("no answer in 5 s")
  }

  fn references(&self, _repository_root: &str, _name: &str, _revision: Option<&str>) -> anyhow::Result<Option<References>> {
    anyhow::bail!("no answer in 5 s")
  }
}

#[test]
fn an_index_that_does_not_answer_takes_nothing_from_the_review_and_says_why() {
  let (repo, diffs, files) = review();
  let add = diffs
    .find_usages(repo.git_executor(), Some(&FailingIndex), &params(&repo, &files, "src/Uses.kt", 4, "add"))
    .unwrap();
  assert_eq!((add.usages.len(), add.index_error.as_deref()), (3, Some("accio did not answer: no answer in 5 s")));
  // a declaration the review lacks: none, and why
  let any = diffs
    .find_declaration(
      repo.git_executor(),
      &object_cache::cache::ObjectCache::disabled(),
      Some(&FailingIndex),
      &params(&repo, &files, "src/Uses.kt", 2, "Store"),
    )
    .unwrap();
  assert_eq!(any.index_error, None, "Store is declared in the review: the index is not asked");
  let int = diffs
    .find_declaration(
      repo.git_executor(),
      &object_cache::cache::ObjectCache::disabled(),
      Some(&FailingIndex),
      &params(&repo, &files, "src/Uses.kt", 3, "Int"),
    )
    .unwrap();
  assert_eq!((int.targets.len(), int.index_error.as_deref()), (0, Some("accio did not answer: no answer in 5 s")));
}
