use crate::diff_scan::CommitDiff;
use crate::module_roots::ModuleRoots;
use crate::symbol_dependencies::{Evidence, SymbolGraph, compute_symbol_dependencies};
use crate::symbol_scan::{SymbolInput, TsAliases};
use code_symbols::extract::summarize;
use code_symbols::language::language_of;
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
