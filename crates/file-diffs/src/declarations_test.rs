//! Going to a declaration in a review: by the scopes of the file, in the other files of the review ranked by the owner,
//! from the texts the diffs read

use crate::accio::{References, SymbolHit, SymbolIndex};
use crate::fixture_test::{commit_all, process_counts, repo_path, unhurried, write};
use crate::model::{BlobRequest, DeclarationSite, DiffFile, DiffRevisions, DiffSide, DiffSource, FindDeclarationParams, ReviewFileSides};
use crate::service::FileDiffs;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use text_diff::policy::WhitespacePolicy;

const USES: &str = "package app\n\nclass Uses(val items: Store) {\n  fun run(times: Int) {\n    items.add(Box.LIMIT)\n    val left = times\n    helper(left)\n  }\n}\n";
const STORE: &str = "package app\n\nclass Store {\n  fun add(value: Int) {}\n}\n\nclass Box {\n  companion object { const val LIMIT = 10 }\n}\n\nfun helper(value: Int) = value\n";
const OTHER: &str = "package other\n\nclass Queue {\n  fun add(value: Int) {}\n}\n";

/// A commit that adds the three files, and the diffs a review of it read
pub(crate) fn review() -> (TestRepo, FileDiffs, Vec<DiffFile>) {
  let repo = TestRepo::new();
  write(&repo, "README.md", b"root\n");
  commit_all(&repo, "Initial");
  for directory in ["src", "lib"] {
    std::fs::create_dir_all(repo.path().join(directory)).unwrap();
  }
  write(&repo, "src/Uses.kt", USES.as_bytes());
  write(&repo, "src/Store.kt", STORE.as_bytes());
  write(&repo, "lib/Queue.kt", OTHER.as_bytes());
  let commit = commit_all(&repo, "Add the files");
  let diffs = unhurried();
  let files = diffs
    .read(
      repo.git_executor(),
      repo_path(&repo),
      &ObjectCache::disabled(),
      &DiffSource::Commit { id: commit },
      WhitespacePolicy::Default,
    )
    .unwrap();
  (repo, diffs, files)
}

/// The params of a name on `line` at the start of `needle` in the new side of `path`
pub(crate) fn params(repo: &TestRepo, files: &[DiffFile], path: &str, line: u32, needle: &str) -> FindDeclarationParams {
  let file = files.iter().find(|it| it.path == path).unwrap();
  let text = match path {
    "src/Uses.kt" => USES,
    "src/Store.kt" => STORE,
    _ => OTHER,
  };
  let column = text.lines().nth(line as usize).unwrap().find(needle).unwrap();
  FindDeclarationParams {
    repository_path: repo_path(repo).to_string(),
    site: DeclarationSite {
      path: path.to_string(),
      side: DiffSide::New,
      blob: BlobRequest {
        id: file.new_id.clone().unwrap(),
        path: None,
      },
      line,
      column: u32::try_from(column).unwrap(),
    },
    files: files
      .iter()
      .map(|it| ReviewFileSides {
        path: it.path.clone(),
        old_path: it.old_path.clone(),
        old_id: it.old_id.clone(),
        new_id: it.new_id.clone(),
        worktree: false,
      })
      .collect(),
    revisions: DiffRevisions::default(),
  }
}

/// The path, line and column of each target
fn targets(diffs: &FileDiffs, repo: &TestRepo, params: &FindDeclarationParams) -> Vec<(String, u32, u32, String)> {
  diffs
    .find_declaration(repo.git_executor(), &ObjectCache::disabled(), None, params)
    .unwrap()
    .targets
    .into_iter()
    .map(|it| (it.path, it.line, it.column, it.kind))
    .collect()
}

#[test]
fn goes_to_a_constant_of_a_type_in_another_file_of_the_review_without_git() {
  let (repo, diffs, files) = review();
  let counting = repo.git_executor().with_process_stats("test");
  let found = diffs
    .find_declaration(&counting, &ObjectCache::disabled(), None, &params(&repo, &files, "src/Uses.kt", 4, "LIMIT"))
    .unwrap();
  assert_eq!(process_counts(&counting), []);
  assert_eq!(found.name, "LIMIT");
  assert_eq!(
    found.targets.iter().map(|it| (it.path.as_str(), it.line, it.column, it.kind.as_str())).collect::<Vec<_>>(),
    [("src/Store.kt", 7, 31, "constant")]
  );
  // a function of the file of the review
  assert_eq!(
    targets(&diffs, &repo, &params(&repo, &files, "src/Uses.kt", 6, "helper")),
    [("src/Store.kt".to_string(), 10, 4, "function".to_string())]
  );
}

#[test]
fn prefers_the_member_of_the_type_a_property_is_declared_with() {
  let (repo, diffs, files) = review();
  let found = targets(&diffs, &repo, &params(&repo, &files, "src/Uses.kt", 4, "add"));
  // `items: Store`: the `add` of `Store` first, the one of `Queue` after it
  assert_eq!(
    found,
    [
      ("src/Store.kt".to_string(), 3, 6, "function".to_string()),
      ("lib/Queue.kt".to_string(), 3, 6, "function".to_string())
    ]
  );
}

