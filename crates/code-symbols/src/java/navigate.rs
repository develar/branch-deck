//! Go to Declaration and Find Usages in a Java file from its PSI (spec/commit/symbol-navigation.spec.md): the name at
//! a place (`Tree::leaf_index`), its qualifier, the kind of the reference, its declaration in the file by the scopes,
//! and the usages of a name, answered by `navigate::{reference_answer, usage_answers}`.
//!
//! - **A name** is an `IDENTIFIER` leaf outside a Javadoc; a keyword (`var`, `this`) is none.
//! - **Its kind** by the node it is the name of or a part of: the name of a declaration (a class, a method, a field, a
//!   local, a parameter of any kind, a pattern variable, an enum constant, a record component, a type parameter, a
//!   label); a callee (`foo()`, `a.foo()`, with its qualifier); a member after a qualifier (`a.b`); a type (the parts
//!   of a `JAVA_CODE_REFERENCE` of a type); else a value. The dotted names of the package, the imports, an annotation
//!   and a module are a value and members after it, without a qualifier, as `scoped_identifier` read them before.
//! - **Its declaration** by the scopes around it, in one walk of the tree that keeps only the declarations of that
//!   name: a class sees its members, its record components and its type parameters wherever they are declared (the
//!   last one before the name, else the first), a method its type parameters, and a method, a lambda, a block, a `for`,
//!   a `catch`, a `try` and a switch rule the parameters and locals declared before the name, a statement the pattern
//!   variables of its condition (`highlight::is_pattern_scope`). `break` and `continue` go to their label. A member
//!   after a qualifier has none.
//! - **The usages** of a local (declared in a method, a lambda or an initializer) are the names bound to it; those of
//!   any other name are its names that no local binds, each with how it uses the name (`access_of`).

use super::highlight::{anonymous_body, is_callee, is_class, is_doc_comment, is_local_scope, is_path_owner, is_pattern_scope, name_leaf, qualifier, reference_owner, text_of};
use crate::navigate::{Access, ReferenceKind};
use crate::psi_navigate::{Binding, IsNamed, Reading, Scoping, access_by_kind, is_local_in};
use java_syntax::psi;
use java_syntax::{NodeId, NodeKind, TokenKind, Tree};

/// Java's reading of its PSI for Go to Declaration and Find Usages (`psi_navigate`)
pub(crate) const NAVIGATION: Reading<java_syntax::Java, Tree> = Reading {
  parse: |text| crate::java::parse(text.as_bytes()),
  psi: |tree| tree,
  is_name: |tree, leaf| tree.token_kind(leaf) == Some(TokenKind::IDENTIFIER) && !in_doc_comment(tree, leaf),
  name: text_of,
  declares,
  kind_in,
  qualifier_text: |tree, qualifier| text_of(tree, qualifier).to_string(),
  is_named: |tree, node, name| tree.token_kind(node) == Some(TokenKind::IDENTIFIER) && tree.text(node) == name.as_bytes(),
  hoisted,
  scopes: |tree, is_named| {
    Box::new(JavaScopes {
      tree,
      is_named,
      labels: Vec::new(),
      elements: None,
    })
  },
  is_local: |tree, declared| is_local_in(tree, declared, is_function),
  access_of,
  name_start: |tree, leaf| tree.range(leaf).start,
};

fn in_doc_comment(tree: &Tree, leaf: NodeId) -> bool {
  std::iter::successors(tree.parent(leaf), |&it| tree.parent(it)).any(|it| is_doc_comment(tree, it))
}

/// The kind of a name in the node `parent` of the kind `kind`, which declares none, and its qualifier
fn kind_in(tree: &Tree, leaf: NodeId, parent: NodeId, kind: NodeKind) -> (ReferenceKind, Option<NodeId>) {
  match kind {
    NodeKind::REFERENCE_EXPRESSION => {
      // `Base.super.run()`: the member is the one of `Base`
      let qualifier = qualifier(tree, parent).map(|it| {
        if tree.node_kind(it) == Some(NodeKind::SUPER_EXPRESSION) {
          psi::super_expression::get_qualifier(tree, it).unwrap_or(it)
        } else {
          it
        }
      });
      let kind = match (is_callee(tree, parent), qualifier.is_some()) {
        (true, true) => ReferenceKind::MemberCall,
        (true, false) => ReferenceKind::Call,
        (false, true) => ReferenceKind::Member,
        (false, false) => ReferenceKind::Value,
      };
      (kind, qualifier)
    }
    NodeKind::JAVA_CODE_REFERENCE if reference_owner(tree, parent).and_then(|it| tree.node_kind(it)).is_some_and(is_path_owner) => (
      if qualifier(tree, parent).is_some() {
        ReferenceKind::Member
      } else {
        ReferenceKind::Value
      },
      None,
    ),
    NodeKind::JAVA_CODE_REFERENCE => (ReferenceKind::Type, None),
    // the member of a static import: `max` of `import static java.lang.Math.max`
    NodeKind::IMPORT_STATIC_REFERENCE => (ReferenceKind::Member, None),
    // a module's name: `a` and `.b` of `requires a.b`; the module's own, when undotted, is its declaration
    NodeKind::MODULE_REFERENCE => {
      let dotted = tree.children(parent).filter(|&it| tree.token_kind(it) == Some(TokenKind::IDENTIFIER)).nth(1).is_some();
      let after_dot = std::iter::successors(tree.previous_sibling(leaf), |&it| tree.previous_sibling(it))
        .find(|&it| !tree.token_kind(it).is_some_and(TokenKind::is_whitespace_or_comment))
        .is_some_and(|it| tree.token_kind(it) == Some(TokenKind::DOT));
      if after_dot {
        (ReferenceKind::Member, None)
      } else if !dotted && tree.parent(parent).and_then(|it| tree.node_kind(it)) == Some(NodeKind::MODULE) {
        (ReferenceKind::Declaration, None)
      } else {
        (ReferenceKind::Value, None)
      }
    }
    _ => (ReferenceKind::Value, None),
  }
}

