//! Go to Declaration and Find Usages in a Starlark file from its PSI (openspec/specs/commit/symbol-navigation/spec.md): the name
//! at a place, the kind of the reference, its declaration in the file by the scopes, and the usages of a name,
//! answered by `navigate::{reference_answer, usage_answers}`, as Java's and C#'s (`psi_navigate`).
//!
//! - **A name** is an `IDENTIFIER` leaf. A label in a string (`":runtime"`, `"//pkg:x"`) is none: a reference here is
//!   a name the tree uses, and a label needs a package model the file does not hold.
//! - **Its kind** by the node around it: the name of a `def`, of a parameter, of a name an `=` assigns or of a name a
//!   `load` binds is a declaration; the callee of a call is a call (`native.cc_library` a member call, `native` its
//!   qualifier); a name after a dot is a member; else a value.
//! - **Its declaration** by the two scopes of Starlark: the file, which sees its `def`s, the names its top level
//!   assigns and the names its `load`s bind wherever they are, and the body of a `def`, which sees its parameters and
//!   the names it assigns. A `load`ed name is declared by its `load`, so Go to Declaration of a loaded macro leads to
//!   the line that loads it.
//! - **The usages** of a name declared in a `def` are the names bound to it, inside that `def` alone; those of any
//!   other name are its names that no local binds, each with how it uses the name (`access_of`): a name a `load`
//!   binds is an import, a name an `=` assigns is its declaration, as every assignment of Starlark declares a name.

use crate::navigate::{Access, ReferenceKind};
use crate::psi_navigate::{Binding, IsNamed, Reading, Scoping, access_by_kind, is_local_in};
use starlark_syntax::{FileKind, NodeId, NodeKind, TokenKind, Tree, psi, string};

/// Starlark's reading of its PSI for Go to Declaration and Find Usages (`psi_navigate`)
pub(crate) const NAVIGATION: Reading<starlark_syntax::Starlark, Tree> = Reading {
  // the parse is the same for every kind of Bazel file: one grammar, and the kind says only what a call declares
  parse: |text| super::parse(text.as_bytes(), FileKind::Bzl),
  psi: |tree| tree,
  is_name,
  name: name_of,
  declares,
  kind_in,
  qualifier_text: |tree, qualifier| text_of(tree, qualifier).to_string(),
  is_named: |tree, leaf, name| is_name(tree, leaf) && name_of(tree, leaf) == name,
  hoisted,
  scopes: |tree, is_named| Box::new(StarlarkScopes { tree, is_named }),
  is_local: |tree, declared| is_local_in(tree, declared, is_function),
  access_of,
  name_start,
};

/// A name of the code: an identifier, and the string a `load` binds a name by (`load(":a.bzl", "macro")`), which
/// declares `macro` in the file
fn is_name(tree: &Tree, leaf: NodeId) -> bool {
  tree.token_kind(leaf) == Some(TokenKind::IDENTIFIER) || loaded_string(tree, leaf).is_some()
}

/// The `STRING_LOAD_VALUE` a leaf is the string of
fn loaded_string(tree: &Tree, leaf: NodeId) -> Option<NodeId> {
  let literal = tree.parent(leaf).filter(|&it| tree.node_kind(it) == Some(NodeKind::STRING_LITERAL_EXPRESSION))?;
  tree.parent(literal).filter(|&it| tree.node_kind(it) == Some(NodeKind::STRING_LOAD_VALUE))
}

/// The name a leaf is: its text, the contents of its quotes for the string of a `load`
fn name_of(tree: &Tree, leaf: NodeId) -> &str {
  let text = text_of(tree, leaf);
  if loaded_string(tree, leaf).is_some() {
    &text[string::contents_range(text)]
  } else {
    text
  }
}

/// Where the name of a leaf starts: inside the quotes for the string of a `load`
fn name_start(tree: &Tree, leaf: NodeId) -> usize {
  let start = tree.range(leaf).start;
  if loaded_string(tree, leaf).is_some() {
    start + string::contents_range(text_of(tree, leaf)).start
  } else {
    start
  }
}

/// The leaf that names what a node declares: its identifier, or the string of a `load(":a.bzl", "macro")`
fn name_leaf(tree: &Tree, node: NodeId) -> Option<NodeId> {
  if tree.node_kind(node) == Some(NodeKind::STRING_LOAD_VALUE) {
    let literal = tree.children(node).find(|&it| tree.node_kind(it) == Some(NodeKind::STRING_LITERAL_EXPRESSION))?;
    return tree.children(literal).find(|&it| tree.is_leaf(it));
  }
  psi::direct_identifier(tree, node)
}

/// The text of a node
fn text_of(tree: &Tree, node: NodeId) -> &str {
  std::str::from_utf8(tree.text(node)).unwrap_or_default()
}

/// The kind of a name in the node `parent` of the kind `kind`, which declares none, and its qualifier
fn kind_in(tree: &Tree, leaf: NodeId, parent: NodeId, kind: NodeKind) -> (ReferenceKind, Option<NodeId>) {
  // the string of a `load` declares the name it binds; `declares` cannot say so, as the leaf's parent is its literal
  if loaded_string(tree, leaf).is_some() {
    return (ReferenceKind::Declaration, None);
  }
  if kind != NodeKind::REFERENCE_EXPRESSION {
    return (ReferenceKind::Value, None);
  }
  // `a.b`: the inner reference is the qualifier of the outer one, which names `b`
  let qualifier = psi::first_node_child(tree, parent);
  match (is_callee(tree, parent), qualifier) {
    (true, Some(qualifier)) => (ReferenceKind::MemberCall, Some(qualifier)),
    (true, None) => (ReferenceKind::Call, None),
    (false, Some(qualifier)) => (ReferenceKind::Member, Some(qualifier)),
    (false, None) => (ReferenceKind::Value, None),
  }
}

