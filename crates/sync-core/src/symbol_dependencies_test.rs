use crate::diff_scan::CommitDiff;
use crate::module_roots::ModuleRoots;
use crate::symbol_dependencies::{Evidence, SymbolGraph, compute_symbol_dependencies};
use crate::symbol_scan::{SymbolInput, TsAliases};
use code_symbols::extract::summarize;
use code_symbols::language::{Language, language_of};
use git_ops::object_texts::TreeEntry;
use object_cache::commit_diff::{FileChange, Hunk};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::Arc;

/// The hunks of a line diff of `old` and `new` with zero context (an LCS, enough for a test's few lines)
fn hunks(old: &str, new: &str) -> Vec<Hunk> {
  let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
  let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
  for i in (0..a.len()).rev() {
    for j in (0..b.len()).rev() {
      lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
    }
  }
  let (mut i, mut j) = (0, 0);
  let mut result: Vec<Hunk> = Vec::new();
  let mut open: Option<Hunk> = None;
  let close = |open: &mut Option<Hunk>, result: &mut Vec<Hunk>| {
    if let Some(mut hunk) = open.take() {
      // git's convention: an empty side starts at the line before
      if hunk.old_lines == 0 {
        hunk.old_start -= 1;
      }
      if hunk.new_lines == 0 {
        hunk.new_start -= 1;
      }
      result.push(hunk);
    }
  };
  while i < a.len() || j < b.len() {
    if i < a.len() && j < b.len() && a[i] == b[j] {
      close(&mut open, &mut result);
      i += 1;
      j += 1;
      continue;
    }
    let hunk = open.get_or_insert_with(|| Hunk {
      old_start: u32::try_from(i + 1).unwrap(),
      old_lines: 0,
      new_start: u32::try_from(j + 1).unwrap(),
      new_lines: 0,
    });
    if j < b.len() && (i == a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
      hunk.new_lines += 1;
      j += 1;
    } else {
      hunk.old_lines += 1;
      i += 1;
    }
  }
  close(&mut open, &mut result);
  result
}

/// A range of commits over files whose content the test gives, and the module structure above them
#[derive(Default)]
struct Range {
  /// Path -> its content now
  files: HashMap<String, String>,
  commits: Vec<CommitDiff>,
  summaries: HashMap<String, Arc<code_symbols::summary::FileSummary>>,
  blobs: usize,
}

impl Range {
  /// The id of a blob of `content`, summarized if the path is source
  fn blob(&mut self, path: &str, content: &str) -> String {
    self.blobs += 1;
    let id = format!("blob{}", self.blobs);
    if let Some(language) = language_of(path) {
      self.summaries.insert(id.clone(), Arc::new(summarize(language, content.as_bytes())));
    }
    id
  }

  /// A commit that writes `files` (`None` deletes the file)
  fn commit(&mut self, files: &[(&str, Option<&str>)]) -> &mut Self {
    let mut changes = Vec::new();
    for &(path, content) in files {
      let before = self.files.get(path).cloned();
      let old_blob = before.as_deref().map(|text| self.blob(path, text));
      let new_blob = content.map(|text| self.blob(path, text));
      changes.push(FileChange {
        status: match (&before, content) {
          (None, _) => 'A',
          (Some(_), None) => 'D',
          _ => 'M',
        },
        path: path.to_string(),
        old_path: None,
        additions: 0,
        deletions: 0,
        binary: false,
        old_blob,
        new_blob,
        hunks: hunks(before.as_deref().unwrap_or_default(), content.unwrap_or_default()),
      });
      match content {
        Some(text) => self.files.insert(path.to_string(), text.to_string()),
        None => self.files.remove(path),
      };
    }
    let id = format!("c{}", self.commits.len());
    self.commits.push(CommitDiff { id, files: changes });
    self
  }

  /// The files as they are before the range
  fn baseline(&mut self, files: &[(&str, &str)]) -> &mut Self {
    self.files.extend(files.iter().map(|(path, content)| ((*path).to_string(), (*content).to_string())));
    self
  }

  /// The graph with a `BUILD.bazel` in each of `modules`
  fn in_modules(&self, modules: &[&str]) -> SymbolGraph {
    let build = [file("BUILD.bazel")];
    let roots: Vec<(&str, &[TreeEntry])> = modules.iter().map(|module| (*module, &build[..])).collect();
    self.graph(&roots, &[], &[])
  }

  fn graph(&self, roots: &[(&str, &[TreeEntry])], crates: &[(&str, &str)], aliases: &[(&str, TsAliases)]) -> SymbolGraph {
    let input = SymbolInput {
      summaries: self.summaries.clone(),
      roots: ModuleRoots::from_entries(roots),
      crates: crates.iter().map(|(dir, name)| ((*dir).to_string(), (*name).to_string())).collect(),
      aliases: aliases.iter().map(|(dir, aliases)| ((*dir).to_string(), aliases.clone())).collect(),
      ..SymbolInput::default()
    };
    compute_symbol_dependencies(&self.commits, &input)
  }
}

fn file(name: &str) -> TreeEntry {
  TreeEntry {
    mode: "100644".to_string(),
    name: name.to_string(),
    id: format!("id-{name}"),
  }
}

/// An edge as `(from, to, [(name, evidence)])`
type Edge = (usize, usize, Vec<(String, Evidence)>);

fn edges(graph: &SymbolGraph) -> Vec<Edge> {
  graph
    .edges
    .iter()
    .map(|edge| (edge.from, edge.to, edge.links.iter().map(|link| (link.name.clone(), link.evidence)).collect()))
    .collect()
}

fn link(name: &str, evidence: Evidence) -> (String, Evidence) {
  (name.to_string(), evidence)
}

#[test]
fn a_kotlin_import_of_a_function_another_commit_adds_is_an_edge() {
  let mut range = Range::default();
  range
    .commit(&[("air/src/a/Util.kt", Some("package a\n\nfun controlUrl(): String = \"\"\n"))])
    .commit(&[("air/src/b/Use.kt", Some("package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n"))]);
  let module = [file("intellij.air.iml")];
  assert_eq!(edges(&range.graph(&[("air", &module)], &[], &[])), [(1, 0, vec![link("controlUrl", Evidence::Import)])]);
}

/// A call of a function of the same package without an import: Kotlin's implicit receivers decide what it means, so
/// it is no evidence. A type of the same package and module is
#[test]
fn the_same_package_is_evidence_for_a_type_in_the_same_module_only() {
  let mut range = Range::default();
  range
    .commit(&[("air/src/a/Util.kt", Some("package a\n\nfun controlUrl(): String = \"\"\n\nclass Lane(val id: Int)\n"))])
    .commit(&[("air/src/a/Use.kt", Some("package a\n\nfun start() = controlUrl()\n\nfun lane() = Lane(1)\n"))])
    .commit(&[("other/src/a/Far.kt", Some("package a\n\nfun far() = Lane(2)\n"))]);
  let (air, other) = ([file("intellij.air.iml")], [file("intellij.other.iml")]);
  assert_eq!(
    edges(&range.graph(&[("air", &air), ("other", &other)], &[], &[])),
    [(1, 0, vec![link("Lane", Evidence::SamePackageType)])]
  );
}

#[test]
fn a_java_member_by_its_class_and_a_static_import_are_edges() {
  let mut range = Range::default();
  range
    .commit(&[(
      "src/acp/AcpControlProtocol.java",
      Some("package acp;\n\npublic final class AcpControlProtocol {\n  public static final String CONTROL_URL_ENV = \"AIR\";\n}\n"),
    )])
    .commit(&[(
      "src/flow/Host.kt",
      Some("package flow\n\nimport acp.AcpControlProtocol\n\nfun env(url: String) = mapOf(AcpControlProtocol.CONTROL_URL_ENV to url)\n"),
    )])
    .commit(&[(
      "src/flow/Other.java",
      Some("package flow;\n\nimport static acp.AcpControlProtocol.CONTROL_URL_ENV;\n\nclass Other {\n  String name() { return CONTROL_URL_ENV; }\n}\n"),
    )]);
  assert_eq!(
    edges(&range.graph(&[], &[], &[])),
    [
      (1, 0, vec![link("AcpControlProtocol", Evidence::Import), link("CONTROL_URL_ENV", Evidence::Qualified)]),
      (2, 0, vec![link("CONTROL_URL_ENV", Evidence::Import)]),
    ]
  );
}

#[test]
fn a_rust_use_of_a_crate_path_and_a_path_of_another_crate_are_edges() {
  let mut range = Range::default();
  range
    .commit(&[("crates/wire/src/lane.rs", Some("pub enum ControlOwner {\n  Agent,\n}\n"))])
    .commit(&[(
      "crates/wire/src/main.rs",
      Some("use crate::lane::ControlOwner;\n\nfn main() {\n  let _ = ControlOwner::Agent;\n}\n"),
    )])
    .commit(&[(
      "crates/guest/src/lib.rs",
      Some("pub fn owner() -> avl_wire::lane::ControlOwner {\n  avl_wire::lane::ControlOwner::Agent\n}\n"),
    )]);
  let cargo = [file("Cargo.toml")];
  let graph = range.graph(
    &[("crates/wire", &cargo), ("crates/guest", &cargo)],
    &[("crates/wire", "avl_wire"), ("crates/guest", "avl_guest")],
    &[],
  );
  let found = edges(&graph);
  assert_eq!(found.iter().map(|(from, to, _)| (*from, *to)).collect::<Vec<_>>(), [(1, 0), (2, 0)], "{found:?}");
  assert!(found[0].2.contains(&link("ControlOwner", Evidence::Import)), "{found:?}");
  assert!(found[1].2.contains(&link("ControlOwner", Evidence::Qualified)), "{found:?}");
}

