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
/// without it (an import, a call in the same file, an override in an object expression), and no line links the two
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
    // no edge yet: a call carries no shape of its arguments, so `buttons("x")` may mean the overload of the baseline
    // too; it gets its edge once a call carries its arguments and they tell the two overloads apart
    (
      "overload-same-file",
      manager(Some(OVERLOAD), None),
      MANAGER,
      manager(Some(OVERLOAD), Some("fun use(): Buttons = buttons(\"x\")")),
      vec![],
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

/// A call carries no shape of its arguments yet, so it can't tell the overloads of a type apart: it needs the commit
/// that added the key of its name, as before the entries, though a later commit adds another overload to it
#[test]
fn a_call_needs_the_commit_that_added_the_key_of_its_name() {
  let registry =
    |more: &str| format!("package p\n\nobject Registry {{\n  fun register(id: String) {{}}\n\n  fun first() = 1\n\n  fun second() = 2\n\n  fun third() = 3\n{more}}}\n");
  let mut range = Range::default();
  range
    .commit(&[("impl/src/p/Registry.kt", Some(&registry("")))])
    .commit(&[("impl/src/p/Registry.kt", Some(&registry("\n  fun register(id: Int) {}\n")))])
    .commit(&[("impl/src/p/Use.kt", Some("package p\n\nclass Use {\n  fun use() = Registry.register(\"x\")\n}\n"))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("register", Evidence::Qualified)])]);
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
