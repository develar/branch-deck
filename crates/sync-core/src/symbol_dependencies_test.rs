use crate::diff_scan::CommitDiff;
use crate::module_roots::ModuleRoots;
use crate::symbol_dependencies::{Evidence, SymbolGraph, compute_symbol_dependencies};
use crate::symbol_scan::{SymbolInput, TsAliases};
use code_symbols::extract::summarize;
use code_symbols::language::{Language, language_of};
use code_symbols::summary::DeclKind;
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

  /// A commit that renames `from` to `to` and gives it `content` (git's `R` status)
  fn rename(&mut self, from: &str, to: &str, content: &str) -> &mut Self {
    let before = self.files.remove(from).unwrap();
    let change = FileChange {
      status: 'R',
      path: to.to_string(),
      old_path: Some(from.to_string()),
      additions: 0,
      deletions: 0,
      binary: false,
      old_blob: Some(self.blob(from, &before)),
      new_blob: Some(self.blob(to, content)),
      hunks: hunks(&before, content),
    };
    self.files.insert(to.to_string(), content.to_string());
    let id = format!("c{}", self.commits.len());
    self.commits.push(CommitDiff { id, files: vec![change] });
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

/// `a.Owner` with a nested object `holder` (`companion object`, `object Nested`) that declares `fun other()` and `members`
fn owner(holder: &str, members: &str) -> String {
  format!("package a\n\nclass Owner {{\n  {holder} {{\n    fun other() = 1\n{members}  }}\n}}\n")
}

/// A range whose first commit adds `members` to the object `holder` of `a.Owner` (`owner`)
fn adding_to_owner(holder: &str, members: &str) -> Range {
  let mut range = Range::default();
  range
    .baseline(&[("src/a/Owner.kt", &owner(holder, ""))])
    .commit(&[("src/a/Owner.kt", Some(&owner(holder, members)))]);
  range
}

const CREATE: &str = "    fun create(): Owner = Owner()\n";

/// The edge of a commit to the first one by the qualified use of `name`
fn qualified(from: usize, name: &str) -> Edge {
  (from, 0, vec![link(name, Evidence::Qualified)])
}

/// A member of a companion without a name of its own is one of the class around it: `Owner.create()` with `Owner`
/// imported (by an alias too) or of the same package, `Owner.Companion.create()` and `a.Owner.create()`, and
/// `Outer.Inner.build()` of the companion of a nested class with `Outer` imported. A type of the name that the file of
/// the use declares, or `Owner` of another package without an import, is none
#[test]
fn a_companion_member_through_its_owner_is_an_edge() {
  let shadow = "package a\n\nclass Use {\n  class Owner {\n    companion object {\n      fun create() = 1\n    }\n  }\n";
  let inner = |members: &str| format!("package a\n\nclass Outer {{\n  class Inner {{\n    companion object {{\n      fun other() = 1\n{members}    }}\n  }}\n}}\n");
  let mut range = adding_to_owner("companion object", CREATE);
  range
    .baseline(&[("src/a/Shadow.kt", &format!("{shadow}}}\n")), ("src/a/Outer.kt", &inner(""))])
    .commit(&[("src/b/Use.kt", Some("package b\n\nimport a.Owner\n\nfun use() = Owner.create()\n"))])
    .commit(&[(
      "src/a/Same.kt",
      Some("package a\n\nfun same() = Owner.Companion.create()\n\nfun short() = Owner.create()\n"),
    )])
    .commit(&[("src/c/Full.kt", Some("package c\n\nfun full() = a.Owner.create()\n"))])
    .commit(&[("src/d/Far.kt", Some("package d\n\nfun far() = Owner.create()\n"))])
    .commit(&[("src/a/Shadow.kt", Some(&format!("{shadow}\n  fun f() = Owner.create()\n}}\n")))])
    .commit(&[("src/e/Alias.kt", Some("package e\n\nimport a.Owner as O\n\nfun alias() = O.create()\n"))])
    .commit(&[("src/a/Outer.kt", Some(&inner("      fun build() = 2\n")))])
    .commit(&[("src/f/Nested.kt", Some("package f\n\nimport a.Outer\n\nfun nested() = Outer.Inner.build()\n"))]);
  assert_eq!(
    edges(&range.graph(&[], &[], &[])),
    [
      qualified(1, "create"),
      qualified(2, "create"),
      qualified(3, "create"),
      qualified(6, "create"),
      (8, 7, vec![link("build", Evidence::Qualified)]),
    ]
  );
}

/// The summary doesn't tell a companion with a name of its own from a nested object: only its path names it
#[test]
fn a_named_companion_needs_its_name() {
  let mut range = adding_to_owner("companion object Factory", CREATE);
  range
    .commit(&[("src/b/Use.kt", Some("package b\n\nimport a.Owner\n\nfun use() = Owner.Factory.create()\n"))])
    .commit(&[("src/c/Nested.kt", Some("package c\n\nimport a.Owner.Factory\n\nfun nested() = Factory.create()\n"))])
    .commit(&[("src/d/Short.kt", Some("package d\n\nimport a.Owner\n\nfun short() = Owner.create()\n"))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [qualified(1, "create"), qualified(2, "create")]);
}

/// Java has a member of a companion on the class around it only when it is `@JvmStatic` (or a `const val`), by
/// `Owner.create()` and `import static a.Owner.create`; `Owner.Companion.plain()` always
#[test]
fn a_java_call_of_a_companion_member_needs_jvm_static() {
  let members = "    @JvmStatic fun create(): Owner = Owner()\n    fun plain() = 2\n    const val LIMIT = 3\n";
  let mut range = adding_to_owner("companion object", members);
  range
    .commit(&[(
      "src/b/Use.java",
      Some("package b;\n\nimport a.Owner;\n\nclass Use {\n  Object use() { return Owner.create(); }\n  int limit() { return Owner.LIMIT; }\n}\n"),
    )])
    .commit(&[(
      "src/b/Static.java",
      Some("package b;\n\nimport static a.Owner.create;\n\nclass Static {\n  Object use() { return create(); }\n}\n"),
    )])
    .commit(&[(
      "src/b/Plain.java",
      Some("package b;\n\nimport a.Owner;\n\nclass Plain {\n  int use() { return Owner.plain(); }\n}\n"),
    )])
    .commit(&[(
      "src/b/Companion.java",
      Some("package b;\n\nimport a.Owner;\n\nclass Companion {\n  int use() { return Owner.Companion.plain(); }\n}\n"),
    )]);
  assert_eq!(
    edges(&range.graph(&[], &[], &[])),
    [
      (1, 0, vec![link("LIMIT", Evidence::Qualified), link("create", Evidence::Qualified)]),
      (2, 0, vec![link("create", Evidence::Import)]),
      qualified(4, "plain"),
    ],
    "`Owner.plain()` of a member without `@JvmStatic` is no use of it"
  );
}

/// `@JvmStatic` is a form of the member (decision 10): a Java call by the class around the companion, or by a
/// top-level object, needs the commit that adds it; a Kotlin one, or Java's through `INSTANCE`, the commit that adds
/// the member
#[test]
fn a_jvm_static_added_later_is_an_edge_for_a_java_call_through_the_owner() {
  let registry = |members: &str| format!("package a\n\nobject Registry {{\n  fun other() = 1\n{members}}}\n");
  let java = |class: &str, call: &str| format!("package b;\n\nimport a.{class};\n\nclass Use{class} {{\n  Object use() {{ return {call}; }}\n}}\n");
  let mut range = adding_to_owner("companion object", CREATE);
  range
    .baseline(&[("src/a/Registry.kt", &registry(""))])
    .commit(&[("src/a/Owner.kt", Some(&owner("companion object", "    @JvmStatic fun create(): Owner = Owner()\n")))])
    .commit(&[("src/b/UseOwner.java", Some(&java("Owner", "Owner.create()")))])
    .commit(&[("src/b/Use.kt", Some("package b\n\nimport a.Owner\n\nfun use() = Owner.create()\n"))])
    .commit(&[("src/a/Registry.kt", Some(&registry("  fun register() = 2\n")))])
    .commit(&[("src/a/Registry.kt", Some(&registry("  @JvmStatic fun register() = 2\n")))])
    .commit(&[("src/b/UseRegistry.java", Some(&java("Registry", "Registry.register()")))])
    .commit(&[(
      "src/c/UseRegistry.java",
      Some(&java("Registry", "Registry.INSTANCE.register()").replacen("package b", "package c", 1)),
    )]);
  assert_eq!(
    edges(&range.graph(&[], &[], &[])),
    [
      (2, 1, vec![link("create", Evidence::Qualified)]),
      qualified(3, "create"),
      (6, 5, vec![link("register", Evidence::Qualified)]),
      (7, 4, vec![link("register", Evidence::Qualified)]),
    ]
  );
}

/// Java calls a member of a Kotlin object through its `INSTANCE`; a companion has none
#[test]
fn a_java_call_through_instance_is_an_edge() {
  let mut range = Range::default();
  range
    .baseline(&[
      ("src/a/Registry.kt", "package a\n\nobject Registry {\n  fun other() = 1\n}\n"),
      ("src/a/Owner.kt", &owner("companion object", "")),
    ])
    .commit(&[
      ("src/a/Registry.kt", Some("package a\n\nobject Registry {\n  fun other() = 1\n  fun register() = 2\n}\n")),
      ("src/a/Owner.kt", Some(&owner("companion object", "    fun register() = 2\n"))),
    ])
    .commit(&[(
      "src/b/Use.java",
      Some("package b;\n\nimport a.Registry;\n\nclass Use {\n  int use() { return Registry.INSTANCE.register(); }\n}\n"),
    )])
    .commit(&[(
      "src/b/Owned.java",
      Some("package b;\n\nimport a.Owner;\n\nclass Owned {\n  int use() { return Owner.INSTANCE.register(); }\n}\n"),
    )]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [qualified(1, "register")]);
}

/// A nested object isn't a companion: `Owner.Nested.create()` is its member, `Owner.create()` none
#[test]
fn a_member_of_a_nested_object_through_the_owner_is_no_edge() {
  let mut range = adding_to_owner("object Nested", "    fun create() = 2\n");
  range
    .commit(&[("src/b/Use.kt", Some("package b\n\nimport a.Owner\n\nfun use() = Owner.Nested.create()\n"))])
    .commit(&[("src/c/Short.kt", Some("package c\n\nimport a.Owner\n\nfun short() = Owner.create()\n"))])
    .commit(&[("src/a/Same.kt", Some("package a\n\nfun same() = Nested.create()\n"))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [qualified(1, "create")], "a nested type is no name of its package");
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

/// The range over `Registry` of a commit per one of `versions`, then a commit per file of `uses`, by its path and calls
fn registry_history(versions: &[String], uses: &[(&str, &[&str])]) -> Range {
  let mut range = file_history(REGISTRY, &registry(""), versions);
  for (path, calls) in uses {
    range.commit(&[(path, Some(&calls_of(calls)))]);
  }
  range
}

/// The range over `Registry` of a commit that adds `OVERLOADS`, then a commit per file of `uses`, by its path and calls
fn over_registry(uses: &[(&str, &[&str])]) -> Range {
  registry_history(&[registry(OVERLOADS)], uses)
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

/// In the file of a Kotlin extension a call on a local, an expression or a name is no use of it, also of a key the
/// range adds: `prompt.openGlobalPrompt(frame)` on a `val prompt = AirComposerInIdeDriver(…)` means the member of that
/// type, which wins over the extension `AirAuthoredScope.openGlobalPrompt` beside it. A bare call or `this` with the
/// extension's receiver as `this`, in a lambda of `with(scope)` or in an extension of the receiver, is a use of it
#[test]
fn an_extension_called_on_a_local_in_its_file_is_no_edge() {
  let settings = |body: &str, more: &str| {
    format!(
      "package p\n\nimport bridge.AirComposerInIdeDriver\n\ninternal fun AirAuthoredScope.launchModelChip(): Int = 1\n\ninternal suspend fun AirAuthoredScope.openGlobalPrompt(frame: AirUiTestFrameScope? = null): AirComposerInIdeDriver {{\n  val prompt = AirComposerInIdeDriver(bridges)\n{body}  return prompt\n}}\n{more}"
    )
  };
  let body = "  prompt.openGlobalPrompt(frame)\n";
  let reopen = "\ninternal suspend fun AirAuthoredScope.reopenGlobalPrompt() {\n  val driver = AirComposerInIdeDriver(bridges)\n  driver.openGlobalPrompt()\n}\n";
  let lambda = "\ninternal suspend fun relaunch(scope: AirAuthoredScope) = with(scope) { openGlobalPrompt() }\n";
  let this_call = "\ninternal suspend fun AirAuthoredScope.reload() = this.openGlobalPrompt()\n";
  let reference = "\ninternal val prompts = listOf(AirAuthoredScope::openGlobalPrompt)\n";
  let path = "air/src/p/AirAuthoredLaunchSettings.kt";
  let mut range = Range::default();
  range
    .baseline(&[(
      "air/src/bridge/AirComposerInIdeDriver.kt",
      "package bridge\n\nclass AirComposerInIdeDriver(bridges: Any) {\n  suspend fun openGlobalPrompt(frame: Any? = null) {}\n}\n",
    )])
    .commit(&[(path, Some(&settings("", "")))])
    .commit(&[(path, Some(&settings(body, reopen)))])
    .commit(&[(path, Some(&settings(body, &format!("{reopen}{lambda}"))))])
    .commit(&[(path, Some(&settings(body, &format!("{reopen}{lambda}{this_call}"))))])
    .commit(&[(path, Some(&settings(body, &format!("{reopen}{lambda}{this_call}{reference}"))))]);
  let same_file = || vec![link("openGlobalPrompt", Evidence::SameFile)];
  assert_eq!(edges(&range.in_modules(&["air"])), [(2, 0, same_file()), (3, 0, same_file()), (4, 0, same_file())]);
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

/// A removal of a top-level function drops the entries of its file only, also when it removes an overload of the
/// baseline: a call of the overload that another file of the range adds keeps its edge. The entries follow their
/// declarations to the file that a commit renames or moves them to, so a removal there drops them, and a later call of
/// the name is no edge, while without the removal it needs the commit that added the function
#[test]
fn a_removal_of_a_top_level_function_drops_the_entries_of_its_file_only() {
  let call = ("impl/src/q/Use.kt", Some("package q\n\nimport p.helper\n\nfun use() = helper(1)\n"));
  let helpers = |more: &str| format!("package p\n\nfun keep() = 0\n{more}\nfun last() = 9\n");
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/B.kt", &helpers("\nfun helper(s: String) = s\n"))])
    .commit(&[("impl/src/p/A.kt", Some("package p\n\nfun helper(x: Int) = x\n"))])
    .commit(&[("impl/src/p/B.kt", Some(&helpers("")))])
    .commit(&[call]);
  let import = || vec![link("helper", Evidence::Import)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, import())]);

  let (from, to, with_helper) = ("impl/src/p/A.kt", "impl/src/p/M.kt", helpers("\nfun helper(x: Int) = x\n"));
  let graph = |moved: bool, removed: bool| {
    let mut range = Range::default();
    range.commit(&[(from, Some(&with_helper))]);
    if moved {
      range.commit(&[(from, Some(&helpers(""))), (to, Some(&with_helper))]);
    } else {
      range.rename(from, to, &with_helper);
    }
    if removed {
      range.commit(&[(to, Some(&helpers("")))]);
    } else {
      range.commit(&[("impl/src/p/Other.kt", Some("package p\n\nfun other() = 1\n"))]);
    }
    range.commit(&[call]);
    range.in_modules(&["impl"])
  };
  for moved in [false, true] {
    let kept = graph(moved, false);
    assert_eq!(edges(&kept), [(3, 0, import())]);
    assert_eq!(kept.edges[0].links[0].declared_in, to);
    assert_eq!(edges(&graph(moved, true)), []);
  }
}

/// The entries of a top-level function follow their declarations one by one, each to one of its identity with its
/// origin: a commit that moves the overloads of two files into one leaves a call that may mean either ambiguous, as
/// it is without the move, also after a later edit of that file, and one that only the public overload takes needs the
/// commit of that overload, wherever the move puts it and whatever signature a later commit gives it, while the
/// overloads of one commit keep its edge
#[test]
fn overloads_a_move_brings_into_one_file_keep_their_origins() {
  let pad = |body: &str| format!("package p\n\nfun first() = 0\n\n{body}\n\nfun last() = 9\n");
  let (a, b, c) = ("impl/src/p/A.kt", "impl/src/p/B.kt", "impl/src/p/C.kt");
  let (int, text, empty) = (pad("fun helper(x: Int) = x"), pad("fun helper(s: String) = s"), pad(""));
  let both = pad("fun helper(x: Int) = x\n\nfun helper(s: String) = s");
  let edited = format!("{both}\nfun more() = 1\n");
  let call = ("impl/src/q/Use.kt", Some("package q\n\nimport p.helper\n\nfun use() = helper(\"x\")\n"));
  let other = ("impl/src/p/Other.kt", Some("package p\n\nfun other() = 1\n"));
  let merge = [(a, Some(empty.as_str())), (b, Some(&empty)), (c, Some(&both))];
  let graph = |commits: &[&[(&str, Option<&str>)]]| {
    let mut range = Range::default();
    for files in commits {
      range.commit(files);
    }
    range.in_modules(&["impl"])
  };
  let (add_int, add_text) = ([(a, Some(int.as_str()))], [(b, Some(text.as_str()))]);
  assert_eq!(edges(&graph(&[&add_int, &add_text, &[other], &[call]])), []);
  assert_eq!(edges(&graph(&[&add_int, &add_text, &merge, &[call]])), []);
  assert_eq!(edges(&graph(&[&add_int, &add_text, &merge, &[(c, Some(&edited))], &[call]])), []);
  let private = pad("fun helper(s: String) = s\n\nprivate fun helper(x: Int) = x");
  let public_one = [merge[0], merge[1], (c, Some(private.as_str()))];
  let import = || vec![link("helper", Evidence::Import)];
  assert_eq!(edges(&graph(&[&add_int, &add_text, &public_one, &[call]])), [(3, 1, import())]);
  let changed = pad("fun helper(s: CharSequence) = s\n\nprivate fun helper(x: Int) = x");
  assert_eq!(edges(&graph(&[&add_int, &add_text, &public_one, &[(c, Some(&changed))], &[call]])), [(4, 1, import())]);

  let one_commit = graph(&[&[add_int[0], add_text[0]], &merge, &[(c, Some(&edited))], &[call]]);
  assert_eq!(edges(&one_commit), [(3, 0, import())]);
  assert_eq!(one_commit.edges[0].links[0].declared_in, c);
}

/// The entries of a top-level function go with their file where a commit renames it, or with their declaration where
/// it moves it, also when the parser can't read the file there (degraded, a parse error in the declaration, one that
/// swallows the declaration): they keep their form and origin there until a commit reads the file again, so a later
/// removal from it drops them and a call of the name is no edge, while without the removal it needs the commit that
/// added the function
#[test]
fn entries_go_with_their_file_into_a_version_the_parser_cannot_read() {
  let call = ("impl/src/q/Use.kt", Some("package q\n\nimport p.helper\n\nfun use() = helper(1)\n"));
  let other = ("impl/src/p/Other.kt", Some("package p\n\nfun other() = 1\n"));
  let (from, to) = ("impl/src/p/A.kt", "impl/src/p/B.kt");
  let small = |body: &str| format!("package p\n\nfun first() = 0\n\n{body}\n\nfun last() = 9\n");
  // a parse error of a few bytes leaves a large file readable
  let large = |body: &str| format!("package p\n\n// {}\n\nfun first() = 0\n\n{body}\n", "padding ".repeat(80));
  // the file without the function, with it, a version the parser can't read, and how: degraded, and the declaration
  // in error or swallowed
  let versions = [
    (
      small(""),
      small("fun helper(x: Int) = x"),
      small("fun helper(x: Int) = x\n\nfun broken( = "),
      (true, Some(false)),
    ),
    (large(""), large("fun helper(x: Int) = x"), large("fun helper(x: Int) = x }"), (false, Some(true))),
    (small(""), small("fun helper(x: Int) = x"), small("fun broken(\n\nfun helper(x: Int) = x"), (true, None)),
  ];
  for (without, with, unreadable, unread) in &versions {
    let summary = summarize(Language::Kotlin, unreadable.as_bytes());
    let helper = summary.declarations.iter().find(|declaration| summary.name(declaration.name) == "helper");
    assert_eq!((summary.is_degraded(), helper.map(|declaration| declaration.in_error)), *unread, "{summary:?}");
    // a move to a file whose summary lacks the declaration is a removal
    let moves: &[bool] = if helper.is_some() { &[false, true] } else { &[false] };
    for (&moved, removed) in moves.iter().flat_map(|moved| [(moved, false), (moved, true)]) {
      let mut range = Range::default();
      range.commit(&[(from, Some(with))]);
      if moved {
        range.commit(&[(from, Some(without)), (to, Some(unreadable))]);
      } else {
        range.rename(from, to, unreadable);
      }
      range
        .commit(&[(to, Some(with))])
        .commit(&[if removed { (to, Some(without.as_str())) } else { other }])
        .commit(&[call]);
      let graph = range.in_modules(&["impl"]);
      if removed {
        assert_eq!(edges(&graph), [], "{unread:?} moved {moved}");
      } else {
        assert_eq!(edges(&graph), [(4, 0, vec![link("helper", Evidence::Import)])], "{unread:?} moved {moved}");
        assert_eq!(graph.edges[0].links[0].declared_in, to, "{unread:?} moved {moved}");
      }
    }
  }
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

/// A parameter with a default that a later commit adds to a new member keeps the edge of a call that both forms accept
/// to the commit that added the member; a call that passes the new parameter, by position or by name, needs the later
/// commit
#[test]
fn a_parameter_added_later_keeps_the_edge_of_a_call_both_forms_accept() {
  let range = registry_history(
    &[
      registry("\n  fun paneOf(id: String) {}\n"),
      registry("\n  fun paneOf(id: String, flag: Boolean = false) {}\n"),
    ],
    &[
      ("impl/src/q/Old.kt", &["Registry.paneOf(\"x\")"]),
      ("impl/src/q/Positional.kt", &["Registry.paneOf(\"x\", true)"]),
      ("impl/src/q/Named.kt", &["Registry.paneOf(\"x\", flag = true)"]),
    ],
  );
  let pane_of = || vec![link("paneOf", Evidence::Qualified)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, pane_of()), (3, 1, pane_of()), (4, 1, pane_of())]);
}

/// A parameter that a commit adds to a member of the baseline is what a call that passes it needs, by position or by
/// name; a call that leaves it to its default compiles without the commit
#[test]
fn a_call_that_needs_the_new_parameter_is_an_edge_to_the_commit_that_adds_it() {
  let range = registry_history(
    &[registry("").replacen("fun focus(id: Int)", "fun focus(id: Int, request: Boolean = false)", 1)],
    &[
      ("impl/src/q/Positional.kt", &["Registry.focus(id, true)"]),
      ("impl/src/q/Named.kt", &["Registry.focus(id, request = true)"]),
      ("impl/src/q/Old.kt", &["Registry.focus(id)"]),
    ],
  );
  let focus = || vec![link("focus", Evidence::Qualified)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, focus()), (2, 0, focus())]);
}

/// A default value for a parameter, or a Java array parameter made a vararg, keeps the parameter types: the same entry
/// in a new form, which a call that leaves the argument out, or passes the elements one by one, needs; a call that
/// passes the argument, or the array, compiles with the old form
#[test]
fn a_new_default_or_vararg_is_an_edge_for_a_call_that_needs_it() {
  let tags = |ids: &str| format!("package p;\n\npublic class Tags {{\n  public static void tag({ids}) {{}}\n\n  public static void first() {{}}\n}}\n");
  let tag = |class: &str, arguments: &str| format!("package q;\n\nimport p.Tags;\n\nclass {class} {{\n  void use(String[] ids) {{\n    Tags.tag({arguments});\n  }}\n}}\n");
  let mut range = Range::default();
  range
    .baseline(&[(REGISTRY, &registry("")), ("impl/src/p/Tags.java", &tags("String[] ids"))])
    .commit(&[
      (REGISTRY, Some(&registry("").replacen("fun mark(on: Boolean)", "fun mark(on: Boolean = true)", 1))),
      ("impl/src/p/Tags.java", Some(&tags("String... ids"))),
    ])
    .commit(&[("impl/src/q/Omitted.kt", Some(&calls_of(&["Registry.mark()"])))])
    .commit(&[("impl/src/q/Passed.kt", Some(&calls_of(&["Registry.mark(false)"])))])
    .commit(&[("impl/src/q/Elements.java", Some(&tag("Elements", "\"a\", \"b\"")))])
    .commit(&[("impl/src/q/Whole.java", Some(&tag("Whole", "ids")))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, vec![link("mark", Evidence::Qualified)]), (3, 0, vec![link("tag", Evidence::Qualified)])]
  );
}