#[test]
fn a_typescript_import_by_a_relative_path_or_an_alias_is_an_edge() {
  let mut range = Range::default();
  range
    .commit(&[("src/lib/stack.ts", Some("export function stackLead(name: string): string {\n  return name;\n}\n"))])
    .commit(&[("src/lib/use.ts", Some("import { stackLead } from \"./stack\";\n\nexport const lead = stackLead(\"a\");\n"))])
    .commit(&[(
      "src/hooks/useLead.ts",
      Some("import { stackLead as lead } from \"@/lib/stack\";\n\nexport function useLead() {\n  return lead(\"b\");\n}\n"),
    )]);
  let aliases = TsAliases {
    paths: vec![("@/*".to_string(), vec!["src/*".to_string()])],
  };
  let web = [file("package.json"), file("tsconfig.json")];
  let graph = range.graph(&[("", &web)], &[], &[("", aliases)]);
  assert_eq!(
    edges(&graph),
    [(1, 0, vec![link("stackLead", Evidence::Import)]), (2, 0, vec![link("stackLead", Evidence::Import)])]
  );
}

/// The commit that removes a declaration needs the commit that removed its uses: the dangling reference before any
/// check sees it
#[test]
fn a_removal_needs_the_commit_that_removed_the_uses() {
  let mut range = Range::default();
  range
    .commit(&[
      ("src/acp/Agent.kt", Some("package acp\n\nobject Agent {\n  const val CONTROL_URL_ENV = \"AIR\"\n}\n")),
      ("src/flow/Host.kt", Some("package flow\n\nimport acp.Agent\n\nfun env() = Agent.CONTROL_URL_ENV\n")),
    ])
    .commit(&[("src/flow/Host.kt", Some("package flow\n\nfun env() = \"AIR\"\n"))])
    .commit(&[("src/acp/Agent.kt", Some("package acp\n\nobject Agent\n"))]);
  let found = edges(&range.graph(&[], &[], &[]));
  assert!(found.contains(&(2, 1, vec![link("CONTROL_URL_ENV", Evidence::RemovesAfter)])), "{found:?}");
}

#[test]
fn a_commit_needs_no_edge_to_itself_a_moved_declaration_is_no_addition_and_two_owners_are_ambiguous() {
  let mut range = Range::default();
  range
    .commit(&[
      ("src/a/Util.kt", Some("package a\n\nfun helper(): Int = 1\n")),
      ("src/b/Use.kt", Some("package b\n\nimport a.helper\n\nfun use() = helper()\n")),
    ])
    // the same function, moved to another file of the package
    .commit(&[("src/a/Util.kt", Some("package a\n")), ("src/a/Helpers.kt", Some("package a\n\nfun helper(): Int = 1\n"))])
    .commit(&[("src/c/Use.kt", Some("package c\n\nimport a.helper\n\nfun use2() = helper()\n"))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(2, 0, vec![link("helper", Evidence::Import)])]);
}

#[test]
fn the_entities_are_the_functions_and_types_around_the_changed_lines() {
  let mut range = Range::default();
  range
    .commit(&[("src/a/Lane.kt", Some("package a\n\nclass Lane {\n  fun start() = 1\n\n  fun stop() = 2\n}\n"))])
    .commit(&[("src/a/Lane.kt", Some("package a\n\nclass Lane {\n  fun start() = 1\n\n  fun stop() = 3\n}\n"))]);
  let graph = range.graph(&[], &[], &[]);
  assert_eq!(graph.entities[1].iter().map(String::as_str).collect::<Vec<_>>(), ["a.Lane", "a.Lane.stop"]);
}

/// An edge as `(from, to, [(name, used_in, line, evidence)])`
type LinkAt = (usize, usize, Vec<(String, String, u32, Evidence)>);

fn links(graph: &SymbolGraph) -> Vec<LinkAt> {
  graph
    .edges
    .iter()
    .map(|edge| {
      let links = edge.links.iter().map(|link| (link.name.clone(), link.used_in.clone(), link.line, link.evidence)).collect();
      (edge.from, edge.to, links)
    })
    .collect()
}

const MANAGER: &str = "impl/src/p/Manager.kt";
const MANAGER_TEST: &str = "impl/testSrc/p/ManagerTest.kt";
const OVERLOAD: &str = "internal open fun buttons(paneId: String): Buttons = Buttons()";
const NEW_NAME: &str = "internal open fun paneButtons(paneId: String): Buttons = Buttons()";
const OVERRIDE_WINDOW: &str = "override fun buttons(window: Window): Buttons = buttons";
const OVERRIDE_PANE_ID: &str = "override fun buttons(paneId: String): Buttons = buttons";
const OVERRIDE_NEW_NAME: &str = "override fun paneButtons(paneId: String): Buttons = buttons";

/// `Manager` of the repro script of the report of 2026-10-17: `buttons(window: Window)`, then `member`, three functions
/// and `last`, so that no line links a use in `last` to `member`
fn manager(member: Option<&str>, last: Option<&str>) -> String {
  let member = member.map_or_else(String::new, |member| format!("  {member}\n\n"));
  let last = last.map_or_else(String::new, |last| format!("\n  {last}\n"));
  format!(
    "package p\n\nopen class Manager {{\n  internal open fun buttons(window: Window): Buttons = Buttons()\n\n{member}  fun first(): Int = 1\n\n  fun second(): Int = 2\n\n  fun third(): Int = 3\n{last}}}\n\nclass Window\nclass Buttons\n"
  )
}

/// A Kotlin test of `package` with `imports` whose object expression of `supertype` (as written after the `:`) has the
/// members `overrides`, a blank line between two; without imports the first is on line 5
fn object_of(package: &str, imports: &[&str], supertype: &str, overrides: &[&str]) -> String {
  let imports = if imports.is_empty() {
    String::new()
  } else {
    format!("import {}\n\n", imports.join("\nimport "))
  };
  let members = overrides.iter().map(|member| format!("    {member}\n")).collect::<Vec<_>>().join("\n");
  format!("package {package}\n\n{imports}class ManagerTest {{\n  fun create(buttons: Buttons): Any = object : {supertype} {{\n{members}  }}\n}}\n")
}

/// The range of `commits` over `Manager` of the report in the module `impl`
fn over_manager(commits: &[&[(&str, &str)]]) -> Range {
  let mut range = Range::default();
  range.baseline(&[(MANAGER, &manager(None, None))]);
  for files in commits {
    let files: Vec<(&str, Option<&str>)> = files.iter().map(|(path, content)| (*path, Some(*content))).collect();
    range.commit(&files);
  }
  range
}

/// The five cases of the repro script of the report of 2026-10-17, in the module `impl` (a `BUILD.bazel`): the first
/// commit declares (a top-level function, an overload of a name of `Manager`, a new name), the second doesn't compile
/// without it (an import, a call in the same file, an override in an object expression), and no line links the two;
/// each is one edge by symbols
#[test]
fn the_cases_of_the_report() {
  let base = manager(None, None);
  let cases: [(&str, String, &str, String, Vec<Edge>); 5] = [
    (
      "control-import",
      format!("{base}fun paneOf(id: String): Buttons = Buttons()\n"),
      "impl/testSrc/q/PaneTest.kt",
      "package q\n\nimport p.paneOf\n\nclass PaneTest {\n  fun create() = paneOf(\"x\")\n}\n".to_string(),
      vec![(1, 0, vec![link("paneOf", Evidence::Import)])],
    ),
    // a text is no `Window`, a class of the file, so `buttons("x")` means the new overload only
    (
      "overload-same-file",
      manager(Some(OVERLOAD), None),
      MANAGER,
      manager(Some(OVERLOAD), Some("fun use(): Buttons = buttons(\"x\")")),
      vec![(1, 0, vec![link("buttons", Evidence::SameFile)])],
    ),
    (
      "control-same-file",
      manager(Some(NEW_NAME), None),
      MANAGER,
      manager(Some(NEW_NAME), Some("fun use(): Buttons = paneButtons(\"x\")")),
      vec![(1, 0, vec![link("paneButtons", Evidence::SameFile)])],
    ),
    (
      "override-new-name",
      manager(Some(NEW_NAME), None),
      MANAGER_TEST,
      object_of("p", &[], "Manager()", &[OVERRIDE_NEW_NAME]),
      vec![(1, 0, vec![link("paneButtons", Evidence::Override)])],
    ),
    (
      "override-overload",
      manager(Some(OVERLOAD), None),
      MANAGER_TEST,
      object_of("p", &[], "Manager()", &[OVERRIDE_WINDOW, OVERRIDE_PANE_ID]),
      vec![(1, 0, vec![link("buttons", Evidence::Override)])],
    ),
  ];
  for (case, declares, path, uses, expected) in cases {
    let range = over_manager(&[&[(MANAGER, &declares)], &[(path, &uses)]]);
    assert_eq!(edges(&range.in_modules(&["impl"])), expected, "{case}");
  }
}