/// Whether a reference is what a call around it calls
fn is_callee(tree: &Tree, reference: NodeId) -> bool {
  tree
    .parent(reference)
    .is_some_and(|call| tree.node_kind(call) == Some(NodeKind::CALL_EXPRESSION) && psi::first_node_child(tree, call) == Some(reference))
}

/// A node whose `IDENTIFIER` child is the name it declares: a `def`, a parameter, a name an `=` assigns or a `for`
/// binds (`TARGET_EXPRESSION`), and a name a `load` binds under another (`load(":a.bzl", kt = "kotlin_library")`)
const fn declares(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::FUNCTION_DECLARATION
      | NodeKind::MANDATORY_PARAMETER
      | NodeKind::OPTIONAL_PARAMETER
      | NodeKind::VARIADIC_PARAMETER
      | NodeKind::KEYWORD_VARIADIC_PARAMETER
      | NodeKind::TARGET_EXPRESSION
      | NodeKind::NAMED_LOAD_VALUE
      | NodeKind::STRING_LOAD_VALUE
  )
}

/// A node of code that runs: the names it declares are locals, which no other file sees
const fn is_function(kind: NodeKind) -> bool {
  matches!(kind, NodeKind::FUNCTION_DECLARATION | NodeKind::LAMBDA_EXPRESSION)
}

/// The scopes of Starlark for `psi_navigate::resolve`: the file and the body of a `def`
struct StarlarkScopes<'t> {
  tree: &'t Tree,
  is_named: IsNamed<'t>,
}

impl Scoping<starlark_syntax::Starlark> for StarlarkScopes<'_> {
  fn skips(&self, _node: NodeId) -> bool {
    false
  }

  fn binding(&mut self, _leaf: NodeId) -> Binding {
    Binding::ByScopes
  }

  fn declared(&self, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    // the parameters of a `def` and what the file declares are hoisted; a name a body assigns is seen from there on
    let declares = kind == NodeKind::TARGET_EXPRESSION && !is_hoisted(self.tree, node, kind);
    name_leaf(self.tree, node).filter(|&it| declares && (self.is_named)(it))
  }

  fn is_scope(&self, _node: NodeId, kind: NodeKind) -> bool {
    is_function(kind)
  }
}

/// The names of the declarations a scope sees wherever they are: the `def`s of the file, the names its top level
/// assigns and the names its `load`s bind; the parameters of a `def`
fn hoisted(tree: &Tree, scope: NodeId, is_named: &dyn Fn(NodeId) -> bool) -> Vec<NodeId> {
  let mut declared = Vec::new();
  let mut push = |declaration: NodeId| {
    if let Some(name) = name_leaf(tree, declaration).filter(|&it| is_named(it)) {
      declared.push(name);
    }
  };
  for node in tree.descendants(scope) {
    if tree.node_kind(node).is_some_and(|kind| declares(kind) && is_hoisted(tree, node, kind)) && scope_of(tree, node) == scope {
      push(node);
    }
  }
  declared
}

/// Whether a declaration is seen in all of its scope, which collects it on entry (`hoisted`): a `def`, a parameter, a
/// name a `load` binds, and a name the file's top level assigns
fn is_hoisted(tree: &Tree, declaration: NodeId, kind: NodeKind) -> bool {
  match kind {
    NodeKind::FUNCTION_DECLARATION
    | NodeKind::MANDATORY_PARAMETER
    | NodeKind::OPTIONAL_PARAMETER
    | NodeKind::VARIADIC_PARAMETER
    | NodeKind::KEYWORD_VARIADIC_PARAMETER
    | NodeKind::NAMED_LOAD_VALUE
    | NodeKind::STRING_LOAD_VALUE => true,
    NodeKind::TARGET_EXPRESSION => scope_of(tree, declaration) == tree.root(),
    _ => false,
  }
}

/// The scope a declaration belongs to: the innermost `def` above it, else the file. A `def`'s own name is above that
/// `def`, so it belongs to the scope around it; a parameter is under its `def`, so it belongs to that `def`
fn scope_of(tree: &Tree, declaration: NodeId) -> NodeId {
  std::iter::successors(tree.parent(declaration), |&it| tree.parent(it))
    .find(|&it| tree.node_kind(it).is_some_and(is_function))
    .unwrap_or_else(|| tree.root())
}

/// How a name is used: imported (a name a `load` binds), declared (a `def`, a parameter, a name an `=` assigns),
/// called, else read
fn access_of(tree: &Tree, leaf: NodeId, kind: ReferenceKind) -> Access {
  let binds_load = loaded_string(tree, leaf).is_some() || tree.parent(leaf).and_then(|it| tree.node_kind(it)) == Some(NodeKind::NAMED_LOAD_VALUE);
  if binds_load {
    return Access::Import;
  }
  access_by_kind(kind).unwrap_or(Access::Read)
}

#[cfg(test)]
#[path = "navigate_test.rs"]
mod tests;
