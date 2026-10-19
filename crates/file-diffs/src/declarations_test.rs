//! Going to a declaration in a review: by the scopes of the file, in the other files of the review ranked by the owner,
//! from the texts the diffs read

use crate::fixture_test::{commit_all, process_counts, repo_path, write};
use crate::model::{BlobRequest, DeclarationSite, DiffFile, DiffSide, DiffSource, FindDeclarationParams, ReviewFileSides};
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
fn review() -> (TestRepo, FileDiffs, Vec<DiffFile>) {
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
  let diffs = FileDiffs::default();
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
fn params(repo: &TestRepo, files: &[DiffFile], path: &str, line: u32, needle: &str) -> FindDeclarationParams {
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
  }
}

/// The path, line and column of each target
fn targets(diffs: &FileDiffs, repo: &TestRepo, params: &FindDeclarationParams) -> Vec<(String, u32, u32, String)> {
  diffs
    .find_declaration(repo.git_executor(), &ObjectCache::disabled(), params)
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
    .find_declaration(&counting, &ObjectCache::disabled(), &params(&repo, &files, "src/Uses.kt", 4, "LIMIT"))
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