/// An override of a member another commit adds is a use of it, by the supertype it names: a subclass of the same
/// package and module, an object expression that imports the type, a Java anonymous class, and the body of an enum
/// entry in the same file
#[test]
fn an_override_of_a_member_another_commit_adds_is_an_edge() {
  let level = |entry: &str, member: &str| format!("package p\n\nenum class Level {{\n  {entry},\n  HIGH;\n{member}}}\n");
  let weight = "\n  open fun weight(): Int = 0\n";
  let mut range = Range::default();
  range
    .baseline(&[(MANAGER, &manager(None, None)), ("impl/src/p/Level.kt", &level("LOW", ""))])
    .commit(&[
      (MANAGER, Some(&manager(Some("open fun paneButtons(paneId: String): Buttons = Buttons()"), None))),
      ("impl/src/p/Level.kt", Some(&level("LOW", weight))),
    ])
    .commit(&[(
      "impl/testSrc/p/SubManager.kt",
      Some("package p\n\nclass SubManager : Manager() {\n  override fun paneButtons(paneId: String): Buttons = Buttons()\n}\n"),
    )])
    .commit(&[("impl/testSrc/q/Pane.kt", Some(&object_of("q", &["p.Buttons", "p.Manager"], "Manager()", &[OVERRIDE_NEW_NAME])))])
    .commit(&[(
      "impl/testSrc/q/PaneJava.java",
      Some(
        "package q;\n\nimport p.Buttons;\nimport p.Manager;\n\nclass PaneJava {\n  Manager create() {\n    return new Manager() {\n      @Override\n      public Buttons paneButtons(String paneId) {\n        return new Buttons();\n      }\n    };\n  }\n}\n",
      ),
    )])
    .commit(&[("impl/src/p/Level.kt", Some(&level("LOW {\n    override fun weight() = 1\n  }", weight)))]);
  let pane = || vec![link("paneButtons", Evidence::Override)];
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, pane()), (2, 0, pane()), (3, 0, pane()), (4, 0, vec![link("weight", Evidence::Override)])]
  );
}

/// The shape of the Safe Push failure of 2026-10-17 (IJAI-4074): a commit adds an overload of a name the type
/// declares, and an object expression of another commit overrides both; only the override of the new one needs it
#[test]
fn an_override_of_a_new_overload_is_an_edge() {
  let range = over_manager(&[
    &[(MANAGER, &manager(Some(OVERLOAD), None))],
    &[(MANAGER_TEST, &object_of("p", &[], "Manager()", &[OVERRIDE_WINDOW, OVERRIDE_PANE_ID]))],
  ]);
  assert_eq!(
    links(&range.in_modules(&["impl"])),
    [(1, 0, vec![("buttons".to_string(), MANAGER_TEST.to_string(), 7, Evidence::Override)])]
  );
}

/// A member that overrides compiles without the commit that adds it to a type: the supertype of that type has the
/// member already, so its override is no use of the commit, while that of a new open member of the same commit is
#[test]
fn an_override_of_an_override_is_no_edge() {
  let mid = |members: &str| format!("package p\n\nopen class Base {{\n  open fun describe(): String = \"\"\n}}\n\nopen class Mid : Base() {{\n{members}}}\n");
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Base.kt", &mid(""))])
    .commit(&[(
      "impl/src/p/Base.kt",
      Some(&mid("  override fun describe(): String = \"mid\"\n\n  open fun label(): String = \"\"\n")),
    )])
    .commit(&[(
      MANAGER_TEST,
      Some(&object_of(
        "p",
        &[],
        "Mid()",
        &["override fun describe(): String = \"test\"", "override fun label(): String = \"test\""],
      )),
    )]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("label", Evidence::Override)])]);
}

/// A Java method may override one of a supertype without `@Override`, so a new method of a Java class with supertypes
/// is no edge for an override, while one of a class without them is
#[test]
fn a_java_method_of_a_class_with_supertypes_is_no_edge_for_an_override() {
  let pane = |method: &str| format!("package p;\n\npublic class Pane {{\n{method}}}\n");
  let titled = |method: &str| format!("package p;\n\npublic class TitledPane extends Pane {{\n{method}}}\n");
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Pane.java", &pane("")), ("impl/src/p/TitledPane.java", &titled(""))])
    .commit(&[
      ("impl/src/p/Pane.java", Some(&pane("  public void show(String id) {}\n"))),
      ("impl/src/p/TitledPane.java", Some(&titled("  public void title(String id) {}\n"))),
    ])
    .commit(&[(MANAGER_TEST, Some(&object_of("p", &[], "Pane()", &["override fun show(id: String) {}"])))])
    .commit(&[(
      "impl/testSrc/p/TitledTest.kt",
      Some(&object_of("p", &[], "TitledPane()", &["override fun title(id: String) {}"])),
    )]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("show", Evidence::Override)])]);
}

/// A class with two supertypes may override the member of either, and the members of a supertype outside the range
/// are unknown: no edge, also when the other is a function type, while the same override in an object expression of
/// the one supertype is
#[test]
fn an_override_with_two_supertypes_is_no_edge() {
  let range = over_manager(&[
    &[(MANAGER, &manager(Some(NEW_NAME), None))],
    &[(
      "impl/testSrc/p/Closing.kt",
      "package p\n\nclass Closing : Manager(), AutoCloseable {\n  override fun paneButtons(paneId: String): Buttons = Buttons()\n\n  override fun close() {}\n}\n",
    )],
    &[(MANAGER_TEST, &object_of("p", &[], "Manager()", &[OVERRIDE_NEW_NAME]))],
    &[(
      "impl/testSrc/p/Handler.kt",
      "package p\n\nclass Handler : Manager(), (String) -> Unit {\n  override fun paneButtons(paneId: String): Buttons = Buttons()\n\n  override fun invoke(id: String) {}\n}\n",
    )],
  ]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("paneButtons", Evidence::Override)])]);
}

/// The supertype of an override must resolve to the type of the member: another package without an import, a type of
/// that name the file declares itself, a wildcard import, or the same package with an import of another type of the
/// name is none; an import, an import alias, the full path, and a nested type through an import of its outer type or of
/// itself are
#[test]
fn an_override_whose_supertype_does_not_resolve_is_no_edge() {
  let shadow = "package p\n\nclass Shadow {\n  open class Manager {\n    open fun paneButtons(paneId: String): Int = 1\n  }\n\n  val manager = object : Manager() {\n    override fun paneButtons(paneId: String): Int = 2\n  }\n}\n";
  let range = over_manager(&[
    &[(MANAGER, &manager(Some(NEW_NAME), None))],
    &[("impl/testSrc/q/NoImport.kt", &object_of("q", &[], "Manager()", &[OVERRIDE_NEW_NAME]))],
    &[("impl/testSrc/p/Shadow.kt", shadow)],
    &[("impl/testSrc/q/Star.kt", &object_of("q", &["p.*"], "Manager()", &[OVERRIDE_NEW_NAME]))],
    &[(
      "impl/testSrc/q/Imported.kt",
      &object_of("q", &["p.Buttons", "p.Manager"], "Manager()", &[OVERRIDE_NEW_NAME]),
    )],
    &[(
      "impl/testSrc/q/Aliased.kt",
      &object_of("q", &["p.Buttons", "p.Manager as Base"], "Base()", &[OVERRIDE_NEW_NAME]),
    )],
    &[("impl/testSrc/q/FullPath.kt", &object_of("q", &["p.Buttons"], "p.Manager()", &[OVERRIDE_NEW_NAME]))],
    &[("impl/testSrc/p/OtherImport.kt", &object_of("p", &["r.Manager"], "Manager()", &[OVERRIDE_NEW_NAME]))],
    // a nested type of its own commit, so that an edge to it means that this one resolved, not `Manager`
    &[(
      "impl/src/p/Outer.kt",
      "package p\n\nclass Outer {\n  open class Inner {\n    open fun paneButtons(paneId: String): Int = 1\n  }\n}\n",
    )],
    &[("impl/testSrc/q/ByOuter.kt", &object_of("q", &["p.Outer"], "Outer.Inner()", &[OVERRIDE_INNER]))],
    &[("impl/testSrc/q/ByInner.kt", &object_of("q", &["p.Outer.Inner"], "Inner()", &[OVERRIDE_INNER]))],
  ]);
  let pane = || link("paneButtons", Evidence::Override);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [
      (4, 0, vec![pane()]),
      (5, 0, vec![pane()]),
      (6, 0, vec![pane()]),
      (9, 8, vec![link("Inner", Evidence::Qualified), link("Outer", Evidence::Import), pane()]),
      (10, 8, vec![link("Inner", Evidence::Import), pane()]),
    ]
  );
}

const OVERRIDE_INNER: &str = "override fun paneButtons(paneId: String): Int = 2";

/// An override of the overload the baseline has is no use of the commit that adds another overload and changes the
/// body of the old one; the override of the new one is
#[test]
fn an_override_of_a_member_the_range_does_not_add_is_no_edge() {
  let declares = manager(Some(OVERLOAD), None).replacen("Buttons = Buttons()", "Buttons = Buttons().also { it.hashCode() }", 1);
  let range = over_manager(&[
    &[(MANAGER, &declares)],
    &[(MANAGER_TEST, &object_of("p", &[], "Manager()", &[OVERRIDE_WINDOW]))],
    &[("impl/testSrc/p/PaneTest.kt", &object_of("p", &[], "Manager()", &[OVERRIDE_PANE_ID]))],
  ]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("buttons", Evidence::Override)])]);
}

