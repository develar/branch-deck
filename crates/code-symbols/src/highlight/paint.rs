//! The walk that paints: a preorder `TreeCursor` walk with an explicit stack, so a deeply nested file cannot overflow
//! the stack. Each node asks the rules of its grammar for a `Paint`; the text between the children of a node takes the
//! style of the innermost node with one. A parent may mark its children before the walk reaches them (the callee of a
//! call, the name of a declaration), as the summary does (`extract.rs`).

use crate::highlight::lex::{Lex, lex};
use crate::highlight::lines::Builder;
use crate::highlight::scopes::Scopes;
use crate::highlight::{COMMENT, Role};
use std::collections::HashMap;
use tree_sitter::{Node, Tree};

/// What the rules of a grammar make of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Paint {
  /// The node has the style of the node around it, and its children are walked
  Inherit,
  /// The node has the style of the node around it, and its children are not walked: a name, whose tokens may be words
  /// a grammar also has as keywords (Kotlin's `value`, `get`, `field`)
  Opaque,
  /// The node has this style, and its children may paint over it
  Style(u32),
  /// The node has this style, and its children are not walked: a comment, a literal
  Leaf(u32),
  /// A leaf in this style whose pieces a lexer finds: the escapes of a string's content, the tags of a doc comment
  Lexed(u32, Lex),
}

/// The rules of a grammar
pub(crate) trait Rules {
  /// The paint of a node by the rules of the grammar, in the scopes of the walk; `None` leaves it to the rules every
  /// grammar shares (`enter`)
  fn node(&mut self, scopes: &mut Scopes, cx: &mut Cx<'_, '_>, node: Node<'_>) -> Option<Paint>;

  /// The paint of a leaf inside a parse error: its kind tells a keyword or a literal, the rest is plain
  fn in_error(&mut self, node: Node<'_>) -> Paint {
    if is_keyword_token(node) { Paint::Leaf(Role::Keyword.style()) } else { Paint::Inherit }
  }
}

/// The paint of a node the walk enters: the mark of its parent, else the rules of the grammar, else a type parameter in
/// scope, a keyword token, or the style around it
fn enter(rules: &mut impl Rules, scopes: &mut Scopes, cx: &mut Cx<'_, '_>, node: Node<'_>, marked: Option<u32>) -> Paint {
  // a marked node with no named children is a leaf: its anonymous tokens are its text (Kotlin's soft keywords)
  if let Some(style) = marked {
    return if node.named_child_count() == 0 { Paint::Leaf(style) } else { Paint::Style(style) };
  }
  if let Some(paint) = rules.node(scopes, cx, node) {
    return paint;
  }
  if node.kind() == "type_identifier" && scopes.is_type_parameter(cx.text_of(node)) {
    Paint::Leaf(Role::TypeParameter.style())
  } else if is_keyword_token(node) {
    Paint::Leaf(Role::Keyword.style())
  } else {
    Paint::Inherit
  }
}

/// A function, a method or a closure: a scope of its own, its name a declaration
pub(crate) fn function(scopes: &mut Scopes, cx: &mut Cx<'_, '_>, node: Node<'_>, name: Option<Node<'_>>) -> Paint {
  scopes.push_local(node.id());
  if let Some(name) = name {
    cx.mark(name, Role::FunctionDeclaration.style());
  }
  Paint::Inherit
}

/// A comment; a doc comment (`/**`, Rust's `///` and `//!`) has a colour of its own and its tags
pub(crate) fn comment(cx: &Cx<'_, '_>, node: Node<'_>) -> Paint {
  let text = cx.text_of(node);
  let doc = (text.starts_with("/**") && text != "/**/") || (text.starts_with("///") && !text.starts_with("////")) || text.starts_with("//!");
  if doc { Paint::Lexed(Role::DocComment.style(), Lex::Doc) } else { Paint::Leaf(COMMENT) }
}

/// A type parameter: its name in its style, and in scope for its uses
pub(crate) fn type_parameter(scopes: &mut Scopes, cx: &mut Cx<'_, '_>, name: Option<Node<'_>>) -> Paint {
  if let Some(name) = name {
    scopes.bind_type_parameter(cx.text_of(name));
    cx.mark(name, Role::TypeParameter.style());
  }
  Paint::Inherit
}

/// A statement with a label: the label in its style
pub(crate) fn label(cx: &mut Cx<'_, '_>, label: Option<Node<'_>>) -> Paint {
  if let Some(label) = label {
    cx.mark(label, Role::Label.style());
  }
  Paint::Inherit
}

/// What the rules see of the walk: the text, the ancestors of the node and the marks on children
pub(crate) struct Cx<'tree, 'text> {
  pub(crate) text: &'text str,
  /// The ancestors of the node, the root first
  ancestors: Vec<Node<'tree>>,
  marks: HashMap<usize, u32>,
}

impl<'tree> Cx<'tree, '_> {
  pub(crate) fn parent(&self) -> Option<Node<'tree>> {
    self.ancestors.last().copied()
  }

  pub(crate) fn grandparent(&self) -> Option<Node<'tree>> {
    self.ancestors.len().checked_sub(2).map(|index| self.ancestors[index])
  }

  /// The ancestors, the nearest first
  pub(crate) fn ancestors(&self) -> impl Iterator<Item = Node<'tree>> + '_ {
    self.ancestors.iter().rev().copied()
  }

  pub(crate) fn text_of(&self, node: Node<'_>) -> &str {
    self.text.get(node.byte_range()).unwrap_or_default()
  }

  /// Gives a node the walk has not reached yet a style that the rules see as `marked`
  pub(crate) fn mark(&mut self, node: Node<'_>, style: u32) {
    self.marks.insert(node.id(), style);
  }
}

