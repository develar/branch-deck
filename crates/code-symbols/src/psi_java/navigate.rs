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
use crate::navigate::{Access, Position, ReferenceAt, ReferenceKind, Usage, UsagesAt, byte_offset, reference_answer, usage_answers};
use java_syntax::{NodeId, NodeKind, TokenKind, Tree};

/// The name at `position` of a Java text and what its syntax says (`navigate::reference_at`)
pub(crate) fn reference_at(text: &str, position: Position) -> Option<ReferenceAt> {
  let (tree, leaf) = parse_with_name_at(text, position)?;
  let name = text_of(&tree, leaf);
  let (kind, qualifier) = kind_of(&tree, leaf);
  let local = match kind {
    ReferenceKind::Declaration | ReferenceKind::Member | ReferenceKind::MemberCall => None,
    _ => resolve(&tree, name, Some(leaf)).last().and_then(|it| it.binding),
  };
  Some(reference_answer(
    text,
    tree.range(leaf).start,
    name.to_string(),
    kind,
    qualifier.map(|it| text_of(&tree, it).to_string()),
    local.map(|it| tree.range(it).start),
  ))
}

/// The usages of the name at `position` (`navigate::usages_at`)
pub(crate) fn usages_at(text: &str, position: Position) -> Option<UsagesAt> {
  let (tree, leaf) = parse_with_name_at(text, position)?;
  let name = text_of(&tree, leaf);
  let names = resolve(&tree, name, None);
  let local = names.iter().find(|it| it.leaf == leaf).and_then(|it| it.binding).filter(|&it| is_local(&tree, it));
  let used = names.iter().filter(|it| match (local, it.binding) {
    (Some(local), binding) => binding == Some(local),
    (None, binding) => binding.is_none_or(|binding| !is_local(&tree, binding)),
  });
  let usages = usage_answers(text, name, used.map(|it| (tree.range(it.leaf).start, access_of(&tree, it.leaf, it.kind))));
  Some(UsagesAt {
    name: name.to_string(),
    local: local.is_some(),
    usages,
  })
}

/// The usages of `name` in a text that is not the one of the name (`navigate::usages_of`): its names that no local binds
pub(crate) fn usages_of(text: &str, name: &str) -> Vec<Usage> {
  // a cheap test first: most files of a review do not have the word
  if !text.contains(name) {
    return Vec::new();
  }
  let tree = crate::psi_java::parse(text.as_bytes());
  let names = resolve(&tree, name, None);
  let used = names.iter().filter(|it| it.binding.is_none_or(|binding| !is_local(&tree, binding)));
  usage_answers(text, name, used.map(|it| (tree.range(it.leaf).start, access_of(&tree, it.leaf, it.kind))))
}

/// The tree of a text and the leaf of the name at `position` in it
fn parse_with_name_at(text: &str, position: Position) -> Option<(Tree, NodeId)> {
  let offset = byte_offset(text, position)?;
  let tree = crate::psi_java::parse(text.as_bytes());
  let leaf = name_at(&tree, offset)?;
  Some((tree, leaf))
}

/// The leaf of the name at `offset`: the character at the place, else the one before it, as the end of a name is
/// still the name (`foo|`)
fn name_at(tree: &Tree, offset: usize) -> Option<NodeId> {
  let leaves = tree.leaf_index();
  let is_name = |leaf: &NodeId| tree.token_kind(*leaf) == Some(TokenKind::IDENTIFIER) && !in_doc_comment(tree, *leaf);
  leaves
    .at(offset)
    .filter(is_name)
    .or_else(|| offset.checked_sub(1).and_then(|before| leaves.at(before)).filter(is_name))
}

fn in_doc_comment(tree: &Tree, leaf: NodeId) -> bool {
  std::iter::successors(tree.parent(leaf), |&it| tree.parent(it)).any(|it| is_doc_comment(tree, it))
}

