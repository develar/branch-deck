use crate::api::{ApiChange, ApiChangeKind, Usages};
use crate::files::{FileFacts, MechanicalKind, RepeatedEditGroup, changed_files, file_facts, test_subject};
use crate::test_support::{plan_of, range};
use object_cache::commit_diff::FileChange;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use trial_check::diff_scan::CommitDiff;

/// A change of `path` with the `status` of `--raw`; `old_path` for a rename
fn file(status: char, path: &str, old_path: Option<&str>) -> FileChange {
  FileChange {
    status,
    path: path.to_string(),
    old_path: old_path.map(str::to_string),
    additions: 1,
    deletions: 0,
    binary: false,
    old_blob: None,
    new_blob: None,
    hunks: Vec::new(),
  }
}

fn commit(files: Vec<FileChange>) -> CommitDiff {
  CommitDiff { id: "c".to_string(), files }
}

/// The facts of `diffs` with no text, no link, and `api`
fn facts_of(diffs: &[CommitDiff], api: &[ApiChange]) -> Vec<FileFacts> {
  file_facts(&changed_files(diffs), &HashMap::new(), &[], &[], api).0
}

/// The paths of the facts that are untested
fn untested(facts: &[FileFacts]) -> Vec<&str> {
  facts.iter().filter(|facts| facts.untested).map(|facts| facts.path.as_str()).collect()
}

#[test]
fn a_test_names_its_subject_by_its_path() {
  let subjects: Vec<Option<String>> = [
    "src/test/kotlin/a/SessionTest.kt",
    "src/a/SessionTests.java",
    "crates/a/src/session_test.rs",
    "crates/a/tests/session.rs",
    "src/lib/session.test.ts",
    "src/lib/session.spec.tsx",
    "src/__tests__/session.ts",
    "scripts/test_session.py",
    "src/a/Session.kt",
    "src/a/Test.kt",
  ]
  .iter()
  .map(|path| test_subject(path))
  .collect();
  let session = Some("session".to_string());
  assert_eq!(
    subjects,
    [
      session.clone(),
      session.clone(),
      session.clone(),
      session.clone(),
      session.clone(),
      session.clone(),
      session.clone(),
      session,
      None,
      None
    ]
  );
}

/// A source file changed without its test is untested; one changed with its test, a test and a file of no known
/// language are not; a deleted file is listed as deleted and never untested, a file deleted and added again is a file
/// like any other, one added and deleted again is not listed
#[test]
fn a_source_file_changed_without_its_test_is_untested() {
  let diffs = [
    commit(vec![
      file('M', "src/a/Session.kt", None),
      file('M', "src/a/Login.kt", None),
      file('A', "README.md", None),
      file('D', "src/a/Back.kt", None),
      file('A', "src/a/Tmp.kt", None),
    ]),
    commit(vec![
      file('M', "src/test/kotlin/a/LoginTest.kt", None),
      file('D', "src/a/Gone.kt", None),
      file('A', "src/a/Back.kt", None),
      file('D', "src/a/Tmp.kt", None),
    ]),
  ];
  let facts = facts_of(&diffs, &[]);
  assert_eq!(untested(&facts), ["src/a/Back.kt", "src/a/Session.kt"]);
  let listed: Vec<(&str, Option<MechanicalKind>)> = facts.iter().map(|facts| (facts.path.as_str(), facts.mechanical)).collect();
  assert_eq!(
    listed,
    [
      ("src/a/Back.kt", None),
      ("src/a/Session.kt", None),
      ("src/a/Gone.kt", Some(MechanicalKind::Deleted)),
      ("src/a/Login.kt", None),
      ("src/test/kotlin/a/LoginTest.kt", None),
      ("README.md", None)
    ]
  );
}