/// An `internal` member reaches its module only: an override in another module doesn't compile with or without it
#[test]
fn an_override_in_another_module_of_an_internal_member_is_no_edge() {
  let test = object_of("q", &["p.Buttons", "p.Manager"], "Manager()", &[OVERRIDE_NEW_NAME]);
  let range = over_manager(&[
    &[(MANAGER, &manager(Some(NEW_NAME), None))],
    &[("other/src/q/Far.kt", &test)],
    &[("impl/testSrc/q/Near.kt", &test)],
  ]);
  assert_eq!(edges(&range.in_modules(&["impl", "other"])), [(2, 0, vec![link("paneButtons", Evidence::Override)])]);
}

/// An extension member is overridden by one of the same receiver: another receiver, or none, is another member; so is
/// a property of the name of a function
#[test]
fn an_override_of_another_receiver_or_kind_is_no_edge() {
  let override_with = |receiver: &str| object_of("p", &[], "Manager()", &[&format!("override fun {receiver}decorate(times: Int): String = \"\"")]);
  let range = over_manager(&[
    &[(
      MANAGER,
      &manager(Some("open fun String.decorate(times: Int): String = this\n\n  open fun panes(): Int = 1"), None),
    )],
    &[("impl/testSrc/p/IntTest.kt", &override_with("Int."))],
    &[("impl/testSrc/p/PlainTest.kt", &override_with(""))],
    &[("impl/testSrc/p/StringTest.kt", &override_with("String."))],
    &[("impl/testSrc/p/PropertyTest.kt", &object_of("p", &[], "Manager()", &["override val panes: Int = 2"]))],
  ]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(3, 0, vec![link("decorate", Evidence::Override)])]);
}

/// A declaration the parser recovered around may be misread, so a commit whose before side has one changes no entry of
/// its key, which is only present or absent then; the same commit over a clean file is an edge
#[test]
fn a_before_side_in_error_changes_no_state() {
  let callback = |body: &str| {
    format!(
      "package p\n\nclass Window\n\nclass Registry {{\n  fun first(): Int = 1\n\n  fun second(): Int = 2\n\n  fun third(): Int = 3\n\n  fun fourth(): Int = 4\n\n  fun fifth(): Int = 5\n\n  fun sixth(): Int = 6\n}}\n\ninterface Callback {body}\n"
    )
  };
  // the grammar misreads a function without a body before the `}` of its line, at the end of the file
  let misread = callback("{ fun call() }");
  let summary = summarize(Language::Kotlin, misread.as_bytes());
  assert!(summary.declarations.iter().any(|declaration| declaration.in_error) && !summary.is_degraded(), "{summary:?}");
  let graph = |before: &str| {
    let mut range = Range::default();
    range
      .baseline(&[("impl/src/p/Callback.kt", before)])
      .commit(&[("impl/src/p/Callback.kt", Some(&callback("{\n  fun call()\n\n  fun call(window: Window)\n}")))])
      .commit(&[(MANAGER_TEST, Some(&object_of("p", &[], "Callback", &["override fun call(window: Window) {}"])))]);
    edges(&range.in_modules(&["impl"]))
  };
  assert_eq!(graph(&misread), []);
  assert_eq!(graph(&callback("{\n  fun call()\n}")), [(1, 0, vec![link("call", Evidence::Override)])]);
}

/// A commit that removes a member needs the older commit that removed its override: without it the override overrides
/// nothing
#[test]
fn a_removal_needs_the_commit_that_removed_its_override() {
  let mut range = Range::default();
  range
    .baseline(&[
      (MANAGER, &manager(Some(NEW_NAME), None)),
      (MANAGER_TEST, &object_of("p", &[], "Manager()", &[OVERRIDE_NEW_NAME])),
    ])
    .commit(&[(MANAGER_TEST, Some(&object_of("p", &[], "Manager()", &[])))])
    .commit(&[(MANAGER, Some(&manager(None, None)))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("paneButtons", Evidence::RemovesAfter)])]);
}

/// Two overloads a new file declares are two entries of one key: the override of each, the first included, is an edge
#[test]
fn two_overloads_in_a_new_file_keep_both() {
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Window.kt", "package p\n\nclass Window\n")])
    .commit(&[(
      "impl/src/p/Panes.kt",
      Some("package p\n\nopen class Panes {\n  open fun pane(window: Window): Int = 1\n\n  open fun pane(paneId: String): Int = 2\n}\n"),
    )])
    .commit(&[(MANAGER_TEST, Some(&object_of("p", &[], "Panes()", &["override fun pane(window: Window): Int = 3"])))])
    .commit(&[(
      "impl/testSrc/p/PaneTest.kt",
      Some(&object_of("p", &[], "Panes()", &["override fun pane(paneId: String): Int = 4"])),
    )]);
  let links = || vec![link("Panes", Evidence::SamePackageType), link("pane", Evidence::Override)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, links()), (2, 0, links())]);
}

/// A commit that changes the form of an entry (its parameter types) is what a use needs that only the new form
/// accepts; a use that the old form accepted too keeps its edge to the commit that added the entry
#[test]
fn a_new_form_of_an_entry_is_an_edge_for_the_uses_that_need_it() {
  let range = over_manager(&[
    &[(MANAGER, &manager(Some(NEW_NAME), None))],
    &[(MANAGER, &manager(Some("internal open fun paneButtons(paneId: CharSequence): Buttons = Buttons()"), None))],
    &[(
      MANAGER_TEST,
      &object_of("p", &[], "Manager()", &["override fun paneButtons(paneId: CharSequence): Buttons = buttons"]),
    )],
    &[(
      MANAGER,
      &manager(
        Some("internal open fun paneButtons(paneId: CharSequence): Buttons = Buttons()"),
        Some("fun use(): Buttons = paneButtons(\"x\")"),
      ),
    )],
  ]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(2, 1, vec![link("paneButtons", Evidence::Override)]), (3, 0, vec![link("paneButtons", Evidence::SameFile)])]
  );
}

/// A use its own commit's declarations may mean is ambiguous like one of two older commits: `b.Lane` of the same
/// package, which Kotlin picks, against `a.Lane` of the wildcard import. A file of another package has only the import
#[test]
fn a_use_its_own_commit_may_mean_is_no_edge() {
  let use_of = |package: &str| format!("package {package}\n\nimport a.*\n\nfun lane() = Lane()\n");
  let mut range = Range::default();
  range
    .commit(&[("impl/src/a/Lane.kt", Some("package a\n\nclass Lane\n"))])
    .commit(&[("impl/src/b/Lane.kt", Some("package b\n\nclass Lane\n")), ("impl/src/b/Use.kt", Some(&use_of("b")))])
    .commit(&[("impl/src/c/Use.kt", Some(&use_of("c")))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("Lane", Evidence::StarImport)])]);
}

/// A name qualified by another type means a member of that type, also in a file that declares the name itself: the
/// private wrapper `register` of a new class and a new overload beside one of the baseline, around
/// `Registry.register(id)`, and in Rust `Registry::create()` in the `create` of another `impl`
#[test]
fn a_name_qualified_by_another_type_is_no_use_of_a_member_of_its_own_file() {
  let plugin = |members: &str| format!("package p\n\nclass Plugin {{\n  fun register(x: Int) {{}}\n\n  fun first() = 1\n\n  fun second() = 2\n\n  fun third() = 3\n{members}}}\n");
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Plugin.kt", &plugin(""))])
    .commit(&[("impl/src/p/Registry.kt", Some("package p\n\nobject Registry {\n  fun register(id: String) {}\n}\n"))])
    .commit(&[(
      "impl/src/p/Wrapper.kt",
      Some("package p\n\nclass Wrapper {\n  private fun register(id: String) {\n    Registry.register(id)\n  }\n}\n"),
    )])
    .commit(&[("impl/src/p/Plugin.kt", Some(&plugin("\n  fun register(id: String) {\n    Registry.register(id)\n  }\n")))]);
  let register = || vec![link("register", Evidence::Qualified)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, register()), (2, 0, register())]);

  let mut range = Range::default();
  range
    .commit(&[(
      "crates/wire/src/registry.rs",
      Some("pub struct Registry;\n\nimpl Registry {\n  pub fn create() -> Self {\n    Registry\n  }\n}\n"),
    )])
    .commit(&[(
      "crates/wire/src/plugin.rs",
      Some("use crate::registry::Registry;\n\npub struct Plugin;\n\nimpl Plugin {\n  pub fn create() -> Self {\n    let _ = Registry::create();\n    Self\n  }\n}\n"),
    )]);
  let cargo = [file("Cargo.toml")];
  let graph = range.graph(&[("crates/wire", &cargo)], &[("crates/wire", "wire")], &[]);
  assert_eq!(edges(&graph), [(1, 0, vec![link("Registry", Evidence::Import), link("create", Evidence::Qualified)])]);
}

/// A type with a supertype may inherit more overloads of a name, which the range doesn't show, so a call can't tell
/// its overloads apart: it needs the commit that added the key of its name, as before the entries, though its argument
/// fits only the overload a later commit adds, which the same call of an object without supertypes needs
#[test]
fn a_call_needs_the_commit_that_added_the_key_of_its_name() {
  let registry = |name: &str, more: &str| format!("package p\n\nobject {name} {{\n  fun first() = 1\n\n  fun second() = 2\n\n  fun third() = 3\n{more}}}\n");
  let (string, int) = ("\n  fun register(id: String) {}\n", "\n  fun register(id: String) {}\n\n  fun register(id: Int) {}\n");
  let use_of = |name: &str| format!("package p\n\nclass Use{name} {{\n  fun use() = {name}.register(1)\n}}\n");
  let mut range = Range::default();
  range
    .baseline(&[
      ("impl/src/p/Base.kt", "package p\n\nopen class Base\n"),
      ("impl/src/p/Registry.kt", &registry("Registry : Base()", "")),
      ("impl/src/p/Plain.kt", &registry("Plain", "")),
    ])
    .commit(&[
      ("impl/src/p/Registry.kt", Some(&registry("Registry : Base()", string))),
      ("impl/src/p/Plain.kt", Some(&registry("Plain", string))),
    ])
    .commit(&[
      ("impl/src/p/Registry.kt", Some(&registry("Registry : Base()", int))),
      ("impl/src/p/Plain.kt", Some(&registry("Plain", int))),
    ])
    .commit(&[("impl/src/p/UseRegistry.kt", Some(&use_of("Registry")))])
    .commit(&[("impl/src/p/UsePlain.kt", Some(&use_of("Plain")))]);
  let register = || vec![link("register", Evidence::Qualified)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, register()), (3, 1, register())]);
}