/// A renamed parameter is a new form of its entry: a call that names the parameter by its new name needs the commit,
/// one that passes it by position compiles with the old name too
#[test]
fn a_renamed_parameter_is_an_edge_for_a_call_by_its_new_name() {
  let range = registry_history(
    &[registry("").replacen("fun select(id: String)", "fun select(name: String)", 1)],
    &[
      ("impl/src/q/Named.kt", &["Registry.select(name = \"x\")"]),
      ("impl/src/q/Positional.kt", &["Registry.select(\"x\")"]),
    ],
  );
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("select", Evidence::Qualified)])]);
}

/// Other files of a package may declare overloads of a top-level function, and private properties of its name, that
/// the range doesn't show, so its key has no forms: a call by the new name of a parameter that a commit renames, and a
/// use of a private property that it makes `internal`, are no edge, while the same uses of members changed alike are
#[test]
fn a_changed_form_of_a_top_level_function_or_property_is_no_edge() {
  let util = |name: &str, reach: &str| format!("package p\n\nfun paneOf({name}: String) {{}}\n\n{reach}val paneCount = 0\n\nfun first() = 1\n");
  let mut range = Range::default();
  range
    .baseline(&[(REGISTRY, &registry("\n  private val count = 0\n")), ("impl/src/p/Util.kt", &util("id", "private "))])
    .commit(&[
      (
        REGISTRY,
        Some(&registry("\n  internal val count = 0\n").replacen("fun select(id: String)", "fun select(name: String)", 1)),
      ),
      ("impl/src/p/Util.kt", Some(&util("name", "internal "))),
    ])
    .commit(&[(
      "impl/src/q/Use.kt",
      Some(
        "package q\n\nimport p.Registry\nimport p.paneCount\nimport p.paneOf\n\nfun use() {\n  paneOf(name = \"x\")\n  Registry.select(name = \"x\")\n  val total = paneCount + Registry.count\n}\n",
      ),
    )]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, vec![link("count", Evidence::Qualified), link("select", Evidence::Qualified)])]
  );
}

