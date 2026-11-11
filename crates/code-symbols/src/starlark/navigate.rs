//! Go to Declaration and Find Usages in a Starlark file from its PSI (openspec/specs/commit/symbol-navigation/spec.md): the name
//! at a place, the kind of the reference, its declaration in the file by the scopes, and the usages of a name,
//! answered by `psi_navigate` and `navigate`, as Java's and C#'s.
//!
//! - **A name** is an `IDENTIFIER` leaf. A label in a string (`":runtime"`, `"//pkg:x"`) is none: a reference here is
//!   a name the tree uses, and a label needs a package model the file does not hold.
//! - **Its kind** by the node around it: the name of a `def`, of a parameter, of a name an `=` assigns or of a name a
//!   `load` binds is a declaration; the callee of a call is a call (`native.cc_library` a member call, `native` its
//!   qualifier); a name after a dot is a member; else a value.
//! - **Its declaration** by the scopes of Starlark: the file, which sees its `def`s, the names its top level
//!   assigns and the names its `load`s bind wherever they are; a `def` or a `lambda`, which sees its parameters and
//!   the names its body binds; a comprehension, which sees its variables. A name a `def` or a comprehension binds is
//!   one variable in all of it, as Starlark reads it: its declaration is the first place that binds it (the
//!   parameter, else the first assignment, `for` or `def` inside), a later one writes it (`Reading::rebinds`), and a
//!   name before its first assignment is still the variable's, a closure's too. The default of a parameter is read
//!   where its function is defined and the first collection of a comprehension where it stands: neither sees its own
//!   scope (`Scoping::sees`). A `load`ed name is declared by its `load`, so Go to Declaration of a loaded macro leads
//!   to the line that loads it.
//! - **The usages** of a name declared in a `def` or by a comprehension are the names bound to it, inside that scope
//!   alone; those of any other name are its names that no local binds, each with how it uses the name (`access_of`):
//!   a name a `load` binds is an import, a name an `=` of the file assigns is its declaration, as every assignment of
//!   the top level declares a name.
//! - **What a local is** (`local_of`, openspec/specs/commit/name-bindings/spec.md): a name a body assigns a variable,
//!   the name a `for` or a comprehension binds a loop variable, the parameter of a `lambda`, and the parameter of a
//!   `def`; a call may name the parameter of either unless it is `*args` or `**kwargs`; a `def` inside a `def` is
//!   `Other`.
//! - **Its site** (`site_of`): the name of a named argument, a name a tuple or a list of targets assigns.

use crate::navigate::{Access, LocalBinding, LocalKind, ReferenceKind, Site};
use crate::psi_navigate::{Binding, Reading, Scoping, access_by_kind, is_local_in, is_within, parent_kind};
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
  scopes: |tree, _| Box::new(StarlarkScopes { tree }),
  is_local: |tree, declared| is_local_in(tree, declared, is_scope),
  local_of,
  site_of,
  rebinds,
  is_implicit: |_| false,
  access_of,
  name_start,
  // a text with an error of its parser: what stands where the parser gave up is no name of the tree
  reads_in_part: |tree| tree.errors().len() > 0,
};

/// What holds the targets of an assignment or a `for` beside the statement itself: `a, b = pair`, `[a, b] = pair`
const fn is_target_list(kind: NodeKind) -> bool {
  matches!(kind, NodeKind::TUPLE_EXPRESSION | NodeKind::LIST_LITERAL_EXPRESSION | NodeKind::PARENTHESIZED_EXPRESSION)
}

/// What a local declares, by its declaration and what it is a part of. A call may name a parameter of a `def` and of
/// a `lambda` alike (`key(item = 2)`)
fn local_of(tree: &Tree, declaration: NodeId, kind: NodeKind) -> LocalBinding {
  match kind {
    NodeKind::MANDATORY_PARAMETER | NodeKind::OPTIONAL_PARAMETER | NodeKind::VARIADIC_PARAMETER | NodeKind::KEYWORD_VARIADIC_PARAMETER => {
      let kind_of_parameter = match parent_kind(tree, declaration).and_then(|it| parent_kind(tree, it.0)) {
        Some((_, NodeKind::LAMBDA_EXPRESSION)) => LocalKind::LambdaParameter,
        Some((_, NodeKind::FUNCTION_DECLARATION)) => LocalKind::Parameter,
        _ => return LocalBinding::OTHER,
      };
      LocalBinding {
        // `*args` and `**kwargs` take what no parameter names
        named_by_calls: matches!(kind, NodeKind::MANDATORY_PARAMETER | NodeKind::OPTIONAL_PARAMETER),
        ..LocalBinding::of(kind_of_parameter)
      }
    }
    NodeKind::TARGET_EXPRESSION => {
      let holder = std::iter::successors(parent_kind(tree, declaration), |it| parent_kind(tree, it.0)).find(|it| !is_target_list(it.1));
      LocalBinding::of(match holder {
        Some((_, NodeKind::ASSIGNMENT_STATEMENT)) => LocalKind::Variable,
        // a `for` statement and the `for` of a comprehension
        Some(_) => LocalKind::LoopVariable,
        None => LocalKind::Other,
      })
    }
    _ => LocalBinding::OTHER,
  }
}