/// A renamed file is listed at its new path with its first path, also after a second move, and a Rust module file names
/// the directory its test names
#[test]
fn a_renamed_file_is_listed_at_its_new_path() {
  let diffs = [
    commit(vec![
      file('R', "crates/auth/src/session/mod.rs", Some("crates/auth/src/old.rs")),
      file('A', "crates/auth/tests/session.rs", None),
      file('M', "crates/auth/src/token.rs", None),
    ]),
    commit(vec![file('R', "crates/auth/src/token/mod.rs", Some("crates/auth/src/token.rs"))]),
  ];
  let facts = facts_of(&diffs, &[]);
  let paths: Vec<(&str, Option<&str>, bool)> = facts.iter().map(|facts| (facts.path.as_str(), facts.old_path.as_deref(), facts.untested)).collect();
  assert_eq!(
    paths,
    [
      ("crates/auth/src/token/mod.rs", Some("crates/auth/src/token.rs"), true),
      ("crates/auth/src/session/mod.rs", Some("crates/auth/src/old.rs"), false),
      ("crates/auth/tests/session.rs", None, false)
    ]
  );
}

/// The usages left outside the change of a file's removed and changed API are its `breaks`, which rank it first; a use
/// updated in the change, an addition and a declaration without usages break nothing, so an untested file ranks above
/// them
#[test]
fn the_usages_of_a_files_api_rank_it_first() {
  let diffs = [commit(vec![
    file('M', "src/a/Session.kt", None),
    file('M', "src/a/Api.kt", None),
    file('M', "src/a/Unused.kt", None),
    file('M', "src/test/a/ApiTest.kt", None),
    file('M', "src/test/a/UnusedTest.kt", None),
  ])];
  let api = [
    api_change("src/a/Api.kt", "few", ApiChangeKind::Changed, 2),
    api_change("src/a/Api.kt", "many", ApiChangeKind::Removed, 12),
    api_change("src/a/Api.kt", "new", ApiChangeKind::Added, 30),
    api_change("src/a/Unused.kt", "unused", ApiChangeKind::Changed, 0),
  ];
  let facts = facts_of(&diffs, &api);
  let ranked: Vec<(&str, u32, bool)> = facts.iter().map(|facts| (facts.path.as_str(), facts.breaks, facts.untested)).collect();
  assert_eq!(
    ranked,
    [
      ("src/a/Api.kt", 14, false),
      ("src/a/Session.kt", 0, true),
      ("src/a/Unused.kt", 0, false),
      ("src/test/a/ApiTest.kt", 0, false),
      ("src/test/a/UnusedTest.kt", 0, false)
    ]
  );
}

/// `breaks` is complete only when every removed or changed declaration of the file has a complete usage count: a name
/// not asked or a cut answer makes it a lower bound, also at 0, an addition does not, and a file without such an API is
/// complete
#[test]
fn breaks_is_a_lower_bound_when_a_usage_count_of_its_api_is_unknown_or_cut() {
  let diffs = [commit(vec![
    file('M', "src/a/Known.kt", None),
    file('M', "src/a/Unknown.kt", None),
    file('M', "src/a/Cut.kt", None),
    file('M', "src/a/Added.kt", None),
    file('M', "src/a/Plain.kt", None),
  ])];
  let mut unknown = api_change("src/a/Unknown.kt", "toString", ApiChangeKind::Removed, 0);
  unknown.usages = None;
  let mut cut = api_change("src/a/Cut.kt", "Policy", ApiChangeKind::Changed, 0);
  cut.usages.as_mut().unwrap().complete = false;
  let mut added = api_change("src/a/Added.kt", "fresh", ApiChangeKind::Added, 0);
  added.usages = None;
  let api = [
    api_change("src/a/Known.kt", "few", ApiChangeKind::Changed, 2),
    api_change("src/a/Unknown.kt", "kept", ApiChangeKind::Changed, 1),
    unknown,
    cut,
    added,
  ];
  let facts = facts_of(&diffs, &api);
  let mut breaks: Vec<(&str, u32, bool)> = facts.iter().map(|facts| (facts.path.as_str(), facts.breaks, facts.breaks_complete)).collect();
  breaks.sort_unstable();
  assert_eq!(
    breaks,
    [
      ("src/a/Added.kt", 0, true),
      ("src/a/Cut.kt", 0, false),
      ("src/a/Known.kt", 2, true),
      ("src/a/Plain.kt", 0, true),
      ("src/a/Unknown.kt", 1, false)
    ]
  );
}

