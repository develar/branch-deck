//! Going to a declaration in a review: by the scopes of the file, in the other files of the review ranked by the owner,
//! from the texts the diffs read

use crate::accio::{DeclarationAt, HitKind, References, SymbolHit, SymbolIndex, TestReach};
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
  let (repo, diffs, files, _) = source_review(&[], &[("src/Uses.kt", USES), ("src/Store.kt", STORE), ("lib/Queue.kt", OTHER)]);
  (repo, diffs, files)
}

/// The params of a name on `line` at the start of `needle` in the new side of `path`
pub(crate) fn params(repo: &TestRepo, files: &[DiffFile], path: &str, line: u32, needle: &str) -> FindDeclarationParams {
  let text = match path {
    "src/Uses.kt" => USES,
    "src/Store.kt" => STORE,
    _ => OTHER,
  };
  let column = text.lines().nth(line as usize).unwrap().find(needle).unwrap();
  source_params(repo, files, DiffRevisions::default(), path, line, u32::try_from(column).unwrap())
}

/// The file, its parents made
fn write_file(repo: &TestRepo, path: &str, text: &str) {
  if let Some(parent) = std::path::Path::new(path).parent() {
    std::fs::create_dir_all(repo.path().join(parent)).unwrap();
  }
  write(repo, path, text.as_bytes());
}

/// The diffs a review of `commit` read
fn read_review(diffs: &FileDiffs, repo: &TestRepo, commit: &str) -> Vec<DiffFile> {
  diffs
    .read(
      repo.git_executor(),
      repo_path(repo),
      &ObjectCache::disabled(),
      &DiffSource::Commit { id: commit.to_string() },
      WhitespacePolicy::Default,
    )
    .unwrap()
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

/// A repository whose first commit adds `before` and whose second adds `sources`, and the diffs a review of the second
/// read
fn source_review(before: &[(&str, &str)], sources: &[(&str, &str)]) -> (TestRepo, FileDiffs, Vec<DiffFile>, String) {
  let repo = TestRepo::new();
  write(&repo, "README.md", b"root\n");
  for (path, text) in before {
    write_file(&repo, path, text);
  }
  commit_all(&repo, "Initial");
  for (path, text) in sources {
    write_file(&repo, path, text);
  }
  let commit = commit_all(&repo, "Add source");
  let diffs = unhurried();
  let files = read_review(&diffs, &repo, &commit);
  (repo, diffs, files, commit)
}

fn source_params(repo: &TestRepo, files: &[DiffFile], revisions: DiffRevisions, path: &str, line: u32, column: u32) -> FindDeclarationParams {
  let file = files.iter().find(|it| it.path == path).unwrap();
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
      column,
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
    revisions,
  }
}

/// The revisions of a review of `commit`
fn revisions_of(commit: &str) -> DiffRevisions {
  DiffRevisions {
    old: Some(format!("{commit}^")),
    new: Some(commit.to_string()),
  }
}

/// The jump lands on the name the parser read, at the same column whether the declaration is in the change or in the
/// index; the kind word still differs, which the shared vocabulary closes
#[test]
fn finds_the_rust_impl_method_at_one_column_inside_and_outside_the_review() {
  let declaration = "struct Text;\nimpl Text {\n  #[doc(alias = \"needle\")] pub fn needle(&self, needle: &str) {}\n}\n";
  let uses = "fn run(value: &crate::lib::Text) { value.needle(\"\"); }\n";
  let (local_repo, local_diffs, local_files, local_commit) = source_review(&[], &[("src/lib.rs", declaration), ("src/uses.rs", uses)]);
  let call = uses.lines().next().unwrap();
  let local_params = source_params(
    &local_repo,
    &local_files,
    revisions_of(&local_commit),
    "src/uses.rs",
    0,
    u32::try_from(call.find("needle").unwrap()).unwrap(),
  );
  let local = local_diffs
    .find_declaration(local_repo.git_executor(), &ObjectCache::disabled(), None, &local_params)
    .unwrap();
  let local = &local.targets[0];

  let uses = "fn run() { crate::lib::needle(\"\"); }\n";
  let (outside_repo, outside_diffs, outside_files, outside_commit) = source_review(&[("src/lib.rs", declaration)], &[("src/uses.rs", uses)]);
  let declaration_line = declaration.lines().nth(2).unwrap();
  let outside_column = declaration_line.find("fn needle").unwrap() + 3;
  let outside_params = source_params(
    &outside_repo,
    &outside_files,
    revisions_of(&outside_commit),
    "src/uses.rs",
    0,
    u32::try_from(uses.find("needle").unwrap()).unwrap(),
  );
  let index = FakeIndex {
    hits: vec![(
      "needle",
      hit_with_column("src/lib.rs", 3, u32::try_from(outside_column).unwrap(), HitKind::Method, "needle"),
    )],
    queries: std::sync::Mutex::default(),
  };
  let outside = outside_diffs
    .find_declaration(outside_repo.git_executor(), &ObjectCache::disabled(), Some(&index), &outside_params)
    .unwrap();
  let outside = &outside.targets[0];

  // the first `needle` of the line is the one of the attribute, which the column no longer takes
  let first = declaration_line.find("needle").unwrap();
  assert_ne!(first, outside_column);
  assert_eq!(
    (local.path.as_str(), local.line, local.column, local.end_column),
    (
      "src/lib.rs",
      2,
      u32::try_from(outside_column).unwrap(),
      u32::try_from(outside_column + "needle".len()).unwrap()
    )
  );
  assert_eq!(
    (outside.path.as_str(), outside.line, outside.column, outside.end_column),
    (local.path.as_str(), local.line, local.column, local.end_column)
  );
  // the two sides still name the kind differently: the index says `method`, the review's own walk `function`
  assert_eq!((local.kind.as_str(), outside.kind.as_str()), ("function", "method"));
}