/// A commit that changes only the body of a member keeps the form of its entry, which stays of the baseline: no call
/// of it needs the commit, not even one by the name of its parameter, while the same call of a member whose parameter
/// the same commit renames does
#[test]
fn a_changed_body_is_no_new_form() {
  let changed = registry("")
    .replacen("fun focus(id: Int) {}", "fun focus(id: Int) {\n    println(id)\n  }", 1)
    .replacen("fun select(id: String)", "fun select(name: String)", 1);
  let range = registry_history(&[changed], &[("impl/src/q/Use.kt", &["Registry.focus(id = 1)", "Registry.select(name = \"x\")"])]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("select", Evidence::Qualified)])]);
}

/// A member whose reach a commit widens is what a use needs that the old reach forbade: a private function and a
/// private value of an object made `internal` and used by `Owner.member` from another file of its module, and an
/// `internal` function made public and used through an import from another module. A use that the old reach allowed
/// needs no commit: the `internal` function through an import from its module, the private ones in their object,
/// where a private function that the same commit adds is used too, which needs it
#[test]
fn a_widened_member_is_an_edge_for_a_use_the_old_reach_forbids() {
  let panes = |reach: [&str; 3], more: &str| {
    format!(
      "package p\n\nobject Panes {{\n  {}fun helper(id: String) {{}}\n\n  {}fun near(id: String) {{}}\n\n  {}val size: Int = 0\n\n  fun first() = 1\n\n  fun second() = 2\n{more}}}\n",
      reach[0], reach[1], reach[2]
    )
  };
  let widened = ["@VisibleForTesting internal ", "", "internal "];
  let fresh = "\n  private fun fresh() = 3\n";
  let path = "impl/src/p/Panes.kt";
  let mut range = Range::default();
  range
    .baseline(&[(path, &panes(["private ", "internal ", "private "], ""))])
    .commit(&[(path, Some(&panes(widened, fresh)))])
    .commit(&[(
      "impl/src/q/Near.kt",
      Some("package q\n\nimport p.Panes\nimport p.Panes.near\n\nfun use() {\n  Panes.helper(\"x\")\n  near(\"y\")\n  val count = Panes.size\n}\n"),
    )])
    .commit(&[("other/src/q/Far.kt", Some("package q\n\nimport p.Panes.near\n\nfun far() = near(\"y\")\n"))])
    .commit(&[(
      path,
      Some(&panes(
        widened,
        &format!("{fresh}\n  fun use() = helper(\"z\")\n\n  fun count() = size\n\n  fun again() = fresh()\n"),
      )),
    )]);
  assert_eq!(
    edges(&range.in_modules(&["impl", "other"])),
    [
      (1, 0, vec![link("helper", Evidence::Qualified), link("size", Evidence::Qualified)]),
      (2, 0, vec![link("near", Evidence::Import)]),
      (3, 0, vec![link("fresh", Evidence::SameFile)]),
    ]
  );
}

/// A supertype other than `Any` may declare an overload that a call means while the member of its name is private
/// (`Base.helper(id: Any)` takes `Panes.helper("x")`), so the call of a member widened in an object with one needs no
/// commit, while that of the member of an object without supertypes does; a value hides its name in the supertypes,
/// so the use of a widened value needs the commit in either
#[test]
fn a_widened_member_of_a_type_with_supertypes_is_no_edge_for_a_call() {
  let object =
    |header: &str, reach: &str| format!("package p\n\nobject {header} {{\n  {reach}fun helper(id: String) {{}}\n\n  {reach}val size: Int = 0\n\n  fun first() = 1\n}}\n");
  let use_of = |name: &str| format!("package q\n\nimport p.{name}\n\nfun use() {{\n  {name}.helper(\"x\")\n  val count = {name}.size\n}}\n");
  let (panes, plain) = ("impl/src/p/Panes.kt", "impl/src/p/Plain.kt");
  let mut range = Range::default();
  range
    .baseline(&[
      ("impl/src/p/Base.kt", "package p\n\nopen class Base\n"),
      (panes, &object("Panes : Base()", "private ")),
      (plain, &object("Plain", "private ")),
    ])
    .commit(&[(panes, Some(&object("Panes : Base()", "internal "))), (plain, Some(&object("Plain", "internal ")))])
    .commit(&[("impl/src/q/UsePanes.kt", Some(&use_of("Panes")))])
    .commit(&[("impl/src/q/UsePlain.kt", Some(&use_of("Plain")))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [
      (1, 0, vec![link("size", Evidence::Qualified)]),
      (2, 0, vec![link("helper", Evidence::Qualified), link("size", Evidence::Qualified)]),
    ]
  );
}

/// An older private form reaches as far as the compiler may let it: in Java the whole top-level class, so a private
/// method of a nested class that a commit adds and a later one widens is used in the class around it before the
/// widening, and in Kotlin the class around a companion, which the summary doesn't tell from a nested object when it
/// has a name of its own (`Factory`), so the class around a nested object too. Such a use needs the commit that added
/// the member; a use in another top-level class of the Java file needs the later commit
#[test]
fn an_older_private_form_reaches_the_top_level_java_class_and_the_class_around_a_nested_kotlin_object() {
  let outer = |reach: Option<&str>, uses: &str| {
    let helper = reach.map_or_else(String::new, |reach| format!("\n    {reach}static int helper() {{\n      return 1;\n    }}\n"));
    format!("package p;\n\npublic class Outer {{\n  void first() {{}}\n\n  static class Inner {{\n    void second() {{}}\n{helper}  }}\n{uses}}}\n")
  };
  let java_uses = "\n  int use() {\n    return Inner.helper();\n  }\n}\n\nclass Other {\n  int use() {\n    return Outer.Inner.helper();\n  }\n";
  let lanes = |reach: Option<&str>, uses: &str| {
    let member = |name: &str| reach.map_or_else(String::new, |reach| format!("\n    {reach}fun {name}() = 1\n"));
    format!(
      "package p\n\nclass Lanes {{\n  fun first() = 1\n\n  companion object Factory {{\n    fun second() = 2\n{}  }}\n\n  object Pool {{\n    fun third() = 3\n{}  }}\n{uses}}}\n",
      member("create"),
      member("take")
    )
  };
  let kotlin_uses = "\n  fun copy() = create()\n\n  fun borrow() = Pool.take()\n";
  let (java, kotlin) = ("impl/src/p/Outer.java", "impl/src/p/Lanes.kt");
  let mut range = Range::default();
  range
    .baseline(&[(java, &outer(None, "")), (kotlin, &lanes(None, ""))])
    .commit(&[(java, Some(&outer(Some("private "), ""))), (kotlin, Some(&lanes(Some("private "), "")))])
    .commit(&[(java, Some(&outer(Some(""), ""))), (kotlin, Some(&lanes(Some("internal "), "")))])
    .commit(&[(java, Some(&outer(Some(""), java_uses))), (kotlin, Some(&lanes(Some("internal "), kotlin_uses)))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [
      (
        2,
        0,
        vec![link("create", Evidence::SameFile), link("helper", Evidence::SameFile), link("take", Evidence::SameFile)]
      ),
      (2, 1, vec![link("helper", Evidence::SameFile)]),
    ]
  );
}

/// A member that a commit makes `open` is what an override of it needs: one of a final member doesn't compile. A call
/// of it compiles before, so it needs no commit, while the same call by the new name of a parameter that a later
/// commit renames needs that one
#[test]
fn a_member_made_open_is_an_edge_for_an_override_only() {
  let open = "open fun paneButtons(paneId: String): Buttons = Buttons()";
  let renamed = "open fun paneButtons(id: String): Buttons = Buttons()";
  let mut range = Range::default();
  range
    .baseline(&[(MANAGER, &manager(Some("fun paneButtons(paneId: String): Buttons = Buttons()"), None))])
    .commit(&[(MANAGER, Some(&manager(Some(open), None)))])
    .commit(&[(MANAGER_TEST, Some(&object_of("p", &[], "Manager()", &[OVERRIDE_NEW_NAME])))])
    .commit(&[(MANAGER, Some(&manager(Some(open), Some("fun use(): Buttons = paneButtons(\"x\")"))))])
    .commit(&[(MANAGER, Some(&manager(Some(renamed), Some("fun use(): Buttons = paneButtons(\"x\")"))))])
    .commit(&[(MANAGER, Some(&manager(Some(renamed), Some("fun use(): Buttons = paneButtons(id = \"x\")"))))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, vec![link("paneButtons", Evidence::Override)]), (4, 3, vec![link("paneButtons", Evidence::SameFile)])]
  );
}

/// Whether a member is static is no form: an inner class made a public nested one is the same entry in a new form,
/// judged by its reach only, so a use of its type from its package (Java) or its module (Kotlin), which the inner class
/// allowed, needs no commit, while one from another package or module needs the commit that widened it
#[test]
fn an_older_form_is_judged_as_static_as_the_entry_is_now() {
  let java = |inner: &str| format!("package p;\n\npublic class Outer {{\n  void first() {{}}\n\n  {inner} {{\n    void second() {{}}\n  }}\n\n  void third() {{}}\n}}\n");
  let kotlin = |inner: &str| format!("package p\n\nclass Shell {{\n  fun first() = 1\n\n  {inner} {{\n    fun second() = 2\n  }}\n\n  fun third() = 3\n}}\n");
  let kotlin_use = "package q\n\nimport p.Shell\n\nfun use(inner: Shell.Inner) = inner\n";
  let (outer_java, outer_kotlin) = ("impl/src/p/Outer.java", "impl/src/p/Shell.kt");
  let mut range = Range::default();
  range
    .baseline(&[(outer_java, &java("class Inner")), (outer_kotlin, &kotlin("internal inner class Inner"))])
    .commit(&[(outer_java, Some(&java("public static class Inner"))), (outer_kotlin, Some(&kotlin("class Inner")))])
    .commit(&[
      ("impl/src/p/Use.java", Some("package p;\n\nclass Use {\n  Outer.Inner inner;\n}\n")),
      ("impl/src/q/Use.kt", Some(kotlin_use)),
    ])
    .commit(&[
      ("impl/src/q/Far.java", Some("package q;\n\nimport p.Outer;\n\nclass Far {\n  Outer.Inner inner;\n}\n")),
      ("other/src/q/Far.kt", Some(kotlin_use)),
    ]);
  assert_eq!(
    edges(&range.in_modules(&["impl", "other"])),
    [(2, 0, vec![link("Inner", Evidence::Qualified), link("Inner", Evidence::Qualified)])]
  );
}

/// Whether a member overrides is no form, nor are the supertypes of its type: a member that stops overriding, in a
/// Kotlin class that keeps its supertype or a Java class that drops it, and gets another reach in the same commit is
/// judged by that reach only, so an override that compiled with the older form needs no commit, while the override of
/// a member the same commit adds needs it
#[test]
fn an_older_form_is_judged_as_overriding_as_the_entry_is_now() {
  let manager = |member: &str, more: &str| format!("package p\n\nopen class Manager : Base() {{\n  {member}\n\n  fun first() = 1\n\n  fun second() = 2\n{more}}}\n");
  let pane = |header: &str, member: &str, more: &str| format!("package p;\n\npublic class {header} {{\n  {member}\n\n  public void first() {{}}\n{more}}}\n");
  let (manager_path, pane_path) = ("impl/src/p/Manager.kt", "impl/src/p/Pane.java");
  let mut range = Range::default();
  range
    .baseline(&[
      ("impl/src/p/Base.kt", "package p\n\nopen class Base {\n  open fun paneFoo() {}\n}\n"),
      (manager_path, &manager("override fun paneFoo() {}", "")),
      (pane_path, &pane("Pane extends Base", "@Override\n  public void paneFoo() {}", "")),
    ])
    .commit(&[
      (manager_path, Some(&manager("internal open fun paneFoo() {}", "\n  open fun paneBar() {}\n"))),
      (pane_path, Some(&pane("Pane", "void paneFoo() {}", "\n  public void paneBar() {}\n"))),
    ])
    .commit(&[
      (
        "impl/testSrc/p/ManagerTest.kt",
        Some("package p\n\nclass ManagerTest {\n  fun create(): Any = object : Manager() {\n    override fun paneFoo() {}\n\n    override fun paneBar() {}\n  }\n}\n"),
      ),
      (
        "impl/testSrc/p/PaneTest.java",
        Some("package p;\n\nclass PaneTest {\n  Object create() {\n    return new Pane() {\n      @Override\n      public void paneFoo() {}\n\n      @Override\n      public void paneBar() {}\n    };\n  }\n}\n"),
      ),
    ]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, vec![link("paneBar", Evidence::Override), link("paneBar", Evidence::Override)])]
  );
}