/// A declaration `name` of `path` with `usages` left outside the change, and one updated in it
fn api_change(path: &str, name: &str, kind: ApiChangeKind, usages: u32) -> ApiChange {
  ApiChange {
    path: path.to_string(),
    line: 1,
    name: name.to_string(),
    key: format!("a.{name}"),
    kind,
    changes: Vec::new(),
    before: None,
    after: None,
    usages: Some(Usages {
      inside: 1,
      outside: usages,
      outside_sites: Vec::new(),
      complete: true,
    }),
    query: name.to_string(),
    owner: None,
    file: path.to_string(),
  }
}

const SERVICE: &str = "package a\n\nimport b.Old\nimport b.Other\n\nclass Service {\n  fun run(): Int {\n    return 1\n  }\n}\n";

/// The plan's mechanical kind of each file, by path
fn mechanical(repo: &TestRepo, base: &str, head: &str) -> Vec<(String, Option<MechanicalKind>)> {
  let plan = plan_of(repo, None, &range(repo, base, head)).unwrap();
  let mut kinds: Vec<(String, Option<MechanicalKind>)> = plan.files.into_iter().map(|facts| (facts.path, facts.mechanical)).collect();
  kinds.sort_by(|a, b| a.0.cmp(&b.0));
  kinds
}

/// Only imports, only whitespace, a pure rename, a deletion and a file of the build model are mechanical; a hunk of an
/// import and a line of code, a file of no known language whose `import` line changes, and an added file are not
#[test]
fn imports_whitespace_renames_deletions_and_the_build_model_are_mechanical() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add files",
    &[
      ("src/a/Imports.kt", SERVICE),
      ("src/a/Indent.kt", SERVICE),
      ("src/a/Mixed.kt", SERVICE),
      ("src/a/Gone.kt", SERVICE),
      ("docs/old.txt", "the notes of the module\n"),
      ("docs/import.txt", "import a\n"),
      ("a/a.iml", "<module/>\n"),
    ],
  );
  let write = |path: &str, content: &str| std::fs::write(repo.path().join(path), content).unwrap();
  write("src/a/Imports.kt", &SERVICE.replace("import b.Old\n", "import b.New\n\n"));
  write("src/a/Indent.kt", &SERVICE.replace("    return 1\n", "      return  1\n"));
  write(
    "src/a/Mixed.kt",
    &SERVICE.replace("import b.Other\n\nclass Service {", "import b.Other\nimport b.New\nclass Service : Base {"),
  );
  write("docs/import.txt", "import b\n");
  write("a/a.iml", "<module type=\"JAVA_MODULE\"/>\n");
  write("src/a/Added.kt", "package a\n\nimport b.New\n");
  repo.git(&["mv", "docs/old.txt", "docs/new.txt"]);
  repo.git(&["rm", "-q", "src/a/Gone.kt"]);
  repo.git(&["add", "-A"]);
  let head = repo.commit_with(&["-m", "Change files"], &[]);
  assert_eq!(
    mechanical(&repo, &base, &head),
    [
      ("a/a.iml".to_string(), Some(MechanicalKind::BuildModel)),
      ("docs/import.txt".to_string(), None),
      ("docs/new.txt".to_string(), Some(MechanicalKind::Renamed)),
      ("src/a/Added.kt".to_string(), None),
      ("src/a/Gone.kt".to_string(), Some(MechanicalKind::Deleted)),
      ("src/a/Imports.kt".to_string(), Some(MechanicalKind::ImportsOnly)),
      ("src/a/Indent.kt".to_string(), Some(MechanicalKind::Whitespace)),
      ("src/a/Mixed.kt".to_string(), None),
    ]
  );
}

const LIB_RS: &str = "use std::{\n  fmt,\n  io,\n};\n\npub fn run() -> io::Result<()> {\n  Ok(())\n}\n";
const STORE_TS: &str = "import {\n  load,\n  save,\n} from \"./io\"\n\nexport const store = { load, save }\n";