const REGISTRY: &str = "impl/src/p/Registry.kt";

/// `Registry` of the tests of calls, with `more` members: one overload of each name; `Window`, a class of its file, and
/// `Pane`, a class nested in another one, which the import of `a.Pane` shadows
fn registry(more: &str) -> String {
  format!(
    "package p\n\nimport a.Pane\n\nobject Registry {{\n  fun register(id: Int) {{}}\n\n  fun open(window: Window) {{}}\n\n  fun show(pane: Pane) {{}}\n\n  fun select(id: String) {{}}\n\n  fun focus(id: Int) {{}}\n\n  fun toggle(id: Int) {{}}\n\n  fun mark(on: Boolean) {{}}\n\n  fun first() = 1\n{more}}}\n\nclass Window\n\nclass Outer {{\n  class Pane\n}}\n"
  )
}

/// The overloads a commit adds to `Registry`
const OVERLOADS: &str = "\n  fun register(id: String) {}\n\n  fun open(paneId: String) {}\n\n  fun show(id: String) {}\n\n  fun select(name: String, focus: Boolean = false) {}\n\n  fun focus(id: Int, request: Boolean) {}\n\n  fun toggle(on: Boolean) {}\n\n  fun mark(id: Int) {}\n";

/// A file of the package `q` with the calls `calls` of `Registry`
fn calls_of(calls: &[&str]) -> String {
  format!("package q\n\nimport p.Registry\n\nfun use(window: p.Window, id: Int) {{\n  {}\n}}\n", calls.join("\n  "))
}

/// The range over one file at `path`: `baseline` before it, then a commit per one of `versions`
fn file_history(path: &str, baseline: &str, versions: &[String]) -> Range {
  let mut range = Range::default();
  range.baseline(&[(path, baseline)]);
  for version in versions {
    range.commit(&[(path, Some(version))]);
  }
  range
}

/// The range over `Registry` of a commit that adds `OVERLOADS`, then a commit per file of `uses`, by its path and calls
fn over_registry(uses: &[(&str, &[&str])]) -> Range {
  let mut range = Range::default();
  range.baseline(&[(REGISTRY, &registry(""))]).commit(&[(REGISTRY, Some(&registry(OVERLOADS)))]);
  for (path, calls) in uses {
    range.commit(&[(path, Some(&calls_of(calls)))]);
  }
  range
}

/// A call that only a new overload accepts needs the commit that adds it: a text for the `Int` of the old one or for
/// `Window`, a class of its file, the name of a named argument, one argument more, a boolean for an `Int` and a number
/// for a `Boolean`, and in Java a text for an `int`
#[test]
fn a_call_of_a_new_overload_that_the_arguments_select_is_an_edge() {
  let pane = |more: &str| format!("package p;\n\npublic class Pane {{\n  public static void show(int index) {{}}\n\n  public static void first() {{}}\n{more}}}\n");
  let mut range = Range::default();
  range
    .baseline(&[(REGISTRY, &registry("")), ("impl/src/p/Pane.java", &pane(""))])
    .commit(&[
      (REGISTRY, Some(&registry(OVERLOADS))),
      ("impl/src/p/Pane.java", Some(&pane("\n  public static void show(String id) {}\n"))),
    ])
    .commit(&[(
      "impl/src/q/Use.kt",
      Some(&calls_of(&[
        "Registry.register(\"x\")",
        "Registry.open(\"pane\")",
        "Registry.select(name = \"x\")",
        "Registry.focus(id, true)",
        "Registry.toggle(true)",
        "Registry.mark(1)",
      ])),
    )])
    .commit(&[(
      "impl/src/q/PaneUse.java",
      Some("package q;\n\nimport p.Pane;\n\nclass PaneUse {\n  void use() {\n    Pane.show(\"x\");\n  }\n}\n"),
    )]);
  let qualified = |names: &[&str]| names.iter().map(|name| link(name, Evidence::Qualified)).collect::<Vec<_>>();
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, qualified(&["focus", "mark", "open", "register", "select", "toggle"])), (2, 0, qualified(&["show"]))]
  );
}

/// A call that the old overload accepts compiles without the commit that adds another: a number for its `Int`, the
/// name of its parameter, as many arguments as it has; the call that only the new one accepts is an edge
#[test]
fn a_call_of_the_old_overload_is_no_edge() {
  let range = over_registry(&[
    ("impl/src/q/Old.kt", &["Registry.register(1)", "Registry.select(id = \"x\")", "Registry.focus(-1)"]),
    ("impl/src/q/New.kt", &["Registry.register(\"x\")"]),
  ]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("register", Evidence::Qualified)])]);
}

/// A call that both overloads accept may mean the old one, so it is no edge: an expression, `null`, and a text for a
/// type that the file declares only nested in another type and imports (`a.Pane`), which may take a text; a text for
/// `Window`, a class of the file, is
#[test]
fn a_call_the_arguments_cannot_tell_apart_is_no_edge() {
  let range = over_registry(&[
    (
      "impl/src/q/Any.kt",
      &["Registry.register(id)", "Registry.open(window)", "Registry.select(null)", "Registry.show(\"x\")"],
    ),
    ("impl/src/q/New.kt", &["Registry.open(\"pane\")"]),
  ]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("open", Evidence::Qualified)])]);
}

/// Two overloads that two commits add, and a call that both accept: it may need either commit, so it is no edge; a call
/// that only one accepts needs its commit
#[test]
fn overloads_of_two_commits_and_an_unknown_argument_are_no_edge() {
  let mut range = Range::default();
  range
    .baseline(&[(REGISTRY, &registry(""))])
    .commit(&[(REGISTRY, Some(&registry("\n  fun lane(id: String) {}\n")))])
    .commit(&[(REGISTRY, Some(&registry("\n  fun lane(id: String) {}\n\n  fun lane(id: Int) {}\n")))])
    .commit(&[("impl/src/q/Any.kt", Some(&calls_of(&["Registry.lane(id)"])))])
    .commit(&[("impl/src/q/Text.kt", Some(&calls_of(&["Registry.lane(\"x\")"])))])
    .commit(&[("impl/src/q/Number.kt", Some(&calls_of(&["Registry.lane(1)"])))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(3, 0, vec![link("lane", Evidence::Qualified)]), (4, 1, vec![link("lane", Evidence::Qualified)])]
  );
}

/// In the file of a key of the baseline that a commit gives a new overload, a call means it only inside its type by a
/// bare name, `this` or `super`: `list.add(0, item)` of a `MutableList`, which the old `add(item)` rejects, or a bare
/// call at the top level is no use of the new `Lanes.add(index, item)`. A key the range adds keeps the rule before: a
/// bare call in an extension of its type at the top level is a use of it
#[test]
fn a_call_on_another_receiver_in_the_same_file_is_no_edge() {
  let lanes =
    |more: &str, tail: &str| format!("package p\n\nclass Lanes {{\n  fun add(item: Item) {{}}\n\n  fun first() = 1\n\n  fun second() = 2\n{more}}}\n\nclass Item\n{tail}");
  let add = "\n  fun add(index: Int, item: Item) {}\n";
  let other = "\nfun fill(list: MutableList<Item>, item: Item) {\n  list.add(0, item)\n}\n\nfun top(item: Item) = add(0, item)\n";
  let inside = format!("{add}\n  fun use(item: Item) = this.add(0, item)\n");
  let insert = format!("{inside}\n  fun insert(index: Int, item: Item) {{}}\n");
  let path = "impl/src/p/Lanes.kt";
  let mut range = Range::default();
  range
    .baseline(&[(path, &lanes("", ""))])
    .commit(&[(path, Some(&lanes(add, "")))])
    .commit(&[(path, Some(&lanes(add, other)))])
    .commit(&[(path, Some(&lanes(&inside, other)))])
    .commit(&[(path, Some(&lanes(&insert, other)))])
    .commit(&[(path, Some(&lanes(&insert, &format!("{other}\nfun Lanes.prepend(item: Item) = insert(0, item)\n"))))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(2, 0, vec![link("add", Evidence::SameFile)]), (4, 3, vec![link("insert", Evidence::SameFile)])]
  );
}