/// A keyword token: an anonymous node whose kind is a word (`fun`, `class`, `if`), not an operator
pub(crate) fn is_keyword_token(node: Node<'_>) -> bool {
  !node.is_named() && node.kind().bytes().next().is_some_and(|it| it.is_ascii_alphabetic()) && node.kind().bytes().all(|it| it.is_ascii_alphanumeric() || it == b'_')
}

/// Paints the tree into `out`: every node entered once, the text between nodes in the style around it
pub(crate) fn paint(tree: &Tree, text: &str, rules: &mut impl Rules, out: &mut Builder<'_>) {
  let mut scopes = Scopes::default();
  let mut cx = Cx {
    text,
    ancestors: Vec::new(),
    marks: HashMap::new(),
  };
  // the style of each entered node that has one, with the node; the default foreground below them
  let mut styles: Vec<(usize, u32)> = Vec::new();
  // whether each entered node pushed a style, and whether it is inside a parse error
  let mut frames: Vec<(bool, bool)> = Vec::new();
  let mut cursor = tree.walk();
  'walk: loop {
    let node = cursor.node();
    let in_error = frames.last().is_some_and(|frame| frame.1) || node.is_error();
    let marked = cx.marks.remove(&node.id());
    let paint = if in_error && !node.is_error() && node.child_count() == 0 {
      rules.in_error(node)
    } else {
      enter(rules, &mut scopes, &mut cx, node, marked)
    };
    let current = styles.last().map_or(0, |it| it.1);
    out.push(node.start_byte(), current);
    let (style, descend) = match paint {
      Paint::Inherit => (None, true),
      Paint::Opaque => (None, false),
      Paint::Style(style) => (Some(style), true),
      Paint::Leaf(style) => (Some(style), false),
      Paint::Lexed(base, kind) => {
        lex(kind, cx.text_of(node), node.start_byte(), base, &mut |end, style| out.push(end, style));
        (Some(base), false)
      }
    };
    if !descend || node.child_count() == 0 {
      out.push(node.end_byte(), style.unwrap_or(current));
      scopes.pop(node.id());
    } else if cursor.goto_first_child() {
      if let Some(style) = style {
        styles.push((node.end_byte(), style));
      }
      frames.push((style.is_some(), in_error));
      cx.ancestors.push(node);
      continue;
    }
    // the next node: a sibling, else the next sibling of the nearest ancestor that has one
    loop {
      if cursor.goto_next_sibling() {
        continue 'walk;
      }
      if !cursor.goto_parent() {
        break 'walk;
      }
      let parent = cursor.node();
      let (pushed, _) = frames.pop().unwrap_or((false, false));
      out.push(parent.end_byte(), styles.last().map_or(0, |it| it.1));
      if pushed {
        styles.pop();
      }
      cx.ancestors.pop();
      scopes.pop(parent.id());
    }
  }
}