/// A call wrapped over several lines is only whitespace, and the same wrap in three files is no repeated edit; a line
/// added inside a Rust `use` block or a TypeScript `import` block over several lines is an import change, a change of
/// code after such a block is not
#[test]
fn a_wrapped_line_and_a_line_in_an_import_block_are_mechanical() {
  let service = SERVICE.replace("    return 1\n", "    return compute(first, second)\n");
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add files",
    &[
      ("src/a/Wrap1.kt", &service),
      ("src/a/Wrap2.kt", &service),
      ("src/a/Wrap3.kt", &service),
      ("src/lib.rs", LIB_RS),
      ("src/main.rs", LIB_RS),
      ("src/store.ts", STORE_TS),
    ],
  );
  let wrapped = service.replace("    return compute(first, second)\n", "    return compute(\n      first,\n      second\n    )\n");
  let head = repo.create_commit_with_files(
    "Wrap and import",
    &[
      ("src/a/Wrap1.kt", &wrapped),
      ("src/a/Wrap2.kt", &wrapped),
      ("src/a/Wrap3.kt", &wrapped),
      ("src/lib.rs", &LIB_RS.replace("  io,\n", "  io,\n  collections::HashMap,\n")),
      (
        "src/main.rs",
        &LIB_RS
          .replace("  io,\n", "  io,\n  collections::HashMap,\n")
          .replace("Ok(())", "Err(io::Error::other(\"no\"))"),
      ),
      ("src/store.ts", &STORE_TS.replace("  save,\n", "  save,\n  remove,\n")),
    ],
  );
  assert_eq!(
    mechanical(&repo, &base, &head),
    [
      ("src/a/Wrap1.kt".to_string(), Some(MechanicalKind::Whitespace)),
      ("src/a/Wrap2.kt".to_string(), Some(MechanicalKind::Whitespace)),
      ("src/a/Wrap3.kt".to_string(), Some(MechanicalKind::Whitespace)),
      ("src/lib.rs".to_string(), Some(MechanicalKind::ImportsOnly)),
      ("src/main.rs".to_string(), None),
      ("src/store.ts".to_string(), Some(MechanicalKind::ImportsOnly)),
    ]
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(plan.repeated_edits, []);
}

/// A bracket inside a string or a comment of an import line opens no statement, and a statement whose brackets never
/// close counts as its first line only: a change of code after such a line keeps the file in the tree
#[test]
fn an_unclosed_bracket_on_an_import_line_makes_no_import_of_the_code_after_it() {
  let files = [
    ("src/comment.ts", "import { load } from \"./io\" // (the old loader\n\nexport const value = load(1)\n"),
    ("src/block.ts", "import { load } from \"./io\" /* { */\n\nexport const value = load(1)\n"),
    ("src/string.ts", "import { load } from \"./io{\"\n\nexport const value = load(1)\n"),
    ("src/a/Name.kt", "package a\n\nimport b.`odd(name`\n\nclass Name {\n  val value = 1\n}\n"),
    ("src/open.ts", "import { load,\n\nexport const value = load(1)\n"),
  ];
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add files", &files);
  let changed: Vec<(&str, String)> = files.iter().map(|(path, text)| (*path, text.replace("load(1)", "load(2)").replace("= 1", "= 2"))).collect();
  let changed: Vec<(&str, &str)> = changed.iter().map(|(path, text)| (*path, text.as_str())).collect();
  let head = repo.create_commit_with_files("Change the code", &changed);
  assert_eq!(
    mechanical(&repo, &base, &head),
    [
      ("src/a/Name.kt".to_string(), None),
      ("src/block.ts".to_string(), None),
      ("src/comment.ts".to_string(), None),
      ("src/open.ts".to_string(), None),
      ("src/string.ts".to_string(), None),
    ]
  );
}

/// An indent of Python, Starlark, YAML or a Makefile changes what the file means, so it is no `Whitespace` change;
/// the same indent of Kotlin is
#[test]
fn an_indent_where_the_indentation_has_a_meaning_is_no_whitespace_change() {
  let files = [
    ("tools/run.py", "def run():\n    start()\nstop()\n"),
    ("tools/defs.bzl", "def run():\n    start()\nstop()\n"),
    ("ci/build.yaml", "steps:\n  - run: build\nname: ci\n"),
    ("Makefile", "build:\n\tcargo build\nVALUE = 1\n"),
    ("src/a/Indent.kt", SERVICE),
  ];
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add files", &files);
  let head = repo.create_commit_with_files(
    "Indent",
    &[
      ("tools/run.py", "def run():\n    start()\n    stop()\n"),
      ("tools/defs.bzl", "def run():\n    start()\n    stop()\n"),
      ("ci/build.yaml", "steps:\n  - run: build\n    name: ci\n"),
      ("Makefile", "build:\n\tcargo build\n\tVALUE = 1\n"),
      ("src/a/Indent.kt", &SERVICE.replace("    return 1\n", "      return 1\n")),
    ],
  );
  assert_eq!(
    mechanical(&repo, &base, &head),
    [
      ("Makefile".to_string(), None),
      ("ci/build.yaml".to_string(), None),
      ("src/a/Indent.kt".to_string(), Some(MechanicalKind::Whitespace)),
      ("tools/defs.bzl".to_string(), None),
      ("tools/run.py".to_string(), None),
    ]
  );
}