/// A new form of an entry is a change of the declaration that a commit created: a call that two overloads accept,
/// one of them in a form a later commit gives it, needs the commit that created both, as a new default of one of them,
/// or a private one made public, splits what the call may mean into that commit and the later one
#[test]
fn a_new_form_needs_the_commit_that_created_its_entry() {
  let range = registry_history(
    &[
      registry("\n  fun place(id: String) {}\n\n  fun place(id: Int, flag: Boolean) {}\n"),
      registry("\n  fun place(id: String) {}\n\n  fun place(id: Int, flag: Boolean = false) {}\n"),
    ],
    &[("impl/src/q/Use.kt", &["Registry.place(id)"])],
  );
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("place", Evidence::Qualified)])]);

  let panes = |more: &str| format!("package p\n\nobject Panes {{\n  fun first() = 1\n{more}}}\n");
  let mut range = file_history(
    "impl/src/p/Panes.kt",
    &panes(""),
    &[
      panes("\n  fun helper(id: String) {}\n\n  private fun helper(id: Int) {}\n"),
      panes("\n  fun helper(id: String) {}\n\n  fun helper(id: Int) {}\n"),
    ],
  );
  range.commit(&[("impl/src/q/Use.kt", Some("package q\n\nimport p.Panes\n\nfun use(id: Int) {\n  Panes.helper(id)\n}\n"))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("helper", Evidence::Qualified)])]);
}

/// A use that needs the form its own commit gives an entry needs the older commits back along the forms of that entry:
/// a call that leaves out `id`, which only the default of its own commit makes optional, needs the commit that added
/// `flag`, which it passes
#[test]
fn a_use_of_a_form_its_own_commit_gives_needs_the_older_steps_of_the_entry() {
  let path = "impl/src/p/Registry.kt";
  let registry = |parameters: &str| format!("package p\n\nobject Registry {{\n  fun first() = 1\n\n  fun place(\n{parameters}  ) = 1\n\n  fun last() = 2\n}}\n");
  let mut range = file_history(path, &registry("    id: Int,\n"), &[registry("    id: Int,\n    flag: Boolean,\n")]);
  range.commit(&[("impl/src/p/Other.kt", Some("package p\n\nfun other() = 1\n"))]).commit(&[
    (path, Some(&registry("    id: Int = 0,\n    flag: Boolean,\n"))),
    ("impl/src/q/Use.kt", Some("package q\n\nimport p.Registry\n\nfun use() = Registry.place(flag = true)\n")),
  ]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("place", Evidence::Qualified)])]);
}

/// An `internal` older form reaches a file whose module the build files don't tell (no build file at all, or none
/// above the file of the use), as the compiler may see one module: an `internal` member made public is no edge for a
/// call from such a file, while a member the same commit adds is
#[test]
fn an_internal_older_form_reaches_a_file_of_an_unknown_module() {
  let panes = |reach: &str, more: &str| format!("package p\n\nobject Panes {{\n  {reach}fun helper(id: String) {{}}\n\n  fun first() = 1\n\n  fun second() = 2\n{more}}}\n");
  let mut range = file_history("impl/src/p/Panes.kt", &panes("internal ", ""), &[panes("", "\n  fun fresh(id: String) {}\n")]);
  range.commit(&[(
    "impl/src/q/Use.kt",
    Some("package q\n\nimport p.Panes\n\nfun use() {\n  Panes.helper(\"x\")\n  Panes.fresh(\"y\")\n}\n"),
  )]);
  let fresh = || [(1, 0, vec![link("fresh", Evidence::Qualified)])];
  assert_eq!(edges(&range.in_modules(&[])), fresh());
  assert_eq!(edges(&range.in_modules(&["impl/src/p"])), fresh());
}

/// An import of a member of a type without supertypes names all the overloads of its name, which are its entries, and
/// compiles only when one of them is visible: an import of a private member made `internal` needs the commit that
/// widened it, though the call after it, in a lambda, tells nothing. A type with a supertype may inherit an overload
/// that the import brings in while the member is private, so there only the key decides
#[test]
fn an_import_of_a_member_of_a_type_without_supertypes_is_judged_by_its_reach() {
  let object = |header: &str, reach: &str| format!("package p\n\nobject {header} {{\n  {reach}fun helper(id: String) {{}}\n\n  fun first() = 1\n}}\n");
  let test_of = |name: &str| format!("package q\n\nimport p.{name}.helper\n\nfun test() = run {{\n  helper(\"x\")\n}}\n");
  let (plain, panes) = ("impl/src/p/Plain.kt", "impl/src/p/Panes.kt");
  let mut range = Range::default();
  range
    .baseline(&[
      ("impl/src/p/Base.kt", "package p\n\nopen class Base\n"),
      (plain, &object("Plain", "private ")),
      (panes, &object("Panes : Base()", "private ")),
    ])
    .commit(&[
      (plain, Some(&object("Plain", "@VisibleForTesting internal "))),
      (panes, Some(&object("Panes : Base()", "@VisibleForTesting internal "))),
    ])
    .commit(&[("impl/testSrc/q/PlainTest.kt", Some(&test_of("Plain")))])
    .commit(&[("impl/testSrc/q/PanesTest.kt", Some(&test_of("Panes")))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("helper", Evidence::Import)])]);
}

/// A private member is a use target as far as the compiler lets it be one, for its current form too: in Java the
/// whole top-level class for a qualified use, so a private method of a nested class is used by its class name in the
/// class around it, and in Kotlin the class of a companion. A bare name or `this.name` in the Java class around it
/// means a member of that class (its own `helper` of the baseline, which the index doesn't know), and another
/// top-level class of the Java file, and the class around a nested Kotlin object, see neither
#[test]
fn a_private_member_is_a_use_in_the_top_level_java_class_and_the_class_of_its_companion() {
  let outer = |own: &str, helper: &str, uses: &str| {
    format!(
      "package p;\n\npublic class Outer {{\n  int helper() {{\n    return 2;\n  }}\n{own}\n  void first() {{}}\n\n  static class Inner {{\n    void second() {{}}\n{helper}  }}\n{uses}}}\n"
    )
  };
  let own = "\n  int own() {\n    return helper() + this.helper();\n  }\n";
  let helper = "\n    private static int helper() {\n      return 1;\n    }\n";
  let java_uses = "\n  int use() {\n    return Inner.helper();\n  }\n}\n\nclass Other {\n  int use() {\n    return Outer.Inner.helper();\n  }\n";
  let lanes = |create: &str, take: &str, uses: &str| {
    format!(
      "package p\n\nclass Lanes {{\n  fun first() = 1\n\n  companion object {{\n    fun second() = 2\n{create}  }}\n\n  object Pool {{\n    fun third() = 3\n{take}  }}\n{uses}}}\n"
    )
  };
  let (create, take) = ("\n    private fun create() = 1\n", "\n    private fun take() = 1\n");
  let kotlin_uses = "\n  fun copy() = create()\n\n  fun borrow() = Pool.take()\n";
  let (java, kotlin) = ("impl/src/p/Outer.java", "impl/src/p/Lanes.kt");
  let mut range = Range::default();
  range
    .baseline(&[(java, &outer("", "", "")), (kotlin, &lanes("", "", ""))])
    .commit(&[(java, Some(&outer("", helper, ""))), (kotlin, Some(&lanes(create, take, "")))])
    .commit(&[(java, Some(&outer("", helper, java_uses))), (kotlin, Some(&lanes(create, take, kotlin_uses)))])
    .commit(&[(java, Some(&outer(own, helper, java_uses)))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, vec![link("create", Evidence::SameFile), link("helper", Evidence::SameFile)])]
  );
}

const LANE: &str = "impl/src/p/Lane.kt";

/// `Lane` of the tests of constructors: a class of `header` (`class Lane(val id: Int)`) with two functions and `more`
fn lane(header: &str, more: &str) -> String {
  format!("package p\n\n{header} {{\n  fun first() = 1\n\n  fun second() = 2\n{more}}}\n")
}

/// A Kotlin file of the package `q` with `imports` of `p` and the declarations `uses`
fn uses_of(imports: &[&str], uses: &str) -> String {
  format!("package q\n\nimport p.{}\n\n{uses}\n", imports.join("\nimport p."))
}

/// A Java class `name` of the package `q` with `imports` and the members `members`
fn java_uses_of(imports: &str, name: &str, members: &str) -> String {
  format!("package q;\n\n{imports}\n\nclass {name} {{\n  {members}\n}}\n")
}

/// A parameter that a commit adds to a constructor of the baseline is what a call of the type that passes it needs, by
/// name or by position, also in the file of the type; a call that leaves it to its default compiles without the commit
#[test]
fn a_constructor_call_that_needs_a_new_parameter_is_an_edge() {
  let flag = "class Lane(val id: Int, val flag: Boolean = false)";
  let mut range = file_history(LANE, &lane("class Lane(val id: Int)", ""), &[lane(flag, "")]);
  range
    .commit(&[("impl/src/q/Named.kt", Some(&uses_of(&["Lane"], "fun use(id: Int) = Lane(id, flag = true)")))])
    .commit(&[("impl/src/q/Positional.kt", Some(&uses_of(&["Lane"], "fun use(id: Int) = Lane(id, true)")))])
    .commit(&[("impl/src/q/Old.kt", Some(&uses_of(&["Lane"], "fun use(id: Int) = Lane(id)")))])
    .commit(&[(LANE, Some(&lane(flag, "\n  fun copy() = Lane(id, flag = true)\n")))]);
  let import = || vec![link("Lane", Evidence::Import)];
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, import()), (2, 0, import()), (4, 0, vec![link("Lane", Evidence::SameFile)])]
  );
}

/// The shape of the test of IJAI-4074: an object expression whose supertype's constructor invocation passes a named
/// argument for the parameter that another commit adds to the `internal` primary constructor needs that commit, and so
/// do a plain call of the type with it and the invocation of a subclass, whose supertype is a receiver around it:
/// constructors are not inherited, so no supertype of the type or of a class around the call declares more of them.
/// The public secondary constructor rejects all three. The invocations of a subclass and of an object expression that
/// leave the parameter to its default compile without the commit
#[test]
fn a_supertype_constructor_call_with_a_new_named_argument_is_an_edge() {
  let manager = |parameter: &str| {
    format!(
      "package p\n\nopen class ToolWindowManagerImpl @NonInjectable @TestOnly internal constructor(\n  val project: Project,\n  @field:JvmField internal val isNewUi: Boolean,\n{parameter}  @JvmField internal val coroutineScope: CoroutineScope,\n) : ToolWindowManagerEx(), Disposable {{\n  constructor(project: Project, coroutineScope: CoroutineScope) : this(project, true, coroutineScope)\n\n  internal open fun getButtonManager(toolWindow: ToolWindow): ToolWindowButtonManager = buttons\n\n  fun first() = 1\n}}\n"
    )
  };
  let test = "internal class ToolWindowUnregisterTest {\n  private fun createManager(buttonManager: ToolWindowButtonManager): ToolWindowManagerImpl {\n    val coroutineScope = scope()\n    val manager = object : ToolWindowManagerImpl(project, isNewUi = false, isEdtRequired = false, coroutineScope = coroutineScope) {\n      override fun getButtonManager(toolWindow: ToolWindow): ToolWindowButtonManager = buttonManager\n    }\n    return manager\n  }\n}";
  let mut range = file_history(
    "impl/src/p/ToolWindowManagerImpl.kt",
    &manager(""),
    &[manager("  private val isEdtRequired: Boolean = true,\n")],
  );
  range
    .commit(&[("impl/testSrc/q/ToolWindowUnregisterTest.kt", Some(&uses_of(&["ToolWindowManagerImpl"], test)))])
    .commit(&[(
      "impl/testSrc/q/Create.kt",
      Some(&uses_of(
        &["ToolWindowManagerImpl"],
        "fun create() = ToolWindowManagerImpl(project, true, isEdtRequired = false, coroutineScope = scope)",
      )),
    )])
    .commit(&[(
      "impl/testSrc/q/TestManager.kt",
      Some(&uses_of(
        &["ToolWindowManagerImpl"],
        "class TestManager : ToolWindowManagerImpl(project, isNewUi = false, isEdtRequired = true, coroutineScope = scope)",
      )),
    )])
    .commit(&[(
      "impl/testSrc/q/Plain.kt",
      Some(&uses_of(
        &["ToolWindowManagerImpl"],
        "class Plain : ToolWindowManagerImpl(project, isNewUi = false, coroutineScope = scope)\n\nfun plain(): Any = object : ToolWindowManagerImpl(project, isNewUi = false, coroutineScope = scope) {}",
      )),
    )]);
  let import = || vec![link("ToolWindowManagerImpl", Evidence::Import)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, import()), (2, 0, import()), (3, 0, import())]);
}