/// In the file of a key of the baseline a bare name or `this` means a member of its type only where nothing the file
/// doesn't show may take the call first: the receiver of a lambda (`buildList`'s, `apply`'s) or of an object
/// expression, the extension receiver of a member extension function or of the getter of a member extension property
/// (`MutableList.add(index, element)` takes `add(0, "x")` there), and in an inner class the outer class, whose
/// `foo(x: Int)`, or that of its companion, takes the `foo(1)` that the old `foo(x: String)` of the inner class
/// rejects. Without that `foo`, and in a function of the type, a call that only the new overload accepts is an edge
#[test]
fn a_call_another_receiver_may_take_is_no_edge() {
  let lanes = |more: &str, outer: &str, inner: &str| {
    format!(
      "package p\n\nclass Lanes {{\n  fun add(item: String) {{}}\n\n  fun first() = 1\n\n  fun second() = 2\n{more}}}\n\nclass Outer {{\n  fun bar() = 0\n{outer}\n  inner class Inner {{\n    fun foo(x: String) {{}}\n\n    fun third() = 3\n{inner}  }}\n}}\n"
    )
  };
  let (add, foo) = ("\n  fun add(index: Int, item: String) {}\n", "\n    fun foo(x: Long) {}\n");
  let uses = [
    "\n  fun build(): List<String> = buildList { add(0, \"x\") }\n",
    "\n  fun model() = mutableListOf<String>().apply { this.add(0, \"x\") }\n",
    "\n  fun anonymous() = object : ArrayList<String>() {\n    init {\n      add(0, \"x\")\n    }\n  }\n",
    "\n  fun MutableList<String>.fill() {\n    add(0, \"x\")\n  }\n",
    "\n  val MutableList<String>.filled: Int\n    get() {\n      add(0, \"x\")\n      return size\n    }\n",
    "\n  fun direct() {\n    add(0, \"x\")\n  }\n",
  ];
  let call = format!("{foo}\n    fun g() {{\n      foo(1)\n    }}\n");
  let range_of = |outer: &str| {
    let mut versions = vec![lanes(add, outer, foo)];
    versions.extend((1..=uses.len()).map(|count| lanes(&format!("{add}{}", uses[..count].concat()), outer, foo)));
    versions.push(lanes(&format!("{add}{}", uses.concat()), outer, &call));
    file_history("impl/src/p/Lanes.kt", &lanes("", outer, ""), &versions)
  };
  let add = || (6, 0, vec![link("add", Evidence::SameFile)]);
  assert_eq!(edges(&range_of("\n  fun foo(x: Int) {}\n").in_modules(&["impl"])), [add()]);
  assert_eq!(edges(&range_of("\n  companion object {\n    fun foo(x: Int) {}\n  }\n").in_modules(&["impl"])), [add()]);
  assert_eq!(edges(&range_of("").in_modules(&["impl"])), [add(), (7, 0, vec![link("foo", Evidence::SameFile)])]);
}

/// A Kotlin call that the members of its type reject falls back to the functions the file imports, by name or by a
/// wildcard, and to those of the default imports: in the file of `Lanes` with `import q.add`, `add(0, "x")`, which the
/// old `add(item: String)` rejects, means `q.add(index, item)`, and `error("boom")` and `println("boom")` mean
/// `kotlin.error` and `kotlin.io.println` beside the old `error(code: Int)` and `println(code: Int, extra: Int)`, so
/// none needs the commit that adds the new overloads, nor does any call in a file with a wildcard import. Without the
/// imports `add(0, "x")` and `fault("boom")`, a name of no default import, are edges
#[test]
fn a_call_an_import_or_a_default_import_may_take_is_no_edge() {
  let lanes = |imports: &str, more: &str| {
    format!(
      "package p\n{imports}\nclass Lanes {{\n  fun add(item: String) {{}}\n\n  fun error(code: Int) {{}}\n\n  fun fault(code: Int) {{}}\n\n  fun println(code: Int, extra: Int) {{}}\n{more}}}\n"
    )
  };
  let overloads = "\n  fun add(index: Int, item: String) {}\n\n  fun error(message: String) {}\n\n  fun fault(message: String) {}\n\n  fun println(message: String) {}\n";
  let calls = format!("{overloads}\n  fun fail() {{\n    add(0, \"x\")\n    error(\"boom\")\n    fault(\"boom\")\n    println(\"boom\")\n  }}\n");
  let range_of = |imports: &str| {
    let path = "impl/src/p/Lanes.kt";
    let mut range = Range::default();
    range
      .baseline(&[(path, &lanes(imports, "")), ("impl/src/q/Util.kt", "package q\n\nfun add(index: Int, item: String) {}\n")])
      .commit(&[(path, Some(&lanes(imports, overloads)))])
      .commit(&[(path, Some(&lanes(imports, &calls)))]);
    range
  };
  let same_file = |names: &[&str]| names.iter().map(|name| link(name, Evidence::SameFile)).collect::<Vec<_>>();
  assert_eq!(edges(&range_of("").in_modules(&["impl"])), [(1, 0, same_file(&["add", "fault"]))]);
  assert_eq!(edges(&range_of("\nimport q.add\n").in_modules(&["impl"])), [(1, 0, same_file(&["fault"]))]);
  assert_eq!(edges(&range_of("\nimport q.*\n").in_modules(&["impl"])), []);
}

/// In Java the anonymous class of a double-brace initializer is the receiver of a bare name in it: `add(0, "x")` there
/// means `ArrayList.add(int, E)`, not the new `Lanes.add(int, String)` beside the old `add(String)`, and in a nested
/// class with an `add` of its own that one; in a method of the class it is an edge
#[test]
fn a_call_in_a_java_anonymous_or_nested_class_is_no_edge() {
  let lanes = |more: &str, part: &str| {
    format!(
      "package p;\n\nimport java.util.ArrayList;\n\npublic class Lanes {{\n  void add(String item) {{}}\n\n  void first() {{}}\n\n  void second() {{}}\n{more}\n  static class Part {{\n    void add(int index, String item) {{}}\n\n    void third() {{}}\n{part}  }}\n}}\n"
    )
  };
  let add = "\n  void add(int index, String item) {}\n";
  let model = format!("{add}\n  Object model() {{\n    return new ArrayList<String>() {{{{\n      add(0, \"x\");\n    }}}};\n  }}\n");
  let fill = "\n    void fill() {\n      add(0, \"x\");\n    }\n";
  let direct = format!("{model}\n  void direct() {{\n    add(0, \"x\");\n  }}\n");
  let versions = [lanes(add, ""), lanes(&model, ""), lanes(&model, fill), lanes(&direct, fill)];
  let range = file_history("impl/src/p/Lanes.java", &lanes("", ""), &versions);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(3, 0, vec![link("add", Evidence::SameFile)])]);
}

/// A bare name that an import brings in means what the import names only where its file shows nothing that comes
/// first: in a class whose supertype `Base` has an `add(int)`, the `add(1)` of `import static p.Util.add` means that
/// one, and in Kotlin a lambda's receiver may take it (`with(items) { add(1) }`), so neither needs the commit that adds
/// `Util.add(int)` beside the old `add(String)`; nor does it beside another import of the name, which Java weighs
/// together (`import static q.Other.*` or `import static q.Other.add`, whose `add(int)` takes it), nor in Kotlin, which
/// falls back to a wildcard import (`import r.*`) or a default import (`kotlin.error` takes the `error("boom")` that
/// the old `Util.error(int)` rejects). In a class without a supertype and at the top level of a Kotlin file the call
/// needs the commit
#[test]
fn a_bare_name_of_an_import_that_its_file_may_take_first_is_no_edge() {
  let util = |more: &str| {
    format!(
      "package p;\n\npublic class Util {{\n  public static void add(String x) {{}}\n\n  public static void error(int code) {{}}\n\n  public static void first() {{}}\n{more}}}\n"
    )
  };
  let java = |class: &str, extends: &str, imports: &str| {
    format!("package q;\n\nimport static p.Util.add;\n{imports}\npublic class {class}{extends} {{\n  void g() {{\n    add(1);\n  }}\n}}\n")
  };
  let kotlin = |imports: &str, body: &str| format!("package q\n\nimport p.Util.add\n{imports}\nfun fill(items: MutableList<Int>) = {body}\n");
  let mut range = Range::default();
  range
    .baseline(&[
      ("impl/src/p/Util.java", &util("")),
      ("impl/src/q/Base.java", "package q;\n\npublic class Base {\n  protected void add(int x) {}\n}\n"),
      ("impl/src/q/Other.java", "package q;\n\npublic class Other {\n  public static void add(int x) {}\n}\n"),
    ])
    .commit(&[(
      "impl/src/p/Util.java",
      Some(&util("\n  public static void add(int x) {}\n\n  public static void error(String message) {}\n")),
    )])
    .commit(&[("impl/src/q/Sub.java", Some(&java("Sub", " extends Base", "")))])
    .commit(&[("impl/src/q/Plain.java", Some(&java("Plain", "", "")))])
    .commit(&[("impl/src/q/Fill.kt", Some(&kotlin("", "with(items) { add(1) }")))])
    .commit(&[("impl/src/q/Top.kt", Some(&kotlin("", "add(1)")))])
    .commit(&[("impl/src/q/Star.java", Some(&java("Star", "", "import static q.Other.*;\n")))])
    .commit(&[("impl/src/q/Both.java", Some(&java("Both", "", "import static q.Other.add;\n")))])
    .commit(&[("impl/src/q/Wild.kt", Some(&kotlin("import r.*\n", "add(1)")))])
    .commit(&[("impl/src/q/Fail.kt", Some(&kotlin("import p.Util.error\n", "error(\"boom\")")))]);
  let add = || vec![link("add", Evidence::Import)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, add()), (4, 0, add())]);
}