#[test]
fn goes_to_a_local_by_the_scopes_and_nowhere_from_a_declaration() {
  let (repo, diffs, files) = review();
  assert_eq!(
    targets(&diffs, &repo, &params(&repo, &files, "src/Uses.kt", 6, "left")),
    [("src/Uses.kt".to_string(), 5, 8, "local".to_string())]
  );
  assert_eq!(targets(&diffs, &repo, &params(&repo, &files, "src/Store.kt", 3, "add")), []);
}

/// An index that answers each query from a fixed list, and records the queries
struct FakeIndex {
  hits: Vec<(&'static str, SymbolHit)>,
  queries: std::sync::Mutex<Vec<(String, Option<String>)>>,
}

impl SymbolIndex for FakeIndex {
  fn symbols(&self, _repository_root: &str, query: &str, revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    self
      .queries
      .lock()
      .map_err(|error| anyhow::anyhow!("{error}"))?
      .push((query.to_string(), revision.map(str::to_string)));
    Ok(Some(self.hits.iter().filter(|(name, _)| *name == query).map(|(_, hit)| hit.clone()).collect()))
  }

  fn references(&self, _repository_root: &str, _name: &str, _revision: Option<&str>) -> anyhow::Result<Option<References>> {
    Ok(None)
  }
}

fn hit(path: &str, line: u32, kind: &str, name: &str) -> SymbolHit {
  SymbolHit {
    path: path.to_string(),
    line,
    kind: kind.to_string(),
    name: name.to_string(),
    parent: None,
  }
}

#[test]
fn goes_outside_the_review_through_the_index_at_the_revision_of_the_side() {
  let repo = TestRepo::new();
  std::fs::create_dir_all(repo.path().join("lib")).unwrap();
  write(&repo, "lib/Outside.kt", b"package lib\n\nfun outsider() = 1\n");
  commit_all(&repo, "Initial");
  write(&repo, "Uses.kt", b"fun run() = outsider() + inner()\n\nfun inner() = 2\n");
  let commit = commit_all(&repo, "Use it");
  let diffs = unhurried();
  let files = diffs
    .read(
      repo.git_executor(),
      repo_path(&repo),
      &ObjectCache::disabled(),
      &DiffSource::Commit { id: commit.clone() },
      WhitespacePolicy::Default,
    )
    .unwrap();
  let index = FakeIndex {
    hits: vec![("outsider", hit("lib/Outside.kt", 3, "function", "outsider"))],
    queries: std::sync::Mutex::default(),
  };
  let mut params = single_file_params(&repo, &files, &commit);
  params.site.column = 13;
  let counting = repo.git_executor().with_process_stats("test");
  let found = diffs.find_declaration(&counting, &ObjectCache::disabled(), Some(&index), &params).unwrap();
  // the blob at the revision (`cat-file --batch-check`) and its text for the column (`cat-file --batch`)
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 2)]);
  let target = &found.targets[0];
  assert_eq!(
    (target.path.as_str(), target.line, target.column, target.kind.as_str()),
    ("lib/Outside.kt", 2, 4, "function")
  );
  let outside = target.outside.as_ref().unwrap();
  assert_eq!((outside.revision.as_deref(), outside.line_count), (Some(commit.as_str()), 4));
  assert_eq!(Some(outside.blob_id.clone()), repo.rev_parse(&format!("{commit}:lib/Outside.kt")).ok());
  assert_eq!(*index.queries.lock().unwrap(), [("outsider".to_string(), Some(commit))]);

  // a declaration of the review is found there, without the index
  params.site.column = 25;
  let inner = diffs.find_declaration(repo.git_executor(), &ObjectCache::disabled(), Some(&index), &params).unwrap();
  assert_eq!((inner.targets.len(), inner.targets[0].line, inner.targets[0].outside.is_none()), (1, 2, true));
  assert_eq!(index.queries.lock().unwrap().len(), 1);
}

/// The params of the name at the start of the first line of `Uses.kt`, the only file of the review of `commit`
pub(crate) fn single_file_params(repo: &TestRepo, files: &[DiffFile], commit: &str) -> FindDeclarationParams {
  let file = &files[0];
  FindDeclarationParams {
    repository_path: repo_path(repo).to_string(),
    site: DeclarationSite {
      path: "Uses.kt".to_string(),
      side: DiffSide::New,
      blob: BlobRequest {
        id: file.new_id.clone().unwrap(),
        path: None,
      },
      line: 0,
      column: 0,
    },
    files: vec![ReviewFileSides {
      path: file.path.clone(),
      old_path: None,
      old_id: None,
      new_id: file.new_id.clone(),
      worktree: false,
    }],
    revisions: DiffRevisions {
      old: Some(format!("{commit}^")),
      new: Some(commit.to_string()),
    },
  }
}