/// A type position, an import and a call of a member of an instance mean the type, not a constructor: a constructor
/// parameter that a commit adds is no edge for them, while a call that passes it is; of a type the range adds they
/// need the commit that adds it, not the one that adds a constructor
#[test]
fn a_type_reference_is_no_edge_for_a_new_constructor_parameter() {
  let pane = |parameters: &str| format!("package p;\n\npublic class Pane {{\n  public Pane({parameters}) {{}}\n\n  public void first() {{}}\n}}\n");
  let mut range = Range::default();
  range
    .baseline(&[(LANE, &lane("class Lane(val id: Int)", "")), ("impl/src/p/Pane.java", &pane("int index"))])
    .commit(&[
      (LANE, Some(&lane("class Lane(val id: Int, val flag: Boolean = false)", ""))),
      ("impl/src/p/Pane.java", Some(&pane("int index, boolean focus"))),
    ])
    .commit(&[
      ("impl/src/q/Types.kt", Some(&uses_of(&["Lane"], "fun use(lane: Lane): Int = lane.id"))),
      ("impl/src/q/Types.java", Some(&java_uses_of("import p.Pane;", "Types", "Pane pane;"))),
    ])
    .commit(&[
      ("impl/src/q/Calls.kt", Some(&uses_of(&["Lane"], "fun use() = Lane(1, flag = true)"))),
      ("impl/src/q/Calls.java", Some(&java_uses_of("import p.Pane;", "Calls", "Object pane = new Pane(1, true);"))),
    ]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(2, 0, vec![link("Lane", Evidence::Import), link("Pane", Evidence::Import)])]
  );

  let mut range = Range::default();
  range
    .commit(&[(LANE, Some(&lane("class Lane(val id: Int)", "")))])
    .commit(&[(LANE, Some(&lane("class Lane(val id: Int)", "\n  constructor(name: String) : this(name.length)\n")))])
    .commit(&[("impl/src/q/Types.kt", Some(&uses_of(&["Lane"], "fun use(lane: Lane): Int = lane.id")))])
    .commit(&[("impl/src/q/Import.kt", Some(&uses_of(&["Lane"], "fun other() = 1")))]);
  let import = || vec![link("Lane", Evidence::Import)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, import()), (3, 0, import())]);
}

/// A call that the constructor of the baseline and the one a commit adds both accept may mean the old one: an
/// expression is no edge, a text that only the new one accepts is
#[test]
fn a_call_both_constructors_accept_is_no_edge() {
  let more = "\n  constructor(name: String) : this(name.length)\n";
  let mut range = file_history(LANE, &lane("class Lane(val id: Int)", ""), &[lane("class Lane(val id: Int)", more)]);
  range
    .commit(&[(
      "impl/src/q/Both.kt",
      Some(&uses_of(&["Lane"], "fun use(id: Int, name: String) = listOf(Lane(id), Lane(name), Lane(null))")),
    )])
    .commit(&[("impl/src/q/New.kt", Some(&uses_of(&["Lane"], "fun use() = Lane(\"x\")")))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, vec![link("Lane", Evidence::Import)])]);
}

/// Java's `new` means a constructor of the type its name resolves to, whatever the wildcard imports: a text that only
/// the constructor a commit adds accepts needs it, also for an anonymous class beside a wildcard import of another
/// package, while a number the constructor of the baseline accepts doesn't, nor does a `new` in a file that declares a
/// class of the name, which may be the one it means (`Nested.Pane` of the baseline)
#[test]
fn a_java_new_that_only_a_new_constructor_accepts_is_an_edge() {
  let pane = |more: &str| format!("package p;\n\npublic class Pane {{\n  public Pane(int index) {{}}\n\n  public void first() {{}}\n{more}}}\n");
  let nested = |more: &str| java_uses_of("import p.Pane;", "Nested", &format!("static class Pane {{\n    Pane(String id) {{}}\n  }}\n{more}"));
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Pane.java", &pane("")), ("impl/src/q/Nested.java", &nested(""))])
    .commit(&[("impl/src/p/Pane.java", Some(&pane("\n  public Pane(String id) {}\n")))])
    .commit(&[("impl/src/q/Text.java", Some(&java_uses_of("import p.Pane;", "Text", "Object pane = new Pane(\"x\");")))])
    .commit(&[("impl/src/q/Number.java", Some(&java_uses_of("import p.Pane;", "Number", "Object pane = new Pane(1);")))])
    .commit(&[(
      "impl/src/q/Anonymous.java",
      Some(&java_uses_of("import java.util.*;\nimport p.Pane;", "Anonymous", "Object pane = new Pane(\"y\") {};")),
    )])
    .commit(&[("impl/src/q/Nested.java", Some(&nested("\n  Object pane = new Pane(\"z\");")))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, vec![link("Pane", Evidence::Import)]), (3, 0, vec![link("Pane", Evidence::Import)])]
  );
}

/// A class that declares no constructor has the implicit `()`, which rejects a call with arguments: a Kotlin primary
/// constructor with a default and a Java constructor that a commit declares are what such a call needs, while the call
/// without arguments that the Kotlin one accepts too compiles without the commit
#[test]
fn an_implicit_constructor_takes_no_arguments() {
  let pane = |more: &str| format!("package p;\n\npublic class Pane {{\n  public void first() {{}}\n\n  public void second() {{}}\n{more}}}\n");
  let mut range = Range::default();
  range
    .baseline(&[(LANE, &lane("class Lane", "")), ("impl/src/p/Pane.java", &pane(""))])
    .commit(&[
      (LANE, Some(&lane("class Lane(val size: Int = 0)", ""))),
      ("impl/src/p/Pane.java", Some(&pane("\n  public Pane(int index) {}\n"))),
    ])
    .commit(&[("impl/src/q/Empty.kt", Some(&uses_of(&["Lane"], "fun use() = Lane()")))])
    .commit(&[("impl/src/q/Size.kt", Some(&uses_of(&["Lane"], "fun use() = Lane(1)")))])
    .commit(&[("impl/src/q/Index.java", Some(&java_uses_of("import p.Pane;", "Index", "Object pane = new Pane(1);")))]);
  let import = |name: &str| vec![link(name, Evidence::Import)];
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, import("Lane")), (3, 0, import("Pane"))]);
}

/// A top-level function of the name of a type (a factory `fun Lane(name: String): Lane`) shares its key, and neither
/// replaces the other: a commit that adds the function keeps the entries of the type, which a type position means alone,
/// and a commit that adds a constructor parameter keeps the function, which takes a call its parameters accept: a call
/// that only the function accepts needs its commit, in another file of the package, which only the type links, or beside
/// the type, while one that only the new form of the constructor accepts needs the commit of the parameter, as without
/// the function. A removal of the function keeps the type
#[test]
fn a_factory_function_and_its_type_share_a_key_without_losing_either() {
  let factory = "\nfun Lane(name: String): Lane = Lane(name.length)\n";
  let range_of = |beside: &str, second: (&str, &str)| {
    let mut range = Range::default();
    range
      .commit(&[(LANE, Some(&format!("{}{beside}", lane("class Lane(val id: Int)", ""))))])
      .commit(&[(second.0, Some(second.1))])
      .commit(&[(LANE, Some(&format!("{}{beside}", lane("class Lane(val id: Int, val flag: Boolean = false)", ""))))])
      .commit(&[("impl/src/q/Use.kt", Some(&uses_of(&["Lane"], "fun use(): Lane = Lane(1, flag = true)")))])
      .commit(&[("impl/src/p/Name.kt", Some("package p\n\nfun name() = Lane(\"x\")\n"))]);
    range.in_modules(&["impl"])
  };
  let other = ("impl/src/p/Other.kt", "package p\n\nfun other() = 1\n");
  let elsewhere = range_of("", ("impl/src/p/Lanes.kt", &format!("package p\n{factory}")));
  let import = || vec![link("Lane", Evidence::Import)];
  let package = || vec![link("Lane", Evidence::SamePackageType)];
  assert_eq!(edges(&elsewhere), [(3, 0, import()), (3, 2, import()), (4, 1, package())]);
  assert_eq!(edges(&range_of(factory, other)), [(3, 0, import()), (3, 2, import()), (4, 0, package())]);
  assert_eq!(edges(&range_of("", other)), [(3, 0, import()), (3, 2, import())]);

  // the removal of a function of the baseline of the name keeps the entries of the type
  let mut range = Range::default();
  range
    .baseline(&[("impl/src/p/Lanes.kt", &format!("package p\n{factory}"))])
    .commit(&[(LANE, Some(&lane("class Lane(val id: Int)", "")))])
    .commit(&[("impl/src/p/Lanes.kt", None)])
    .commit(&[("impl/src/q/Use.kt", Some(&uses_of(&["Lane"], "val lane: Lane? = null")))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(2, 0, import())]);
}

/// A type the range adds keeps the edge of every use that it or one of its constructors accepts, as before its
/// constructors: a call of a Kotlin class without constructors and of a Java one, a Kotlin constructor invocation of a
/// supertype by a subclass and by an object expression, a call of a `fun interface`, which has no constructors (a SAM
/// constructor), and a type position. Each in its own file of the package, which no import line links
#[test]
fn a_new_type_keeps_its_edges_for_calls_and_type_positions() {
  let mut range = Range::default();
  range
    .commit(&[
      (LANE, Some("package p\n\nclass Lane\n")),
      ("impl/src/p/Pane.java", Some("package p;\n\npublic class Pane {\n}\n")),
      ("impl/src/p/Ring.kt", Some("package p\n\nopen class Ring(val size: Int = 0)\n")),
      ("impl/src/p/Listener.kt", Some("package p\n\nfun interface Listener {\n  fun on()\n}\n")),
    ])
    .commit(&[("impl/src/p/Call.kt", Some("package p\n\nfun lane() = Lane()\n"))])
    .commit(&[("impl/src/p/New.java", Some("package p;\n\nclass New {\n  Object pane = new Pane();\n}\n"))])
    .commit(&[("impl/src/p/Sub.kt", Some("package p\n\nclass Big : Ring(size = 2)\n"))])
    .commit(&[("impl/src/p/Object.kt", Some("package p\n\nfun ring(): Any = object : Ring() {}\n"))])
    .commit(&[("impl/src/p/Sam.kt", Some("package p\n\nfun listener() = Listener { }\n"))])
    .commit(&[("impl/src/p/Type.kt", Some("package p\n\nval lane: Lane? = null\n"))]);
  let package = |name: &str| vec![link(name, Evidence::SamePackageType)];
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [
      (1, 0, package("Lane")),
      (2, 0, package("Pane")),
      (3, 0, package("Ring")),
      (4, 0, package("Ring")),
      (5, 0, package("Listener")),
      (6, 0, package("Lane"))
    ]
  );
}

/// A type whose reach a commit widens is what a use needs that the old reach forbade, a call of its constructor by its
/// qualified name too, as a constructor has the reach of its type besides its own: a Java package-private class made
/// public and constructed from another package, a Kotlin `internal` class made public and used or constructed from
/// another module, whose implicit `()` is public in both forms; a use from its package or module, which the old reach
/// allowed, needs no commit, nor does a Java use of the Kotlin class from another module, as Java sees an `internal`
/// class as public
#[test]
fn a_widened_type_is_an_edge_for_a_use_the_old_reach_forbids() {
  let pane = |header: &str| format!("package p;\n\n{header} {{\n  void first() {{}}\n\n  void second() {{}}\n}}\n");
  let (java, kotlin_use) = ("impl/src/p/Pane.java", uses_of(&["Lane"], "fun use(): Lane = Lane()"));
  let mut range = Range::default();
  range
    .baseline(&[(LANE, &lane("internal class Lane", "")), (java, &pane("class Pane"))])
    .commit(&[(LANE, Some(&lane("class Lane", ""))), (java, Some(&pane("public class Pane")))])
    .commit(&[
      ("impl/src/q/Near.kt", Some(&kotlin_use)),
      ("impl/src/p/Near.java", Some("package p;\n\nclass Near {\n  Object pane = new Pane();\n}\n")),
    ])
    .commit(&[
      ("other/src/q/Far.kt", Some(&kotlin_use)),
      ("impl/src/q/Far.java", Some(&java_uses_of("", "Far", "Object pane = new p.Pane();"))),
    ])
    .commit(&[("other/src/q/Call.kt", Some("package q\n\nfun call(): Any = p.Lane()\n"))])
    .commit(&[("other/src/q/FarJava.java", Some(&java_uses_of("import p.Lane;", "FarJava", "Object lane = new Lane();")))]);
  assert_eq!(
    edges(&range.in_modules(&["impl", "other"])),
    [
      (2, 0, vec![link("Lane", Evidence::Import), link("Pane", Evidence::Qualified)]),
      (3, 0, vec![link("Lane", Evidence::Qualified)])
    ]
  );
}