/// Java widens an `int` to a `double`, so `scale(2)` compiles without the commit that adds `scale(long)` beside the old
/// `scale(double)` (it picks the new one then, but needs none): no edge, while a text for its `scale(String)` is
#[test]
fn java_widening_accepts_a_number_for_a_double() {
  let scale = |more: &str| format!("package p;\n\npublic class Scale {{\n  public static void scale(double factor) {{}}\n\n  public static void first() {{}}\n{more}}}\n");
  let use_of = |class: &str, argument: &str| format!("package q;\n\nimport p.Scale;\n\nclass {class} {{\n  void use() {{\n    Scale.scale({argument});\n  }}\n}}\n");
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Scale.java", &scale(""))])
    .commit(&[(
      "impl/src/p/Scale.java",
      Some(&scale("\n  public static void scale(long factor) {}\n\n  public static void scale(String spec) {}\n")),
    )])
    .commit(&[("impl/src/q/Zoom.java", Some(&use_of("Zoom", "2")))])
    .commit(&[("impl/src/q/Spec.java", Some(&use_of("Spec", "\"2x\"")))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("scale", Evidence::Qualified)])]);
}

/// A supertype other than `Any` may declare more overloads of a name, which the range doesn't show (`Base` may have a
/// `pane(String)`), so a call can't tell the overloads of a type with one apart: a call that only the new overload of
/// `Panes : Base()` accepts is no edge, nor is one in a class whose supertype a later commit adds on a line of its
/// own, while the same call in a class without supertypes or with `Any` alone is
#[test]
fn an_overload_of_a_type_with_supertypes_is_no_edge_for_calls() {
  let class = |header: &str, more: &str| format!("\nclass {header} {{\n  fun pane(index: Int) {{}}\n\n  fun first() = 1\n{more}}}\n");
  let types = |more: [&str; 4], later: &str| {
    format!(
      "package p\n\nopen class Base\n{}{}{}{}",
      class("Panes : Base()", more[0]),
      class("Plain", more[1]),
      class("Rooted : Any()", more[2]),
      class(&format!("Later(\n  val id: Int,\n){later}"), more[3])
    )
  };
  let pane = "\n  fun pane(id: String) {}\n";
  let used = |name: &str| format!("{pane}\n  fun use{name}() = pane(\"x\")\n");
  let path = "impl/src/p/Panes.kt";
  let mut range = Range::default();
  range
    .baseline(&[(path, &types(["", "", "", ""], ""))])
    .commit(&[(path, Some(&types([pane, pane, pane, pane], "")))])
    .commit(&[(path, Some(&types([&used("Panes"), pane, pane, pane], "")))])
    .commit(&[(path, Some(&types([&used("Panes"), &used("Plain"), pane, pane], "")))])
    .commit(&[(path, Some(&types([&used("Panes"), &used("Plain"), &used("Rooted"), pane], "")))])
    .commit(&[(path, Some(&types([&used("Panes"), &used("Plain"), &used("Rooted"), &used("Later")], " : Base()")))]);
  let found = links(&range.in_modules(&["impl"]));
  let line_of = |text: &str| u32::try_from(range.files[path].lines().position(|line| line.contains(text)).unwrap() + 1).unwrap();
  assert_eq!(
    found,
    [
      (2, 0, vec![("pane".to_string(), path.to_string(), line_of("usePlain"), Evidence::SameFile)]),
      (3, 0, vec![("pane".to_string(), path.to_string(), line_of("useRooted"), Evidence::SameFile)]),
    ]
  );
}

/// Every class has the members of `Any` or `Object`, so a call of one of their names can't tell the overloads of a
/// type apart either: `toString()` compiles without the commit that adds it beside the `toString(level: Int)` of the
/// baseline, while `describe()` of the same commit doesn't
#[test]
fn a_call_of_a_name_every_class_has_is_no_edge() {
  let text =
    |more: &str| format!("package p\n\nclass Text {{\n  fun toString(level: Int): String = \"\"\n\n  fun describe(level: Int): String = \"\"\n\n  fun first() = 1\n{more}}}\n");
  let added = "\n  override fun toString(): String = \"\"\n\n  fun describe(): String = \"\"\n";
  let path = "impl/src/p/Text.kt";
  let mut range = Range::default();
  range
    .baseline(&[(path, &text(""))])
    .commit(&[(path, Some(&text(added)))])
    .commit(&[(path, Some(&text(&format!("{added}\n  fun use() = toString() + describe()\n"))))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("describe", Evidence::SameFile)])]);
}

/// The arguments of a call of a name the range adds fit its one entry in every form Kotlin has, so it keeps its edge:
/// named arguments past a default, a trailing lambda past a default or after a named argument, a lambda in the
/// parentheses before a default, several arguments for a vararg, none, and a spread; and those the check doesn't judge
/// fit too: a named argument before a positional one, a vararg before the last parameter, and a property of a function
/// type, which has no signature
#[test]
fn new_names_keep_their_edges_with_named_arguments_lambda_vararg_and_spread() {
  let mut range = Range::default();
  range
    .commit(&[(
      "impl/src/p/Lanes.kt",
      Some(
        "package p\n\nobject Lanes {\n  fun open(id: String, focus: Boolean = false, title: String = \"\") {}\n\n  fun each(times: Int = 1, block: (Int) -> Unit) {}\n\n  fun after(times: Int, block: () -> Unit) {}\n\n  fun later(block: () -> Unit, times: Int = 1) {}\n\n  fun select(vararg ids: String) {}\n\n  fun mark(id: String, vararg tags: String) {}\n\n  fun spread(first: String, vararg ids: String) {}\n\n  fun pair(first: String, second: Int) {}\n\n  fun tags(vararg ids: String, flag: Boolean) {}\n\n  val handler: (String) -> Unit = {}\n}\n",
      ),
    )])
    .commit(&[(
      "impl/src/p/Use.kt",
      Some(
        "package p\n\nfun use(ids: Array<String>) {\n  Lanes.open(\"x\", title = \"t\")\n  Lanes.each { }\n  Lanes.after(times = 2) { }\n  Lanes.later({ })\n  Lanes.select(\"a\", \"b\", \"c\")\n  Lanes.mark(\"x\")\n  Lanes.spread(\"x\", *ids)\n  Lanes.pair(first = \"a\", 2)\n  Lanes.tags(\"a\", \"b\", flag = true)\n  Lanes.handler(\"x\")\n}\n",
      ),
    )]);
  let names = ["after", "each", "handler", "later", "mark", "open", "pair", "select", "spread", "tags"];
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, names.iter().map(|name| link(name, Evidence::Qualified)).collect::<Vec<_>>())]
  );
}

/// Two files of a package that each add an overload of a top-level function both hold an entry of its key: a call may
/// mean either, so it is no edge, until the commit that deletes one file leaves the other
#[test]
fn top_level_overloads_in_two_files_are_both_entries() {
  let use_of = |name: &str| format!("package q\n\nimport p.paneOf\n\nfun {name}() = paneOf(\"x\")\n");
  let mut range = Range::default();
  range
    .commit(&[("impl/src/p/A.kt", Some("package p\n\nfun paneOf(id: String): Int = 1\n"))])
    .commit(&[("impl/src/p/B.kt", Some("package p\n\nfun paneOf(id: Int): Int = 2\n"))])
    .commit(&[("impl/src/q/Use.kt", Some(&use_of("use")))])
    .commit(&[("impl/src/p/B.kt", None)])
    .commit(&[("impl/src/q/Later.kt", Some(&use_of("later")))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(4, 0, vec![link("paneOf", Evidence::Import)])]);
}

/// The identity of an entry keeps a vararg and the dimensions of an array, so a new `foo(vararg x: Int)` written above
/// an old `foo(x: Int)` is a new entry, not the old one in a new place; two overloads of one side that the identity
/// can't tell apart (`stamp(java.sql.Date)` beside `stamp(java.util.Date)`) leave their key only present or absent
#[test]
fn an_entry_is_its_parameter_types_with_their_arrays() {
  let base = |more: &str| {
    format!(
      "package p\n\nopen class Base {{\n  fun first() = 1\n\n{more}  open fun foo(x: Int) {{}}\n\n  open fun bar(x: Int) {{}}\n\n  open fun stamp(x: java.util.Date) {{}}\n}}\n"
    )
  };
  let added = "  fun foo(vararg x: Int) {}\n\n  open fun bar(vararg x: Int) {}\n\n  fun stamp(x: java.sql.Date) {}\n\n";
  let overrides = "package p\n\nclass Sub : Base() {\n  override fun foo(x: Int) {}\n\n  override fun bar(vararg x: Int) {}\n\n  override fun stamp(x: java.util.Date) {}\n}\n";
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Base.kt", &base(""))])
    .commit(&[("impl/src/p/Base.kt", Some(&base(added)))])
    .commit(&[("impl/src/p/Sub.kt", Some(overrides))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("bar", Evidence::Override)])]);
}