/// One list of nine words filters both sides: a method of an `impl` is a candidate for a call and none for a type
/// position, whether the index answers the word itself or an older accio leaves it out
#[test]
fn one_vocabulary_filters_the_hits_of_the_index_as_it_filters_the_review() {
  let uses = "fn run() { crate::lib::needle(); }\n\nfn typed(value: needle) {}\n";
  let (repo, diffs, files, commit) = source_review(&[("src/lib.rs", "pub struct Text;\n")], &[("src/uses.rs", uses)]);
  let targets = |line: u32, hit: SymbolHit| {
    let column = uses.lines().nth(line as usize).unwrap().find("needle").unwrap();
    let params = source_params(&repo, &files, revisions_of(&commit), "src/uses.rs", line, u32::try_from(column).unwrap());
    let index = FakeIndex {
      hits: vec![("needle", hit)],
      queries: std::sync::Mutex::default(),
    };
    let found = diffs.find_declaration(repo.git_executor(), &ObjectCache::disabled(), Some(&index), &params).unwrap();
    found.targets.len()
  };
  for hit in [
    hit_with_group("src/lib.rs", 1, HitKind::Method, "function", "needle"),
    // an accio older than the word: the kind is folded here, the same way
    hit("src/lib.rs", 1, HitKind::Method, "needle"),
  ] {
    assert_eq!(targets(0, hit.clone()), 1, "a call takes the method");
    assert_eq!(targets(2, hit), 0, "a type position does not");
  }
  // what is no code carries no word and is judged by nothing: the candidate stays
  assert_eq!(targets(2, hit("src/lib.rs", 1, HitKind::Key, "needle")), 1);
  // the word of the answer is what filters, not the kind beside it
  assert_eq!(targets(2, hit_with_group("src/lib.rs", 1, HitKind::Key, "class", "needle")), 1);
  assert_eq!(targets(0, hit_with_group("src/lib.rs", 1, HitKind::Method, "property", "needle")), 0);
}

/// A type alias and an entry of an enum are named by the same two words on both sides of the boundary
#[test]
fn a_type_alias_and_an_enum_entry_are_named_alike_on_both_sides() {
  let declarations = "pub type Shape = u8;\npub enum Color { Red }\n";
  let uses = "fn run(shape: Shape) { let _ = Color::Red; }\n";
  let (repo, diffs, files, commit) = source_review(&[], &[("src/lib.rs", declarations), ("src/uses.rs", uses)]);
  let line = uses.lines().next().unwrap();
  let kind_at = |needle: &str| {
    let params = source_params(&repo, &files, revisions_of(&commit), "src/uses.rs", 0, u32::try_from(line.find(needle).unwrap()).unwrap());
    let found = diffs.find_declaration(repo.git_executor(), &ObjectCache::disabled(), None, &params).unwrap();
    found.targets.first().map(|it| it.kind.clone())
  };
  assert_eq!(kind_at("Shape)"), Some("type_alias".to_string()));
  assert_eq!(kind_at("Red"), Some("enum_entry".to_string()));
}