/// A constructor whose reach a commit widens is what a call needs that the old reach forbade: a private one made
/// `internal` for a call from another file of its module. Not where the companion's `invoke` may take the call, nor
/// where a function of the name that two wildcard imports may bring may take it while the constructor is private, nor
/// from another module, which the `internal` one doesn't reach either
#[test]
fn a_widened_constructor_is_an_edge_for_a_call_the_old_reach_forbids() {
  let invoke = "\n  companion object {\n    operator fun invoke(id: Int): Ring = Ring(id)\n  }\n";
  let ring = "impl/src/p/Ring.kt";
  let ring_of = |header: &str| lane(header, invoke).replace("Lane", "Ring");
  let mut range = Range::default();
  range
    .baseline(&[
      (LANE, &lane("class Lane private constructor(val id: Int)", "")),
      (ring, &ring_of("class Lane private constructor(val id: Int)")),
    ])
    .commit(&[
      (LANE, Some(&lane("class Lane internal constructor(val id: Int)", ""))),
      (ring, Some(&ring_of("class Lane internal constructor(val id: Int)"))),
    ])
    .commit(&[("impl/src/q/Use.kt", Some(&uses_of(&["Lane", "Ring"], "fun use() = listOf(Lane(1), Ring(2))")))])
    .commit(&[("other/src/q/Far.kt", Some(&uses_of(&["Lane"], "fun far() = Lane(1)")))])
    .commit(&[("impl/src/q/Wildcards.kt", Some("package q\n\nimport p.*\nimport r.*\n\nfun use() = Lane(1)\n"))]);
  assert_eq!(edges(&range.in_modules(&["impl", "other"])), [(1, 0, vec![link("Lane", Evidence::Import)])]);
}

/// A Kotlin call of a type falls back from its constructors to the functions of its name: where the file of the call
/// declares one, or imports names that may bring one (a wildcard import beside the import of the type), and in the file
/// of a type of the baseline beside a function of its name, a call that only the new form of a constructor accepts may
/// mean that function, so it is no edge; the same call through an import of the type alone, or in the type's own file,
/// is
#[test]
fn a_kotlin_call_that_a_function_of_its_name_may_take_is_no_edge() {
  let flag = "class Lane(val id: Int, val flag: Boolean = false)";
  let call = "fun use(id: Int) = Lane(id, flag = true)";
  let factory = "\nfun Lane(id: Int, flag: Boolean): Lane = Lane(id)\n";
  let ring = "impl/src/p/Ring.kt";
  let ring_of = |header: &str, more: &str| format!("{}{more}", lane(header, "")).replace("Lane", "Ring");
  let mut range = Range::default();
  range
    .baseline(&[(LANE, &lane("class Lane(val id: Int)", "")), (ring, &ring_of("class Lane(val id: Int)", factory))])
    .commit(&[(LANE, Some(&lane(flag, ""))), (ring, Some(&ring_of(flag, factory)))])
    .commit(&[(
      "impl/src/q/Declared.kt",
      Some(&uses_of(&["Lane"], &format!("{call}\n\nclass Holder {{\n  fun Lane(id: Int, flag: Boolean): Any = id\n}}"))),
    )])
    .commit(&[("impl/src/q/Wildcard.kt", Some(&format!("package q\n\nimport p.Lane\nimport r.*\n\n{call}\n")))])
    .commit(&[(ring, Some(&ring_of(flag, &format!("{factory}\nfun use(id: Int) = Lane(id, flag = true)\n"))))])
    .commit(&[("impl/src/q/Imported.kt", Some(&uses_of(&["Lane"], call)))])
    .commit(&[(LANE, Some(&lane(flag, "\n  fun copy() = Lane(id, flag = true)\n")))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(4, 0, vec![link("Lane", Evidence::Import)]), (5, 0, vec![link("Lane", Evidence::SameFile)])]
  );
}

/// A commit that removes a type from its file, where a function of its name stays, removes the type's entries though a
/// function of its name in another file keeps the key: a type position is no use of the type then, while the import
/// still names that function and needs its commit; where the type stays, the type position needs the commit that adds
/// it, and the import may mean either
#[test]
fn a_type_removed_beside_a_function_of_its_name_leaves_no_entries() {
  let factory = "package p\n\nfun Lane(name: String): Any = name\n";
  let range_of = |second: (&str, &str)| {
    let mut range = Range::default();
    range
      .baseline(&[(LANE, factory)])
      .commit(&[("impl/src/p/Lanes.kt", Some("package p\n\nfun Lane(id: Int, flag: Boolean): Any = id\n"))])
      .commit(&[(LANE, Some(&format!("{factory}\nclass Lane(val id: Int)\n")))])
      .commit(&[(second.0, Some(second.1))])
      .commit(&[("impl/src/q/Use.kt", Some(&uses_of(&["Lane"], "val lane: Lane? = null")))]);
    range.in_modules(&["impl"])
  };
  let import = || vec![link("Lane", Evidence::Import)];
  assert_eq!(edges(&range_of((LANE, factory))), [(3, 0, import())]);
  assert_eq!(edges(&range_of(("impl/src/p/Other.kt", "package p\n\nfun other() = 1\n"))), [(3, 1, import())]);
}

/// Two constructors that the summary can't tell apart, of the same last names of their types or of a type parameter,
/// are both entries: a call of a type the range adds that one of them accepts keeps its edge, while a commit that
/// changes a type of the baseline with such twins is no step of its constructors, which can't be matched across the
/// sides: a call that the old primary constructor, or the public one, accepts compiles without it
#[test]
fn constructors_the_summary_cannot_tell_apart_are_no_form() {
  let box_of = |header: &str| format!("package p\n\n{header} {{\n  constructor(list: List<T>) : this(list.first())\n\n  fun first() = 1\n}}\n");
  let holder =
    |modifier: &str| format!("package p;\n\nimport java.util.List;\n\npublic class Holder<T> {{\n  public Holder(T value) {{}}\n\n  {modifier}Holder(List<T> values) {{}}\n}}\n");
  let (box_path, holder_path) = ("impl/src/p/Box.kt", "impl/src/p/Holder.java");
  let mut range = Range::default();
  range
    .baseline(&[(box_path, &box_of("class Box<T>(val value: T)")), (holder_path, &holder("private "))])
    .commit(&[
      (box_path, Some(&box_of("class Box<T>(val value: T, val flag: Boolean = false)"))),
      (holder_path, Some(&holder(""))),
    ])
    .commit(&[
      ("impl/src/q/Use.kt", Some(&uses_of(&["Box"], "fun use() = Box(value = 1)"))),
      ("impl/src/p/Use.java", Some("package p;\n\nclass Use {\n  Object holder = new Holder<>(\"x\");\n}\n")),
    ])
    .commit(&[
      (
        "impl/src/p/Lane.kt",
        Some("package p\n\nclass Lane<T>(val value: T) {\n  constructor(list: List<T>) : this(list.first())\n}\n"),
      ),
      (
        "impl/src/p/Stamp.java",
        Some("package p;\n\npublic class Stamp {\n  public Stamp(java.util.Date date) {}\n\n  private Stamp(java.sql.Date date) {}\n}\n"),
      ),
    ])
    .commit(&[
      ("impl/src/p/Lanes.kt", Some("package p\n\nfun use() = Lane(value = 1)\n")),
      (
        "impl/src/q/Stamps.java",
        Some(&java_uses_of("", "Stamps", "Object stamp = new p.Stamp(new java.util.Date());")),
      ),
    ]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(3, 2, vec![link("Lane", Evidence::SamePackageType), link("Stamp", Evidence::Qualified)])]
  );
}

/// A Kotlin call of a type may mean a top-level function of its name, which no evidence links from another file of
/// the package: the type stands for such a function that the range adds, so a call that none of the constructors
/// accepts keeps the edge to the commit that adds the type and the function, beside it or in another file
#[test]
fn a_call_that_a_function_of_the_name_of_a_new_type_may_take_needs_the_type() {
  let ring = "package p\n\nclass Ring private constructor(val id: Int) {\n  companion object {\n    fun create(id: Int) = Ring(id)\n  }\n}\n";
  let mut range = Range::default();
  range
    .commit(&[(LANE, Some("package p\n\nclass Lane(val id: Int)\n\nfun Lane(name: String): Lane = Lane(name.length)\n"))])
    .commit(&[("impl/src/p/Use.kt", Some("package p\n\nfun use() = Lane(\"x\")\n"))])
    .commit(&[
      ("impl/src/p/Ring.kt", Some(ring)),
      ("impl/src/p/Rings.kt", Some("package p\n\nfun Ring(name: String): Ring = Ring.create(name.length)\n")),
    ])
    .commit(&[("impl/src/p/Uses.kt", Some("package p\n\nfun use() = Ring(\"x\")\n"))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, vec![link("Lane", Evidence::SamePackageType)]), (3, 2, vec![link("Ring", Evidence::SamePackageType)])]
  );
}

/// A wildcard import brings a type only where no type of its name in the package of the use takes the name first,
/// which another file of it may declare (`q.Pane` beside `import p.*`): a call through one needs the type as it came,
/// so a constructor that a commit adds to a type of the baseline is no edge, while a type the range adds keeps its
/// edge, and so does a call through an import of the type, which takes the name before the package
#[test]
fn a_call_through_a_wildcard_import_needs_only_the_type() {
  let pane = |more: &str| format!("package p;\n\npublic class Pane {{\n  public Pane(int index) {{}}\n\n  public void first() {{}}\n{more}}}\n");
  let more = "\n  constructor(name: String) : this(name.length)\n";
  let mut range = Range::default();
  range
    .baseline(&[
      ("impl/src/p/Pane.java", &pane("")),
      ("impl/src/q/Pane.java", "package q;\n\nclass Pane {\n  Pane(String id) {}\n}\n"),
      (LANE, &lane("class Lane(val id: Int)", "")),
      ("impl/src/q/Lane.kt", "package q\n\nclass Lane(val name: String)\n"),
    ])
    .commit(&[
      ("impl/src/p/Pane.java", Some(&pane("\n  public Pane(String id) {}\n"))),
      (LANE, Some(&lane("class Lane(val id: Int)", more))),
    ])
    .commit(&[
      ("impl/src/q/Use.java", Some(&java_uses_of("import p.*;", "Use", "Object pane = new Pane(\"x\");"))),
      ("impl/src/q/Use.kt", Some("package q\n\nimport p.*\n\nfun use() = Lane(\"x\")\n")),
    ])
    .commit(&[
      (
        "impl/src/q/Explicit.java",
        Some(&java_uses_of("import p.Pane;", "Explicit", "Object pane = new Pane(\"x\");")),
      ),
      ("impl/src/q/Explicit.kt", Some(&uses_of(&["Lane"], "fun use() = Lane(\"x\")"))),
    ])
    .commit(&[("impl/src/p/Ring.kt", Some("package p\n\nclass Ring(val id: Int)\n"))])
    .commit(&[("impl/src/q/Rings.kt", Some("package q\n\nimport p.*\n\nfun use() = Ring(1)\n"))]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [
      (2, 0, vec![link("Lane", Evidence::Import), link("Pane", Evidence::Import)]),
      (4, 3, vec![link("Ring", Evidence::StarImport)])
    ]
  );
}

/// Where no build file tells the modules, an `internal` constructor reaches a call as its public type does, so a call
/// of a type the range adds keeps its edge, as before its constructors; a private one doesn't
#[test]
fn an_internal_constructor_of_no_known_module_takes_a_call() {
  let mut range = Range::default();
  range
    .commit(&[
      ("src/p/Lane.kt", Some("package p\n\nclass Lane internal constructor(val id: Int)\n")),
      ("src/p/Ring.kt", Some("package p\n\nclass Ring private constructor(val id: Int)\n")),
    ])
    .commit(&[("src/q/Use.kt", Some("package q\n\nfun use() = listOf(p.Lane(1), p.Ring(2))\n"))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(1, 0, vec![link("Lane", Evidence::Qualified)])]);
}

/// A line of parse errors that leaves a class of `long_class` clean and its file not degraded, but its constructors
/// unknown
const PARSE_ERRORS: &str = "  @@@ ### $$$ %%% ^^^ &&& *** ((( )))\n";

/// A Kotlin class of the package `p` of `header` (`class Foo`) with `more` and eighty functions, enough bytes for a line
/// of `PARSE_ERRORS` in it to leave its file not degraded
fn long_class(header: &str, more: &str) -> String {
  let functions = (0..80).map(|it| format!("\n  fun function{it}() = {it}\n")).collect::<Vec<_>>().concat();
  let text = format!("package p\n\n{header} {{\n{more}{functions}}}\n");
  if more.contains(PARSE_ERRORS) {
    let summary = summarize(Language::Kotlin, text.as_bytes());
    let class = summary.declarations.iter().find(|it| it.kind == DeclKind::Type && it.container.is_none()).unwrap();
    assert!(class.constructors.is_none() && !class.in_error && !summary.is_degraded(), "{summary:?}");
  }
  text
}