/// A node whose `IDENTIFIER` child is the name it declares
const fn declares(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::CLASS
      | NodeKind::METHOD
      | NodeKind::ANNOTATION_METHOD
      | NodeKind::FIELD
      | NodeKind::ENUM_CONSTANT
      | NodeKind::RECORD_COMPONENT
      | NodeKind::TYPE_PARAMETER
      | NodeKind::LOCAL_VARIABLE
      | NodeKind::PARAMETER
      | NodeKind::RESOURCE_VARIABLE
      | NodeKind::PATTERN_VARIABLE
      | NodeKind::DECONSTRUCTION_PATTERN_VARIABLE
      | NodeKind::LABELED_STATEMENT
  )
}

/// A node whose declarations no name outside it sees
const fn is_scope(kind: NodeKind) -> bool {
  is_class(kind) || is_local_scope(kind)
}

/// A node of code that runs: its declarations are locals, which no other file sees
const fn is_function(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::METHOD | NodeKind::ANNOTATION_METHOD | NodeKind::LAMBDA_EXPRESSION | NodeKind::CLASS_INITIALIZER
  )
}

/// The scopes of Java for `psi_navigate::resolve`, with the labeled statements of the name around the walk and the
/// elements of the annotation types of the file
struct JavaScopes<'t> {
  tree: &'t Tree,
  is_named: IsNamed<'t>,
  /// The labeled statements of the name around the walk: (the end of the statement, its label)
  labels: Vec<(u32, NodeId)>,
  /// The elements of the name of the annotation types of the file, once a name of an annotation's argument asks
  elements: Option<Vec<(&'t str, NodeId)>>,
}

impl Scoping<java_syntax::Java> for JavaScopes<'_> {
  fn skips(&self, node: NodeId) -> bool {
    is_doc_comment(self.tree, node)
  }

  fn binding(&mut self, leaf: NodeId) -> Binding {
    let tree = self.tree;
    if is_label_use(tree, leaf) {
      return Binding::Bound(self.labels.last().map(|it| it.1));
    }
    if tree.parent(leaf).and_then(|it| tree.node_kind(it)) == Some(NodeKind::NAME_VALUE_PAIR) {
      let elements = self.elements.get_or_insert_with(|| annotation_elements(tree, self.is_named));
      return Binding::Bound(element_of(tree, leaf, elements));
    }
    Binding::ByScopes
  }

  fn declared(&self, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    let declares = declares(kind) && kind != NodeKind::LABELED_STATEMENT && !is_hoisted(self.tree, node, kind);
    name_leaf(self.tree, node).filter(|&it| declares && (self.is_named)(it))
  }

  fn is_scope(&self, node: NodeId, kind: NodeKind) -> bool {
    // an anonymous class opens its scope at its `{` (`anonymous_body`)
    (is_scope(kind) && kind != NodeKind::ANONYMOUS_CLASS) || is_pattern_scope(self.tree, node, kind)
  }

  fn opens_at(&self, leaf: NodeId) -> Option<NodeId> {
    anonymous_body(self.tree, leaf)
  }

  fn reached(&mut self, id: u32) {
    while self.labels.last().is_some_and(|it| it.0 <= id) {
      self.labels.pop();
    }
  }

  fn entered(&mut self, node: NodeId, kind: NodeKind) {
    if kind == NodeKind::LABELED_STATEMENT
      && let Some(label) = name_leaf(self.tree, node).filter(|&it| (self.is_named)(it))
    {
      self.labels.push((self.tree.subtree_end(node).0, label));
    }
  }
}