/// A file is mechanical only when every commit that changes it is: an import change and then a change of code is not
#[test]
fn a_file_is_mechanical_only_when_every_hunk_of_every_commit_is() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add files", &[("src/a/Service.kt", SERVICE), ("src/a/Twice.kt", SERVICE)]);
  repo.create_commit_with_files(
    "Change imports",
    &[
      ("src/a/Service.kt", &SERVICE.replace("import b.Old\n", "import b.New\n")),
      ("src/a/Twice.kt", &SERVICE.replace("import b.Old\n", "import b.New\n")),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change code and indent",
    &[
      ("src/a/Service.kt", &SERVICE.replace("import b.Old\n", "import b.New\n").replace("return 1", "return 2")),
      (
        "src/a/Twice.kt",
        &SERVICE.replace("import b.Old\n", "import b.New\n").replace("class Service {", "class Service  {"),
      ),
    ],
  );
  assert_eq!(
    mechanical(&repo, &base, &head),
    [("src/a/Service.kt".to_string(), None), ("src/a/Twice.kt".to_string(), Some(MechanicalKind::ImportsOnly)),]
  );
}

/// The same edit of a line in four files is one group, its files marked with it; an edit of a short line alone (`}`)
/// in the same four files makes no group, nor an edit of two files
#[test]
fn the_same_edit_in_three_or_more_files_is_one_group() {
  let source = |name: &str| format!("package a\n\nclass {name} {{\n  val manager = ProjectManager.getInstance()\n  fun run() {{\n  }}\n}}\n");
  let names = ["A", "B", "C", "D", "E", "F"];
  let repo = TestRepo::new();
  let files: Vec<(String, String)> = names.iter().map(|name| (format!("src/a/{name}.kt"), source(name))).collect();
  let refs: Vec<(&str, &str)> = files.iter().map(|(path, content)| (path.as_str(), content.as_str())).collect();
  let base = repo.create_commit_with_files("Add files", &refs);
  let edited: Vec<(String, String)> = files
    .iter()
    .enumerate()
    .map(|(index, (path, content))| {
      let content = match index {
        // the repeated edit, and a `}` alone
        0..=3 => content
          .replace("ProjectManager.getInstance()", "ProjectManagerEx.getInstanceEx()")
          .replace("  fun run() {\n  }\n", "  fun run() {\n  }}\n"),
        // an edit of two files
        _ => content.replace("val manager", "var manager"),
      };
      (path.clone(), content)
    })
    .collect();
  let refs: Vec<(&str, &str)> = edited.iter().map(|(path, content)| (path.as_str(), content.as_str())).collect();
  let head = repo.create_commit_with_files("Use the Ex manager", &refs);
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(
    plan.repeated_edits,
    [RepeatedEditGroup {
      files: ["A", "B", "C", "D"].iter().map(|name| format!("src/a/{name}.kt")).collect(),
      removed: vec!["val manager = ProjectManager.getInstance()".to_string()],
      added: vec!["val manager = ProjectManagerEx.getInstanceEx()".to_string()],
    }]
  );
  let mut groups: Vec<(&str, Option<u32>)> = plan.files.iter().map(|facts| (facts.path.as_str(), facts.repeated_edit)).collect();
  groups.sort_unstable();
  assert_eq!(
    groups,
    [
      ("src/a/A.kt", Some(0)),
      ("src/a/B.kt", Some(0)),
      ("src/a/C.kt", Some(0)),
      ("src/a/D.kt", Some(0)),
      ("src/a/E.kt", None),
      ("src/a/F.kt", None)
    ]
  );
}