/// The members of the supertypes of a member's type are unknown, so an override in a subtype means the member only by
/// its exact form: neither `foo(x: T)` for the `foo(x: String)` of `Parent`, nor a `Date` its file imports from another
/// package than the member's file; the same type by both files' imports, or a type both leave to the defaults, is
#[test]
fn an_override_of_a_member_of_a_type_with_supertypes_needs_its_exact_form() {
  let base = |members: &str| format!("package p\n\nimport java.sql.Date\n\nopen class Base<T> : Parent() {{\n  fun first() = 1\n{members}}}\n");
  let sub = |name: &str, import: &str, members: &[&str]| format!("package p\n\nimport {import}\n\nclass {name} : Base<Int>() {{\n  {}\n}}\n", members.join("\n\n  "));
  let parent = "package p\n\nimport java.util.Date\n\nopen class Parent {\n  open fun foo(x: String) {}\n\n  open fun stamp(x: Date) {}\n}\n";
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Parent.kt", parent), ("impl/src/p/Base.kt", &base(""))])
    .commit(&[(
      "impl/src/p/Base.kt",
      Some(&base("\n  open fun foo(x: T) {}\n\n  open fun stamp(x: Date) {}\n\n  open fun label(x: String) {}\n")),
    )])
    .commit(&[(
      "impl/src/p/Sub.kt",
      Some(&sub("Sub", "java.util.Date", &["override fun foo(x: String) {}", "override fun stamp(x: Date) {}"])),
    )])
    .commit(&[(
      "impl/src/p/Exact.kt",
      Some(&sub("Exact", "java.sql.Date", &["override fun stamp(x: Date) {}", "override fun label(x: String) {}"])),
    )]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(2, 0, vec![link("label", Evidence::Override), link("stamp", Evidence::Override)])]
  );
}

/// A type of the supertype's name that a scope around the override may mean is no evidence: `Outer.Base` for a class
/// inside `Outer`, or in Java a member type that a class around the overriding one inherits (`Tree.Node` in a subclass
/// of `Tree`). The same supertype at the top level of the file, or in a class without supertypes, is
#[test]
fn an_override_of_a_supertype_a_scope_may_shadow_is_no_edge() {
  let shapes = |top: &str, outer: &str, end: &str| {
    format!(
      "package p\n\nopen class Base {{\n  fun first() = 1\n{top}}}\n\nfun f1() = 1\n\nfun f2() = 2\n\nfun f3() = 3\n\nclass Outer {{\n  open class Base {{\n    open fun area() = 0\n  }}\n\n  fun y1() = 1\n{outer}}}\n{end}"
    )
  };
  let (area, inside) = ("\n  open fun area() = 1\n", "\n  class Sub : Base() {\n    override fun area() = 2\n  }\n");
  let node = |more: &str| format!("package p;\n\npublic class Node {{\n  public void first() {{}}\n{more}}}\n");
  let leaf = |class: &str| format!("package p;\n\nclass {class} {{\n  static class Leaf extends Node {{\n    @Override\n    public void visit() {{}}\n  }}\n}}\n");
  let mut range = Range::default();
  range
    .baseline(&[
      ("impl/src/p/Shapes.kt", &shapes("", "", "")),
      ("impl/src/p/Tree.java", "package p;\n\nclass Tree {\n  static class Node {\n    void visit() {}\n  }\n}\n"),
      ("impl/src/p/Node.java", &node("")),
    ])
    .commit(&[
      ("impl/src/p/Shapes.kt", Some(&shapes(area, "", ""))),
      ("impl/src/p/Node.java", Some(&node("\n  public void visit() {}\n"))),
    ])
    .commit(&[("impl/src/p/Shapes.kt", Some(&shapes(area, inside, "")))])
    .commit(&[(
      "impl/src/p/Shapes.kt",
      Some(&shapes(area, inside, "\nclass Top : Base() {\n  override fun area() = 3\n}\n")),
    )])
    .commit(&[("impl/src/p/Sub.java", Some(&leaf("Sub extends Tree")))])
    .commit(&[("impl/src/p/Plain.java", Some(&leaf("Plain")))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(2, 0, vec![link("area", Evidence::Override)]), (4, 0, vec![link("visit", Evidence::Override)])]
  );
}

/// Every class has `toString` from `Any` or `Object`: an override of it compiles without the commit that adds one to
/// its supertype, while the override of another new method of that commit doesn't
#[test]
fn an_override_of_a_member_of_any_is_no_edge() {
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Pane.java", "package p;\n\npublic class Pane {\n}\n")])
    .commit(&[(
      "impl/src/p/Pane.java",
      Some("package p;\n\npublic class Pane {\n  public String toString() {\n    return \"\";\n  }\n\n  public void show(String id) {}\n}\n"),
    )])
    .commit(&[(
      MANAGER_TEST,
      Some(&object_of(
        "p",
        &[],
        "Pane()",
        &["override fun toString(): String = \"x\"", "override fun show(id: String) {}"],
      )),
    )]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("show", Evidence::Override)])]);
}

/// Where the member's type has supertypes, a type name that neither file imports or declares is one type only in one
/// package: a `Window` of the package of an override in another package, or of its wildcard import, may be another
/// type, so the override is no edge; a common type of the default imports (`String`, `StringBuilder` of `java.lang`,
/// Kotlin's `HashMap` with its type arguments, `IllegalStateException`) is the same in every package, and the same
/// package is the same type
#[test]
fn an_exact_form_needs_one_package_for_a_type_name_neither_file_binds() {
  let base = |members: &str| format!("package p\n\nopen class Base : Parent() {{\n  fun first() = 1\n{members}}}\n");
  let sub = |package: &str, imports: &str, member: &str| format!("package {package}\n\n{imports}class Sub : Base() {{\n  override fun {member} {{}}\n}}\n");
  let defaults = ["append(out: StringBuilder)", "fill(items: HashMap<String, Int>)", "fail(error: IllegalStateException)"];
  let members = ["show(x: Window)", "label(x: String)"]
    .iter()
    .chain(&defaults)
    .map(|it| format!("\n  open fun {it} {{}}\n"))
    .collect::<Vec<_>>()
    .concat();
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Parent.kt", "package p\n\nopen class Parent\n"), ("impl/src/p/Base.kt", &base(""))])
    .commit(&[("impl/src/p/Base.kt", Some(&base(&members)))])
    .commit(&[("impl/testSrc/q/Own.kt", Some(&sub("q", "import p.Base\n\n", "show(x: Window)")))])
    .commit(&[("impl/testSrc/r/Star.kt", Some(&sub("r", "import p.Base\nimport s.*\n\n", "show(x: Window)")))])
    .commit(&[("impl/testSrc/q/Label.kt", Some(&sub("q", "import p.Base\n\n", "label(x: String)")))])
    .commit(&[("impl/testSrc/p/Near.kt", Some(&sub("p", "", "show(x: Window)")))]);
  for (index, member) in defaults.iter().enumerate() {
    range.commit(&[(&format!("impl/testSrc/q/Default{index}.kt"), Some(&sub("q", "import p.Base\n\n", member)))]);
  }
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [
      (3, 0, vec![link("label", Evidence::Override)]),
      (4, 0, vec![link("show", Evidence::Override)]),
      (5, 0, vec![link("append", Evidence::Override)]),
      (6, 0, vec![link("fill", Evidence::Override)]),
      (7, 0, vec![link("fail", Evidence::Override)]),
    ]
  );
}

/// A value in capitals that the file declares (a singleton, an enum constant, a constant) qualifies no other type, nor
/// does a value in small letters the file imports: `DEFAULT.register("x")` and `plugin.register("x")` in the file of a
/// new `register` are uses of it, while a name in capitals that the file declares as a type (`JBUI`), imports (`UI`)
/// or neither (`IJUI`, an object of another file of its package) means a member of that type
#[test]
fn a_value_in_capitals_is_no_other_type() {
  let plugin = |members: &str, uses: &[&str]| {
    let uses = uses.iter().map(|it| format!("\n{it}\n")).collect::<Vec<_>>().concat();
    format!("package p\n\nimport p.util.plugin\nimport q.UI\n\nclass Plugin {{\n  fun first() = 1\n{members}}}\n\nobject JBUI\n\nval DEFAULT = Plugin()\n{uses}")
  };
  let register = "\n  fun register(id: String) {}\n";
  let uses = [
    "fun a() = DEFAULT.register(\"x\")",
    "fun b() = JBUI.register(\"x\")",
    "fun c() = UI.register(\"x\")",
    "fun d() = IJUI.register(\"x\")",
    "fun e() = plugin.register(\"x\")",
  ];
  let path = "impl/src/p/Plugin.kt";
  let mut range = Range::default();
  range
    .baseline(&[
      (path, &plugin("", &[])),
      ("impl/src/p/IJUI.kt", "package p\n\nobject IJUI {\n  fun register(id: String) {}\n}\n"),
      ("impl/src/p/util/Plugins.kt", "package p.util\n\nimport p.Plugin\n\nval plugin = Plugin()\n"),
    ])
    .commit(&[(path, Some(&plugin(register, &[])))]);
  for count in 1..=uses.len() {
    range.commit(&[(path, Some(&plugin(register, &uses[..count])))]);
  }
  let register = || vec![link("register", Evidence::SameFile)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, register()), (5, 0, register())]);
}

/// A member its own commit adds is no origin of an unqualified use outside its type: `register("x")` at the top level
/// of a file whose new class declares a `register(x: String)` that the arguments fit means the function the file
/// imports, which another commit adds; in the class, the use may mean either
#[test]
fn a_member_of_its_own_commit_is_no_origin_of_an_unqualified_use_outside_its_type() {
  let mut range = Range::default();
  range.commit(&[("impl/src/p/Util.kt", Some("package p\n\nfun register(id: String) {}\n"))]).commit(&[(
    "impl/src/q/Plugin.kt",
    Some("package q\n\nimport p.register\n\nclass Plugin {\n  fun register(x: String) {}\n\n  fun start() = register(\"y\")\n}\n\nfun main() = register(\"x\")\n"),
  )]);
  assert_eq!(
    links(&range.in_modules(&["impl"])),
    [(1, 0, vec![("register".to_string(), "impl/src/q/Plugin.kt".to_string(), 11, Evidence::Import)])]
  );
}