/// The elements of the name of the annotation types (`@interface`) of the file: the name of the type and of its element
fn annotation_elements<'t>(tree: &'t Tree, is_named: &dyn Fn(NodeId) -> bool) -> Vec<(&'t str, NodeId)> {
  tree
    .descendants(tree.root())
    .filter(|&it| tree.node_kind(it) == Some(NodeKind::ANNOTATION_METHOD))
    .filter_map(|method| {
      let element = name_leaf(tree, method).filter(|&it| is_named(it))?;
      let class = tree.parent(method).filter(|&it| tree.node_kind(it) == Some(NodeKind::CLASS))?;
      Some((text_of(tree, name_leaf(tree, class)?), element))
    })
    .collect()
}

/// The element an argument of an annotation names (`value` of `@Range(value = 1)`): the one of the annotation type of
/// that name in the file
fn element_of(tree: &Tree, name: NodeId, elements: &[(&str, NodeId)]) -> Option<NodeId> {
  let annotation = tree.parent(tree.parent(tree.parent(name)?)?)?;
  let reference = psi::annotation::get_name_reference_element(tree, annotation)?;
  let type_name = text_of(tree, name_leaf(tree, reference)?);
  elements.iter().find(|it| it.0 == type_name).map(|it| it.1)
}

/// The names of the declarations a scope sees wherever they are: the members, record components and type parameters
/// of a class, the classes of the file, the type parameters of a method
fn hoisted(tree: &Tree, scope: NodeId, is_named: &dyn Fn(NodeId) -> bool) -> Vec<NodeId> {
  let mut declared = Vec::new();
  for child in tree.children(scope) {
    let Some(child_kind) = tree.node_kind(child) else { continue };
    let inner: &mut dyn Iterator<Item = NodeId> = match child_kind {
      NodeKind::TYPE_PARAMETER_LIST | NodeKind::RECORD_HEADER => &mut tree.children(child),
      _ => &mut std::iter::once(child),
    };
    for declaration in inner {
      if tree.node_kind(declaration).is_some_and(|it| is_hoisted(tree, declaration, it))
        && let Some(name) = name_leaf(tree, declaration).filter(|&it| is_named(it))
      {
        declared.push(name);
      }
    }
  }
  declared
}

/// Whether a declaration is seen in all of its scope, which collects it on entry (`hoisted`), rather than from where
/// the walk meets it
fn is_hoisted(tree: &Tree, declaration: NodeId, kind: NodeKind) -> bool {
  let parent = tree.parent(declaration).and_then(|it| tree.node_kind(it));
  match kind {
    NodeKind::TYPE_PARAMETER | NodeKind::RECORD_COMPONENT => true,
    NodeKind::FIELD | NodeKind::METHOD | NodeKind::ANNOTATION_METHOD | NodeKind::ENUM_CONSTANT | NodeKind::CLASS => {
      parent.is_some_and(|it| is_class(it) || it == NodeKind::JAVA_FILE)
    }
    _ => false,
  }
}

/// The name of a `break` or a `continue`: its label
fn is_label_use(tree: &Tree, leaf: NodeId) -> bool {
  tree
    .parent(leaf)
    .and_then(|it| tree.node_kind(it))
    .is_some_and(|it| matches!(it, NodeKind::BREAK_STATEMENT | NodeKind::CONTINUE_STATEMENT))
}

/// How a name is used: declared, imported, called, written (the end of the left side of an assignment, the operand of
/// `++` or `--`), else read
fn access_of(tree: &Tree, leaf: NodeId, kind: ReferenceKind) -> Access {
  if let Some(access) = access_by_kind(kind) {
    return access;
  }
  let range = tree.range(leaf);
  let operand = |node: NodeId| tree.children(node).find(|&it| !tree.is_leaf(it) && !is_doc_comment(tree, it));
  for node in std::iter::successors(tree.parent(leaf), |&it| tree.parent(it)) {
    match tree.node_kind(node) {
      Some(NodeKind::IMPORT_STATEMENT | NodeKind::IMPORT_STATIC_STATEMENT | NodeKind::IMPORT_MODULE_STATEMENT) => return Access::Import,
      Some(NodeKind::ASSIGNMENT_EXPRESSION) => {
        let left = operand(node).map(|it| tree.range(it));
        return if left.is_some_and(|it| it.end == range.end && it.start <= range.start) {
          Access::Write
        } else {
          Access::Read
        };
      }
      Some(NodeKind::PREFIX_EXPRESSION | NodeKind::POSTFIX_EXPRESSION) => {
        let increments = tree
          .children(node)
          .any(|it| matches!(tree.token_kind(it), Some(TokenKind::PLUSPLUS | TokenKind::MINUSMINUS)));
        if increments && operand(node).is_some_and(|it| tree.range(it).end == range.end) {
          return Access::Write;
        }
      }
      Some(kind) if is_scope(kind) => return Access::Read,
      _ => {}
    }
  }
  Access::Read
}

#[cfg(test)]
#[path = "navigate_test.rs"]
mod tests;