/// What the site of a name makes of it beside its binding (`Site`). Starlark has one kind of name: a call and a value
/// of a spelling are one binding
fn site_of(tree: &Tree, leaf: NodeId, _kind: ReferenceKind, _local: Option<(NodeId, LocalBinding)>) -> Site {
  match parent_kind(tree, leaf) {
    Some((_, NodeKind::NAMED_ARGUMENT_EXPRESSION)) => Site::NamedArgument,
    Some((target, NodeKind::TARGET_EXPRESSION)) if parent_kind(tree, target).is_some_and(|it| is_target_list(it.1)) => Site::Destructured,
    _ => Site::Plain,
  }
}

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
  // `cc_library(name = "a")`: the parameter of what is called, which no scope of the file binds
  if kind == NodeKind::NAMED_ARGUMENT_EXPRESSION {
    return (ReferenceKind::Member, None);
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

/// Whether the name of a declaration may name a variable its scope already has: a name a `def` or a comprehension
/// binds by an assignment, a `for` or a `def` inside, each one variable in all of its scope
fn rebinds(tree: &Tree, leaf: NodeId) -> bool {
  parent_kind(tree, leaf)
    .is_some_and(|(declaration, kind)| matches!(kind, NodeKind::TARGET_EXPRESSION | NodeKind::FUNCTION_DECLARATION) && scope_of(tree, declaration) != tree.root())
}

/// A comprehension, whose variables no name outside it sees
const fn is_comprehension(kind: NodeKind) -> bool {
  matches!(kind, NodeKind::LIST_COMP_EXPRESSION | NodeKind::DICT_COMP_EXPRESSION)
}

/// A node whose declarations no name outside it sees, locals all: a `def`, a `lambda`, a comprehension
const fn is_scope(kind: NodeKind) -> bool {
  is_function(kind) || is_comprehension(kind)
}

/// The scopes of Starlark for `psi_navigate::resolve`: the file, the body of a `def` or a `lambda`, a comprehension
struct StarlarkScopes<'t> {
  tree: &'t Tree,
}

impl Scoping<starlark_syntax::Starlark> for StarlarkScopes<'_> {
  fn skips(&self, _node: NodeId) -> bool {
    false
  }

  fn binding(&mut self, _leaf: NodeId) -> Binding {
    Binding::ByScopes
  }

  fn is_scope(&self, _node: NodeId, kind: NodeKind) -> bool {
    is_scope(kind)
  }

  /// Every declaration is seen in all of its scope, which collects it on entry (`hoisted`)
  fn declared(&self, _node: NodeId, _kind: NodeKind) -> Option<NodeId> {
    None
  }

  /// The default of a parameter is read where its function is defined, and the first collection of a comprehension
  /// where the comprehension stands: neither sees the names their scope declares, as the name of a `def` sees none of
  /// its own
  fn sees(&self, scope: NodeId, _declared: NodeId, leaf: NodeId, _kind: ReferenceKind) -> bool {
    let tree = self.tree;
    match tree.node_kind(scope) {
      Some(kind) if is_function(kind) => {
        let parameters = tree.children(scope).find(|&it| tree.node_kind(it) == Some(NodeKind::PARAMETER_LIST));
        // of the names of a parameter list, only a parameter's own is no part of a default
        let in_default = parameters.is_some_and(|it| is_within(tree, leaf, it)) && !parent_kind(tree, leaf).is_some_and(|it| declares(it.1));
        !in_default && tree.parent(leaf) != Some(scope)
      }
      Some(kind) if is_comprehension(kind) => {
        let first_in = tree.children(scope).find(|&it| tree.token_kind(it) == Some(TokenKind::IN_KEYWORD));
        let collection = first_in.and_then(|it| std::iter::successors(tree.next_sibling(it), |&it| tree.next_sibling(it)).find(|&it| !tree.is_leaf(it)));
        !collection.is_some_and(|it| is_within(tree, leaf, it))
      }
      _ => true,
    }
  }
}

/// The names of the declarations of a scope, which sees each wherever it is: the `def`s of the file, the names its top
/// level assigns and the names its `load`s bind; of a `def`, a `lambda` and a comprehension the first of the name
/// alone, the parameter before a name the body binds, as the name is one variable there
fn hoisted(tree: &Tree, scope: NodeId, is_named: &dyn Fn(NodeId) -> bool) -> Vec<NodeId> {
  let declarations = tree
    .descendants(scope)
    .filter(|&it| tree.node_kind(it).is_some_and(declares) && scope_of(tree, it) == scope);
  let names = declarations.filter_map(|it| name_leaf(tree, it).filter(|&it| is_named(it)));
  if scope == tree.root() { names.collect() } else { names.take(1).collect() }
}

/// The scope a declaration belongs to: the innermost `def`, `lambda` or comprehension above it, else the file. A
/// `def`'s own name is above that `def`, so it belongs to the scope around it; a parameter is under its `def`, so it
/// belongs to that `def`
fn scope_of(tree: &Tree, declaration: NodeId) -> NodeId {
  std::iter::successors(tree.parent(declaration), |&it| tree.parent(it))
    .find(|&it| tree.node_kind(it).is_some_and(is_scope))
    .unwrap_or_else(|| tree.root())
}

/// How a name is used: imported (a name a `load` binds), declared (a `def`, a parameter, a name an `=` assigns),
/// written (a name its scope declared before and binds again), called, else read
fn access_of(tree: &Tree, leaf: NodeId, kind: ReferenceKind) -> Access {
  let parent = tree.parent(leaf).and_then(|it| tree.node_kind(it));
  if loaded_string(tree, leaf).is_some() || parent == Some(NodeKind::NAMED_LOAD_VALUE) {
    return Access::Import;
  }
  let binds_again = kind != ReferenceKind::Declaration && parent.is_some_and(declares);
  access_by_kind(kind).unwrap_or(if binds_again { Access::Write } else { Access::Read })
}

#[cfg(test)]
#[path = "navigate_test.rs"]
mod tests;