/// The constructors of a class with a parse error in it are unknown, as the error may have swallowed one: a commit that
/// breaks the class or mends it is no step of its constructors, so a call that a constructor of the baseline accepts
/// is no edge to the mend, while one that only a constructor a later commit adds accepts needs that commit
#[test]
fn a_class_with_a_parse_error_in_it_is_no_step_of_its_constructors() {
  let foo = |more: &str| long_class("class Foo", &format!("  constructor(name: String)\n{more}"));
  let path = "impl/src/p/Foo.kt";
  let mut range = Range::default();
  range
    .baseline(&[(path, &foo(""))])
    .commit(&[(path, Some(&foo(PARSE_ERRORS)))])
    .commit(&[(path, Some(&foo("")))])
    .commit(&[("impl/src/q/Name.kt", Some(&uses_of(&["Foo"], "fun use() = Foo(\"y\")")))])
    .commit(&[(path, Some(&foo("\n  constructor(id: Int, flag: Boolean) : this(\"\")\n")))])
    .commit(&[("impl/src/q/Flag.kt", Some(&uses_of(&["Foo"], "fun use() = Foo(1, true)")))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(4, 3, vec![link("Foo", Evidence::Import)])]);
}

/// A commit whose sides of a type can't be matched, twin constructors on one side or constructors that one side knows
/// and the other doesn't (a parse error in the class), leaves its key only present or absent, also of a type the range
/// adds: its constructors judge no call then, as they may miss what the commit gave them, and the type takes any call,
/// from the start for a new class with twin constructors. So a call that passes a parameter such a commit adds needs
/// the commit that added the type, by twin constructors of a new class and by a parameter that comes with a parse
/// error, while over a clean class it needs the parameter's
#[test]
fn a_type_whose_sides_a_commit_cannot_match_takes_any_call() {
  let graph = |versions: &[String], call: &str| {
    let mut range = Range::default();
    for version in versions {
      range.commit(&[(LANE, Some(version))]);
    }
    range.commit(&[("impl/src/p/Use.kt", Some(&format!("package p\n\n{call}\n")))]);
    edges(&range.in_modules(&["impl"]))
  };
  let twins = |parameter: &str| {
    lane(
      &format!("class Lane(val date: java.util.Date{parameter})"),
      "\n  constructor(date: java.sql.Date) : this(java.util.Date(date.time))\n",
    )
  };
  let package = || vec![link("Lane", Evidence::SamePackageType)];
  assert_eq!(graph(&[twins("")], "fun use() = Lane()"), [(1, 0, package())]);
  assert_eq!(
    graph(
      &[twins(""), twins(", val flag: Boolean = false")],
      "fun use(date: java.util.Date) = Lane(date, flag = true)"
    ),
    [(2, 0, package())]
  );
  let (header, flag) = ("class Lane(val id: Int)", "class Lane(val id: Int, val flag: Boolean = false)");
  let call = "fun use() = Lane(1, flag = true)";
  assert_eq!(graph(&[long_class(header, ""), long_class(flag, PARSE_ERRORS)], call), [(2, 0, package())]);
  assert_eq!(graph(&[long_class(header, ""), long_class(flag, "")], call), [(2, 1, package())]);
}

/// A constructor that a type the range adds gets while its key is only present or absent (by the commit that mends a
/// parse error in the class) is as old as the type, which it needs, once a later commit changes the type on clean
/// sides: a call that it accepts needs the commit that added the type, not the baseline
#[test]
fn a_constructor_that_comes_while_its_type_takes_any_call_is_as_old_as_the_type() {
  let header = "class Lane(val id: Int)";
  let mut range = Range::default();
  range
    .commit(&[(LANE, Some(&long_class(header, PARSE_ERRORS)))])
    .commit(&[(LANE, Some(&long_class(header, "")))])
    .commit(&[(LANE, Some(&long_class(header, "\n  fun last() = 0\n")))])
    .commit(&[("impl/src/p/Use.kt", Some("package p\n\nfun use() = Lane(1)\n"))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(3, 0, vec![link("Lane", Evidence::SamePackageType)])]);
}

/// A function of the name of a type, a factory `fun Lane(name: String)` or an `invoke` of its companion, takes a call of
/// the type only when its parameters accept it, by the arguments that tell the constructors apart, and the call needs
/// the factory's commit then. So a call that passes a parameter a commit adds to the constructor of a type of the
/// baseline needs that commit beside a factory that rejects it, in the package of the type and through an import, and
/// so does one of a type the range adds whose companion's `invoke` rejects it, while a call that only the factory
/// accepts needs the factory's commit, which only the type links from another file of the package, and one that only
/// the `invoke` accepts the commit of the type
#[test]
fn a_function_of_the_name_of_a_type_takes_only_a_call_its_parameters_accept() {
  let ring = |header: &str| format!("package p\n\n{header} {{\n  companion object {{\n    operator fun invoke(name: String): Ring = Ring(name.length)\n  }}\n}}\n");
  let (rings, flag) = ("impl/src/p/Ring.kt", "val flag: Boolean = false");
  let mut range = Range::default();
  range
    .baseline(&[(LANE, &lane("class Lane(val id: Int)", ""))])
    .commit(&[("impl/src/p/Lanes.kt", Some("package p\n\nfun Lane(name: String): Lane = Lane(name.length)\n"))])
    .commit(&[(LANE, Some(&lane(&format!("class Lane(val id: Int, {flag})"), "")))])
    .commit(&[("impl/src/p/Flag.kt", Some("package p\n\nfun flag() = Lane(1, flag = true)\n"))])
    .commit(&[("impl/src/q/Flag.kt", Some(&uses_of(&["Lane"], "fun flag() = Lane(1, flag = true)")))])
    .commit(&[("impl/src/p/Name.kt", Some("package p\n\nfun name() = Lane(\"x\")\n"))])
    .commit(&[(rings, Some(&ring("class Ring(val id: Int)")))])
    .commit(&[(rings, Some(&ring(&format!("class Ring(val id: Int, {flag})"))))])
    .commit(&[("impl/src/p/Rings.kt", Some("package p\n\nfun rings() = listOf(Ring(1, flag = true), Ring(\"x\"))\n"))]);
  let graph = range.in_modules(&["impl"]);
  let package = |name: &str| vec![link(name, Evidence::SamePackageType)];
  assert_eq!(
    edges(&graph),
    [
      (2, 1, package("Lane")),
      (3, 1, vec![link("Lane", Evidence::Import)]),
      (4, 0, package("Lane")),
      (7, 5, package("Ring")),
      (7, 6, package("Ring"))
    ]
  );
  let factory = graph.edges.iter().find(|edge| (edge.from, edge.to) == (4, 0)).unwrap();
  assert_eq!(factory.links[0].declared_in, "impl/src/p/Lanes.kt");
}

/// A function of the name of a type that a commit adds to the type's file, or removes from it, is of its own kind
/// there, as in another file: a call that only it accepts needs the commit that adds it (whose call of the constructor
/// needs the type's), and once a commit removes it a call that only a constructor a later commit adds accepts needs
/// the constructor's commit
#[test]
fn a_function_of_the_name_of_a_type_comes_and_goes_beside_it() {
  let (header, factory) = ("class Lane(val id: Int)", "\nfun Lane(name: String): Lane = Lane(name.length)\n");
  let call = ("impl/src/p/Use.kt", Some("package p\n\nfun use() = Lane(\"x\")\n"));
  let with_factory = format!("{}{factory}", lane(header, ""));
  let package = || vec![link("Lane", Evidence::SamePackageType)];
  let mut range = Range::default();
  range.commit(&[(LANE, Some(&lane(header, "")))]).commit(&[(LANE, Some(&with_factory))]).commit(&[call]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(1, 0, vec![link("Lane", Evidence::SameFile)]), (2, 1, package())]);

  let mut range = Range::default();
  range
    .commit(&[(LANE, Some(&with_factory))])
    .commit(&[(LANE, Some(&lane(header, "")))])
    .commit(&[(LANE, Some(&lane(header, "\n  constructor(name: String) : this(name.length)\n")))])
    .commit(&[call]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(3, 2, package())]);
}

/// A function of the name of a type takes a call by its parameters as they are now: a commit that changes them keeps
/// its origin, so a call that only its new form accepts needs the commit that added it
#[test]
fn a_function_of_the_name_of_a_type_takes_a_call_by_its_parameters_now() {
  let factory = |parameters: &str| format!("package p\n\nfun first() = 0\n\nfun Lane({parameters}): Lane = TODO(name)\n\nfun last() = 9\n");
  let path = "impl/src/p/Factory.kt";
  let mut range = Range::default();
  range
    .commit(&[(LANE, Some(&lane("class Lane(val id: Int)", "")))])
    .commit(&[(path, Some(&factory("name: String")))])
    .commit(&[(path, Some(&factory("name: String, flag: Boolean")))])
    .commit(&[("impl/src/p/Use.kt", Some("package p\n\nfun use() = Lane(\"x\", true)\n"))]);
  assert_eq!(edges(&range.in_modules(&["impl"])), [(3, 1, vec![link("Lane", Evidence::SamePackageType)])]);
}

/// A function of the name of a type stands in for the type only where the use sees it, so a call that a constructor
/// accepts keeps the constructor's commit beside a private one of another file, one of another module of the package
/// and a Rust item of the same path; one the use sees accepts the call too, and with it the call is ambiguous
#[test]
fn a_function_of_the_name_of_a_type_that_the_use_does_not_see_takes_no_call() {
  let graph = |function: (&str, &str)| {
    let mut range = Range::default();
    range
      .commit(&[(function.0, Some(function.1))])
      .commit(&[(LANE, Some(&lane("class Lane(val id: Int)", "")))])
      .commit(&[("impl/src/p/Use.kt", Some("package p\n\nfun use() = Lane(\"x\".length)\n"))]);
    let (build, cargo) = ([file("BUILD.bazel")], [file("Cargo.toml")]);
    edges(&range.graph(&[("impl", &build), ("fixtures", &build), ("rust", &cargo)], &[], &[]))
  };
  let package = || vec![link("Lane", Evidence::SamePackageType)];
  let private = "package p\n\nprivate fun Lane(name: String): Any = name\n\nfun helper() = Lane(\"h\")\n";
  assert_eq!(graph(("impl/src/p/Helpers.kt", private)), [(2, 1, package())]);
  let public = "package p\n\nfun Lane(name: String): Any = name\n";
  assert_eq!(graph(("fixtures/src/p/Lanes.kt", public)), [(2, 1, package())]);
  assert_eq!(graph(("rust/src/p.rs", "pub fn Lane(name: &str) {}\n")), [(2, 1, package())]);
  assert_eq!(graph(("impl/src/p/Lanes.kt", public)), []);
}

/// A constructor parameter that a commit adds to a nested class, Kotlin's and Java's, is what a call of it by its
/// qualified name needs that passes it, while a call that leaves it to its default, or that Java's old constructor,
/// which the commit keeps, accepts, compiles without the commit
#[test]
fn a_call_of_a_nested_class_that_needs_a_new_parameter_is_an_edge() {
  let outer = |header: &str| format!("package p\n\nclass Outer {{\n  {header}\n\n  fun first() = 1\n}}\n");
  let frame =
    |more: &str| format!("package p;\n\npublic class Frame {{\n  public static class Inner {{\n    public Inner(int id) {{}}\n{more}  }}\n\n  public void first() {{}}\n}}\n");
  let (kotlin, java) = ("impl/src/p/Outer.kt", "impl/src/p/Frame.java");
  let mut range = Range::default();
  range
    .baseline(&[(kotlin, &outer("class Inner(val id: Int)")), (java, &frame(""))])
    .commit(&[
      (kotlin, Some(&outer("class Inner(val id: Int, val flag: Boolean = false)"))),
      (java, Some(&frame("\n    public Inner(int id, boolean flag) {}\n"))),
    ])
    .commit(&[
      ("impl/src/q/Flag.kt", Some(&uses_of(&["Outer"], "fun use() = Outer.Inner(1, flag = true)"))),
      (
        "impl/src/q/Flag.java",
        Some(&java_uses_of("import p.Frame;", "Flag", "Object inner = new Frame.Inner(1, true);")),
      ),
    ])
    .commit(&[
      ("impl/src/q/Plain.kt", Some(&uses_of(&["Outer"], "fun use() = Outer.Inner(1)"))),
      (
        "impl/src/q/Plain.java",
        Some(&java_uses_of("import p.Frame;", "Plain", "Object inner = new Frame.Inner(1);")),
      ),
    ]);
  assert_eq!(
    edges(&range.in_modules(&["impl"])),
    [(1, 0, vec![link("Inner", Evidence::Qualified), link("Inner", Evidence::Qualified)])]
  );
}

/// `a.Base` of the baseline, whose primary constructor a commit gives the parameter `name` with a default
fn base_with_a_new_parameter() -> Range {
  let base = |parameters: &str| format!("package a\n\nopen class Base({parameters})\n");
  let mut range = Range::default();
  range
    .baseline(&[("src/a/Base.kt", &base("val size: Int"))])
    .commit(&[("src/a/Base.kt", Some(&base("val size: Int, val name: String = \"\"")))]);
  range
}