/// The kind of a name by the node around it, and its qualifier
fn kind_of(tree: &Tree, leaf: NodeId) -> (ReferenceKind, Option<NodeId>) {
  let Some(parent) = tree.parent(leaf) else { return (ReferenceKind::Value, None) };
  let Some(kind) = tree.node_kind(parent) else { return (ReferenceKind::Value, None) };
  if declares(kind) {
    return (ReferenceKind::Declaration, None);
  }
  match kind {
    NodeKind::REFERENCE_EXPRESSION => {
      // `Base.super.run()`: the member is the one of `Base`
      let qualifier = qualifier(tree, parent).map(|it| {
        if tree.node_kind(it) == Some(NodeKind::SUPER_EXPRESSION) {
          tree.first_child_of_kind(it, NodeKind::JAVA_CODE_REFERENCE).unwrap_or(it)
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

/// A name of the walk: its leaf, its kind and the name of the declaration it is bound to
struct Named {
  leaf: NodeId,
  kind: ReferenceKind,
  binding: Option<NodeId>,
}

/// A scope of the walk: the end of its subtree and the names of its declarations of the name, in the order of the text
struct Frame {
  end: u32,
  declared: Vec<NodeId>,
}

/// Every name `name` of the tree in the order of the text, with its kind and its binding; up to `until` only, when
/// given
fn resolve(tree: &Tree, name: &str, until: Option<NodeId>) -> Vec<Named> {
  let count = u32::try_from(tree.len()).unwrap_or(u32::MAX);
  let is_named = |node: NodeId| tree.token_kind(node) == Some(TokenKind::IDENTIFIER) && tree.text(node) == name.as_bytes();
  let mut names = Vec::new();
  let mut frames = vec![Frame {
    end: count,
    declared: hoisted(tree, tree.root(), &is_named),
  }];
  // the labeled statements of the name around the walk: (the end of the statement, its label)
  let mut labels: Vec<(u32, NodeId)> = Vec::new();
  // the elements of the name of the annotation types of the file, once a name of an annotation's argument asks
  let mut elements: Option<Vec<(&str, NodeId)>> = None;
  let mut id = 0;
  while id < count {
    let node = NodeId(id);
    while frames.len() > 1 && frames.last().is_some_and(|it| it.end <= id) {
      frames.pop();
    }
    while labels.last().is_some_and(|it| it.0 <= id) {
      labels.pop();
    }
    if is_doc_comment(tree, node) {
      id = tree.subtree_end(node).0;
      continue;
    }
    if tree.is_leaf(node) {
      if let Some(class) = anonymous_body(tree, node) {
        frames.push(Frame {
          end: tree.subtree_end(class).0,
          declared: hoisted(tree, class, &is_named),
        });
      }
      if is_named(node) {
        let (kind, _) = kind_of(tree, node);
        let binding = match kind {
          ReferenceKind::Declaration => Some(node),
          ReferenceKind::Member | ReferenceKind::MemberCall => None,
          _ if is_label_use(tree, node) => labels.last().map(|it| it.1),
          _ if tree.parent(node).and_then(|it| tree.node_kind(it)) == Some(NodeKind::NAME_VALUE_PAIR) => {
            let elements = elements.get_or_insert_with(|| annotation_elements(tree, &is_named));
            element_of(tree, node, elements)
          }
          _ => declaration_in(&frames, tree.range(node).start, tree),
        };
        names.push(Named { leaf: node, kind, binding });
        if until == Some(node) {
          break;
        }
      }
      id += 1;
      continue;
    }
    if let Some(kind) = tree.node_kind(node) {
      if declares(kind)
        && kind != NodeKind::LABELED_STATEMENT
        && !is_hoisted(tree, node, kind)
        && let Some(declared) = name_leaf(tree, node).filter(|&it| is_named(it))
        && let Some(frame) = frames.last_mut()
      {
        frame.declared.push(declared);
      }
      if kind == NodeKind::LABELED_STATEMENT
        && let Some(label) = name_leaf(tree, node).filter(|&it| is_named(it))
      {
        labels.push((tree.subtree_end(node).0, label));
      }
      // an anonymous class opens its scope at its `{` (`anonymous_body`)
      if (is_scope(kind) && kind != NodeKind::ANONYMOUS_CLASS) || is_pattern_scope(tree, node, kind) {
        frames.push(Frame {
          end: tree.subtree_end(node).0,
          declared: hoisted(tree, node, &is_named),
        });
      }
    }
    id += 1;
  }
  names
}

/// The elements of the name of the annotation types (`@interface`) of the file: the name of the type and of its element
fn annotation_elements<'t>(tree: &'t Tree, is_named: &impl Fn(NodeId) -> bool) -> Vec<(&'t str, NodeId)> {
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
  let reference = tree.first_child_of_kind(annotation, NodeKind::JAVA_CODE_REFERENCE)?;
  let type_name = text_of(tree, name_leaf(tree, reference)?);
  elements.iter().find(|it| it.0 == type_name).map(|it| it.1)
}

/// The declaration of the scopes whose name starts at `start`: in the innermost scope that declares one, the last one
/// before it, else the first (a member declared after its use)
fn declaration_in(frames: &[Frame], start: usize, tree: &Tree) -> Option<NodeId> {
  let frame = frames.iter().rev().find(|it| !it.declared.is_empty())?;
  frame
    .declared
    .iter()
    .rev()
    .find(|&&it| tree.range(it).start <= start)
    .or_else(|| frame.declared.first())
    .copied()
}

/// The names of the declarations a scope sees wherever they are: the members, record components and type parameters
/// of a class, the classes of the file, the type parameters of a method
fn hoisted(tree: &Tree, scope: NodeId, is_named: &impl Fn(NodeId) -> bool) -> Vec<NodeId> {
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

/// Whether the name of a declaration is a local: a method, a lambda or an initializer around what it declares
fn is_local(tree: &Tree, declared: NodeId) -> bool {
  let Some(declaration) = tree.parent(declared) else { return false };
  std::iter::successors(tree.parent(declaration), |&it| tree.parent(it)).any(|it| tree.node_kind(it).is_some_and(is_function))
}

/// How a name is used: declared, imported, called, written (the end of the left side of an assignment, the operand of
/// `++` or `--`), else read
fn access_of(tree: &Tree, leaf: NodeId, kind: ReferenceKind) -> Access {
  if kind == ReferenceKind::Declaration {
    return Access::Declaration;
  }
  if matches!(kind, ReferenceKind::Call | ReferenceKind::MemberCall) {
    return Access::Call;
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