#[test]
fn returns_the_typescript_overload_signatures_and_implementation() {
  let declarations = concat!(
    "function parse(value: string): string;\n",
    "function parse(value: number): number;\n",
    "function parse(value: string | number) { return value; }\n",
  );
  let uses = "function run() { parse(1); }\n";
  let (repo, diffs, files, commit) = source_review(&[], &[("src/functions.ts", declarations), ("src/uses.ts", uses)]);
  let call = uses.lines().next().unwrap();
  let params = source_params(&repo, &files, revisions_of(&commit), "src/uses.ts", 0, u32::try_from(call.find("parse").unwrap()).unwrap());
  let found = diffs.find_declaration(repo.git_executor(), &ObjectCache::disabled(), None, &params).unwrap();
  assert_eq!(found.name, "parse");
  assert_eq!(
    found.targets.iter().map(|it| (it.line, it.kind.as_str())).collect::<Vec<_>>(),
    [(0, "function"), (1, "function"), (2, "function")]
  );
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

/// An index that answers each query from a fixed list, and records the queries of each lookup: one entry per lookup,
/// the names it asked in their order
struct FakeIndex {
  hits: Vec<(&'static str, SymbolHit)>,
  queries: std::sync::Mutex<Vec<(Vec<String>, Option<String>)>>,
}

impl SymbolIndex for FakeIndex {
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Ok(self.symbols_any(repository_root, &[query], revision)?.and_then(|answers| answers.into_iter().next()))
  }

  fn symbols_any(&self, _repository_root: &str, queries: &[&str], revision: Option<&str>) -> anyhow::Result<Option<Vec<Vec<SymbolHit>>>> {
    self
      .queries
      .lock()
      .map_err(|error| anyhow::anyhow!("{error}"))?
      .push((queries.iter().map(|it| (*it).to_string()).collect(), revision.map(str::to_string)));
    Ok(Some(
      queries
        .iter()
        .map(|query| self.hits.iter().filter(|(name, _)| name == query).map(|(_, hit)| hit.clone()).collect())
        .collect(),
    ))
  }

  fn references(&self, _repository_root: &str, _name: &str, _revision: Option<&str>) -> anyhow::Result<Option<References>> {
    Ok(None)
  }
  /// A declaration lookup asks no place
  fn references_at(&self, _repository_root: &str, _declarations: &[DeclarationAt], _revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    anyhow::bail!("A declaration lookup asked a place")
  }

  /// A declaration lookup asks no tests
  fn tests(&self, _repository_root: &str, _declarations: &[DeclarationAt], _given: &[String], _revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    anyhow::bail!("A declaration lookup asked for tests")
  }
}

fn hit(path: &str, line: u32, kind: HitKind, name: &str) -> SymbolHit {
  SymbolHit {
    path: path.to_string(),
    line,
    column: None,
    kind,
    group: None,
    name: name.to_string(),
    parent: None,
  }
}

/// A hit of an accio that answers the word a review shares
fn hit_with_group(path: &str, line: u32, kind: HitKind, group: &str, name: &str) -> SymbolHit {
  let mut hit = hit(path, line, kind, name);
  hit.group = Some(group.to_string());
  hit
}

fn hit_with_column(path: &str, line: u32, column: u32, kind: HitKind, name: &str) -> SymbolHit {
  let mut hit = hit(path, line, kind, name);
  hit.column = Some(column);
  hit
}

#[test]
fn goes_outside_the_review_through_the_index_at_the_revision_of_the_side() {
  let repo = TestRepo::new();
  std::fs::create_dir_all(repo.path().join("lib")).unwrap();
  write(&repo, "lib/Outside.kt", b"package lib\n\n@Deprecated(\"outsider\") fun outsider() = 1\n");
  write(&repo, "lib/Unknown.kt", b"package lib\n\n@Deprecated(\"outsider\") fun outsider() = 2\n");
  commit_all(&repo, "Initial");
  write(&repo, "Uses.kt", b"fun run() = outsider() + inner()\n\nfun inner() = 2\n");
  let commit = commit_all(&repo, "Use it");
  let diffs = unhurried();
  let files = read_review(&diffs, &repo, &commit);
  let index = FakeIndex {
    hits: vec![
      ("outsider", hit_with_column("lib/Outside.kt", 3, 28, HitKind::Function, "outsider")),
      ("outsider", hit_with_column("lib/Unknown.kt", 3, 28, HitKind::Unknown, "outsider")),
    ],
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
    (target.path.as_str(), target.line, target.column, target.end_column, target.kind.as_str()),
    ("lib/Outside.kt", 2, 28, 36, "function")
  );
  assert!(found.targets.iter().any(|target| target.path == "lib/Unknown.kt" && target.kind == "unknown"));
  let outside = target.outside.as_ref().unwrap();
  assert_eq!((outside.revision.as_deref(), outside.line_count), (Some(commit.as_str()), 4));
  assert_eq!(Some(outside.blob_id.clone()), repo.rev_parse(&format!("{commit}:lib/Outside.kt")).ok());
  // one lookup, one query of the index: the bare name, since the call names no owner
  assert_eq!(*index.queries.lock().unwrap(), [(vec!["outsider".to_string()], Some(commit))]);

  let old_index = FakeIndex {
    hits: vec![("outsider", hit("lib/Outside.kt", 3, HitKind::Function, "outsider"))],
    queries: std::sync::Mutex::default(),
  };
  let fallback = diffs.find_declaration(repo.git_executor(), &ObjectCache::disabled(), Some(&old_index), &params).unwrap();
  assert_eq!((fallback.targets[0].column, fallback.targets[0].end_column), (13, 21));

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
    revisions: revisions_of(commit),
  }
}