/// `super(…)` of a constructor calls a constructor of the superclass: the Java one of `extends`, beside an interface,
/// and the Kotlin one of the only supertype of a class without a primary constructor. One that passes the new
/// parameter needs its commit, one that leaves it to its default doesn't
#[test]
fn a_super_call_that_needs_a_new_constructor_parameter_is_an_edge() {
  let mut range = base_with_a_new_parameter();
  range
    .commit(&[(
      "src/b/Sub.kt",
      Some("package b\n\nimport a.Base\n\nclass Sub : Base {\n  constructor(size: Int) : super(size, \"sub\")\n}\n"),
    )])
    .commit(&[(
      "src/b/JavaSub.java",
      Some("package b;\n\nimport a.Base;\n\nclass JavaSub extends Base implements Runnable {\n  JavaSub() {\n    super(1, \"java\");\n  }\n\n  public void run() {}\n}\n"),
    )])
    .commit(&[(
      "src/b/Old.kt",
      Some("package b\n\nimport a.Base\n\nclass Old : Base {\n  constructor(size: Int) : super(size)\n}\n"),
    )]);
  let base = || vec![link("Base", Evidence::Import)];
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(1, 0, base()), (2, 0, base())]);
}

/// The summary doesn't tell a class from an interface: beside another supertype `super(…)` may mean either
#[test]
fn a_kotlin_super_call_with_two_supertypes_is_no_edge() {
  let mut range = base_with_a_new_parameter();
  range
    .commit(&[(
      "src/b/Two.kt",
      Some("package b\n\nimport a.Base\n\nclass Two : Base, Runnable {\n  constructor(size: Int) : super(size, \"two\")\n\n  override fun run() {}\n}\n"),
    )])
    .commit(&[(
      "src/b/One.kt",
      Some("package b\n\nimport a.Base\n\nclass One : Base {\n  constructor(size: Int) : super(size, \"one\")\n}\n"),
    )]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(2, 0, vec![link("Base", Evidence::Import)])]);
}

/// `this(…)` calls a constructor of its own class: one its own commit gives a parameter needs no other commit, one of a
/// later commit that passes that parameter needs it, and one that leaves it to its default doesn't
#[test]
fn a_this_call_is_no_edge_across_its_own_commit() {
  let lane = |parameters: &str, secondary: &str| format!("package a\n\nclass Lane({parameters}) {{\n{secondary}  fun id() = 1\n}}\n");
  let added = "  constructor(name: String) : this(name.length, true)\n\n";
  let mut range = Range::default();
  range
    .baseline(&[("src/a/Lane.kt", &lane("val size: Int", ""))])
    .commit(&[("src/a/Lane.kt", Some(&lane("val size: Int, val flag: Boolean = false", added)))])
    .commit(&[(
      "src/a/Lane.kt",
      Some(&lane("val size: Int, val flag: Boolean = false", &format!("{added}  constructor() : this(0, true)\n\n"))),
    )])
    .commit(&[(
      "src/a/Lane.kt",
      Some(&lane(
        "val size: Int, val flag: Boolean = false",
        &format!("{added}  constructor() : this(0, true)\n\n  constructor(size: Long) : this(size.toInt())\n\n"),
      )),
    )]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(1, 0, vec![link("Lane", Evidence::SameFile)])]);
}

/// `a.Base` with `members` beside `fun other()`, open, `: supertypes` when given
fn base(supertypes: &str, members: &str) -> String {
  format!("package a\n\nopen class Base{supertypes} {{\n  fun other() = 1\n{members}}}\n")
}

/// A range whose first commit adds `members` to `a.Base` (`base`) of the baseline
fn adding_to_base(supertypes: &str, members: &str) -> Range {
  let mut range = Range::default();
  range
    .baseline(&[("src/a/Base.kt", &base(supertypes, ""))])
    .commit(&[("src/a/Base.kt", Some(&base(supertypes, members)))]);
  range
}

/// `b.Sub : <supertypes>` with `import a.Base` and a function whose body is `body`
fn sub(supertypes: &str, body: &str) -> String {
  format!("package b\n\nimport a.Base\n\nclass Sub : {supertypes} {{\n  fun use() {{\n    {body}\n  }}\n}}\n")
}

const OPEN: &str = "  open fun open(id: Int) = id\n";

/// A subclass calls a member it gets from its one supertype by a bare name or `this`: that commit's, by the supertype
/// through an import, its full path or the same package and module, from Kotlin and Java
#[test]
fn a_bare_call_of_a_member_the_supertype_gets_is_an_edge() {
  let mut range = adding_to_base("", OPEN);
  range
    .commit(&[("src/b/Sub.kt", Some(&sub("Base()", "open(1)")))])
    .commit(&[("src/c/This.kt", Some(&sub("Base()", "this.open(2)").replacen("package b", "package c", 1)))])
    .commit(&[("src/d/Full.kt", Some("package d\n\nclass Full : a.Base() {\n  fun use() = open(3)\n}\n"))])
    .commit(&[("src/a/Same.kt", Some("package a\n\nclass Same : Base() {\n  fun use() = open(4)\n}\n"))])
    .commit(&[(
      "src/b/JavaSub.java",
      Some("package b;\n\nimport a.Base;\n\nclass JavaSub extends Base {\n  int use() { return open(5); }\n}\n"),
    )]);
  let inherited = |from: usize| (from, 0, vec![link("open", Evidence::Inherited)]);
  assert_eq!(edges(&range.in_modules(&["src"])), [inherited(1), inherited(2), inherited(3), inherited(4), inherited(5)]);
}

/// `super.name()` means the member of the supertype too; a call of an overload the arguments select needs the commit
/// of that overload, also beside one of the baseline
#[test]
fn a_super_call_of_a_new_member_is_an_edge() {
  let mut range = Range::default();
  range
    .baseline(&[("src/a/Base.kt", &base("", OPEN))])
    .commit(&[("src/a/Base.kt", Some(&base("", &format!("{OPEN}  open fun open(name: String) = name\n"))))])
    .commit(&[("src/b/Sub.kt", Some(&sub("Base()", "super.open(\"x\")")))])
    .commit(&[("src/c/Bare.kt", Some(&sub("Base()", "open(\"y\")").replacen("package b", "package c", 1)))])
    .commit(&[("src/d/Old.kt", Some(&sub("Base()", "open(1)").replacen("package b", "package d", 1)))]);
  let inherited = |from: usize| (from, 0, vec![link("open", Evidence::Inherited)]);
  assert_eq!(
    edges(&range.graph(&[], &[], &[])),
    [inherited(1), inherited(2)],
    "the overload of the baseline takes `open(1)`"
  );
}

/// With two supertypes the member may come from either; a Java class implementing an interface besides is one too
#[test]
fn a_bare_call_with_two_supertypes_is_no_edge() {
  let mut range = adding_to_base("", OPEN);
  range
    .commit(&[(
      "src/b/Sub.kt",
      Some(&sub("Base(), Runnable", "open(1)").replacen("  fun use()", "  override fun run() {}\n\n  fun use()", 1)),
    )])
    .commit(&[(
      "src/b/JavaSub.java",
      Some("package b;\n\nimport a.Base;\n\nclass JavaSub extends Base implements Runnable {\n  int use() { return open(1); }\n\n  public void run() {}\n}\n"),
    )])
    .commit(&[("src/c/One.kt", Some(&sub("Base()", "open(1)").replacen("package b", "package c", 1)))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(3, 0, vec![link("open", Evidence::Inherited)])]);
}

/// A lambda (and an object expression, a local declaration) may have a receiver of its own; a member extension's
/// receiver comes first
#[test]
fn a_bare_call_in_a_lambda_is_no_edge() {
  let mut range = adding_to_base("", OPEN);
  range
    .commit(&[("src/b/Sub.kt", Some(&sub("Base()", "run { open(1) }")))])
    .commit(&[(
      "src/c/Extension.kt",
      Some("package c\n\nimport a.Base\n\nclass Sub : Base() {\n  fun String.use() = open(1)\n}\n"),
    )])
    .commit(&[("src/d/Plain.kt", Some(&sub("Base()", "open(1)").replacen("package b", "package d", 1)))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(3, 0, vec![link("open", Evidence::Inherited)])]);
}

/// A declaration of the name in the file (a member of the subclass, a function of the file) or an import that may bring
/// it takes the call, or took it before the member came
#[test]
fn a_bare_call_the_subclass_declares_is_no_edge() {
  let mut range = adding_to_base("", OPEN);
  range
    .commit(&[(
      "src/b/Own.kt",
      Some("package b\n\nimport a.Base\n\nclass Own : Base() {\n  fun open(name: String) = name\n\n  fun use() = open(1)\n}\n"),
    )])
    .commit(&[(
      "src/c/TopLevel.kt",
      Some("package c\n\nimport a.Base\n\nfun open(id: Int) = id\n\nclass Sub : Base() {\n  fun use() = open(1)\n}\n"),
    )])
    .commit(&[(
      "src/d/Imported.kt",
      Some("package d\n\nimport a.Base\nimport q.open\n\nclass Sub : Base() {\n  fun use() = open(1)\n}\n"),
    )])
    .commit(&[(
      "src/e/Static.java",
      Some("package e;\n\nimport a.Base;\nimport static q.Util.open;\n\nclass Static extends Base {\n  int use() { return open(1); }\n}\n"),
    )])
    .commit(&[("src/f/Plain.kt", Some(&sub("Base()", "open(1)").replacen("package b", "package f", 1)))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(5, 0, vec![link("open", Evidence::Inherited)])]);
}

/// A member that overrides one of the supertype's own supertype compiles the call without its commit (`Base :
/// Disposable` adds `override fun dispose()`), and so does a member of a Kotlin type with supertypes of a name it had
#[test]
fn a_call_of_an_override_the_supertype_adds_is_no_edge() {
  let mut range = adding_to_base(" : Disposable", "  override fun dispose() {}\n\n  fun fresh() = 2\n");
  range
    .commit(&[("src/b/Sub.kt", Some(&sub("Base()", "dispose()")))])
    .commit(&[("src/c/Fresh.kt", Some(&sub("Base()", "fresh()").replacen("package b", "package c", 1)))]);
  assert_eq!(
    edges(&range.graph(&[], &[], &[])),
    [(2, 0, vec![link("fresh", Evidence::Inherited)])],
    "a new name of a type with supertypes needs its commit"
  );
}

/// A Java method of a class with supertypes may override one without `@Override`, which then compiles the call
#[test]
fn a_java_supertype_with_supertypes_is_no_edge() {
  let java_base = |extends: &str, members: &str| format!("package a;\n\npublic class JavaBase{extends} {{\n  public void other() {{}}\n{members}}}\n");
  let use_of = |class: &str| format!("package b;\n\nimport a.{class};\n\nclass Use{class} extends {class} {{\n  void use() {{ fresh(); }}\n}}\n");
  let mut range = Range::default();
  range
    .baseline(&[
      ("src/a/JavaBase.java", &java_base(" extends Root", "")),
      ("src/a/Plain.java", &java_base("", "").replace("JavaBase", "Plain")),
    ])
    .commit(&[
      ("src/a/JavaBase.java", Some(&java_base(" extends Root", "  public void fresh() {}\n"))),
      ("src/a/Plain.java", Some(&java_base("", "  public void fresh() {}\n").replace("JavaBase", "Plain"))),
    ])
    .commit(&[("src/b/UseJavaBase.java", Some(&use_of("JavaBase")))])
    .commit(&[("src/b/UsePlain.java", Some(&use_of("Plain")))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(2, 0, vec![link("fresh", Evidence::Inherited)])]);
}

/// A private member is no member of the subclass; an `internal` one only of the same module
#[test]
fn a_private_member_of_the_supertype_is_no_edge() {
  let mut range = adding_to_base("", "  private fun hidden() = 2\n\n  internal fun inside() = 3\n");
  range
    .commit(&[("air/src/b/Sub.kt", Some(&sub("Base()", "hidden()")))])
    .commit(&[("other/src/c/Far.kt", Some(&sub("Base()", "inside()").replacen("package b", "package c", 1)))])
    .commit(&[("src/d/Near.kt", Some(&sub("Base()", "inside()").replacen("package b", "package d", 1)))]);
  let (src, air, other) = ([file("BUILD.bazel")], [file("BUILD.bazel")], [file("BUILD.bazel")]);
  assert_eq!(
    edges(&range.graph(&[("src", &src), ("air", &air), ("other", &other)], &[], &[])),
    [(3, 0, vec![link("inside", Evidence::Inherited)])]
  );
}

/// The member of the supertype's supertype is no member the class names: `Sub : Mid`, `Mid : Base`
#[test]
fn a_bare_call_through_a_grandparent_is_no_edge() {
  let mut range = adding_to_base("", OPEN);
  range
    .baseline(&[("src/a/Mid.kt", "package a\n\nopen class Mid : Base()\n")])
    .commit(&[("src/b/Sub.kt", Some("package b\n\nimport a.Mid\n\nclass Sub : Mid() {\n  fun use() = open(1)\n}\n"))])
    .commit(&[("src/c/Direct.kt", Some(&sub("Base()", "open(1)").replacen("package b", "package c", 1)))]);
  assert_eq!(edges(&range.graph(&[], &[], &[])), [(2, 0, vec![link("open", Evidence::Inherited)])]);
}
