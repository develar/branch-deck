//! The highlighting of a Java file from its PSI, as IntelliJ colours it without resolving names
//! (`JavaHighlightingColors`): the tokens from the leaves of the tree, the roles of names from the nodes around them,
//! painted by `highlight::paint::paint_leaf`.
//!
//! - **Tokens** are the lexer's, as the parser left them: every keyword (`var`, `record`, `yield`, `sealed`,
//!   `non-sealed`, `permits`, `when` where the parser reads them as keywords), `true`, `false`, `null`; the numbers; a
//!   string, a text block and a character literal with their escapes (`\n`, an octal `\012`, a unicode `A`, the
//!   line continuation of a text block); a comment, a Javadoc (`/** … */`, `///`) with its tags.
//! - **An annotation** (`@Name`, `@a.b.Name`) in its colour, its arguments (`ANNOTATION_PARAMETER_LIST`) as code.
//! - **Declarations**: the name of a method (italic when it is static), of a field (italic when static, also in an
//!   interface), of an enum constant, of a record component, of a type parameter (and its uses as a type).
//! - **Uses**: a field after a qualifier (`this.count`, `Box.LIMIT`: static when the qualifier is a type name or the
//!   name a constant's), a static call (`Math.max`), and an unqualified name that is a field of a class around it with
//!   no local or parameter of that name in between (`highlight::scopes`): the members of an anonymous class from its
//!   `{` (`anonymous_body`), a pattern variable in the statement that ends it (`pattern_scope::is_pattern_scope`).
//!
//! One pass over the nodes in preorder (their ids), with a stack of the open nodes that colour or scope their subtree:
//! no recursion, whatever the nesting.

use super::pattern_scope::is_pattern_scope;
use crate::highlight::lines::{Builder, LineRuns};
use crate::highlight::paint::{Open, Paint, close_before, comment_paint, node_count, paint_escapes, paint_leaf};
use crate::highlight::scopes::Scopes;
use crate::highlight::{FIELD, ITALIC, Role, STATIC_FIELD, is_constant_name, is_type_name};
use java_syntax::psi;
use java_syntax::psi::modifier::Modifier;
use java_syntax::{NodeId, NodeKind, TokenKind, Tree};
use std::collections::HashMap;

/// A static method, declared or called: the colour of a declaration, italic, as IntelliJ's `STATIC_METHOD`
const STATIC_METHOD: u32 = Role::FunctionDeclaration.style() | ITALIC;

/// The highlighting of a Java text (its size within `highlight::MAX_BYTES`): any bytes parse, so it always has one
pub(crate) fn highlight(text: &str) -> LineRuns {
  let tree = crate::java::parse(text.as_bytes());
  let mut out = LineRuns::builder(text);
  Painter {
    tree: &tree,
    scopes: Scopes::default(),
    field_styles: HashMap::new(),
  }
  .paint(&mut out);
  out.finish()
}

/// What a composite the walk enters is to it
enum Entered {
  /// Nothing to remember: it binds a name or is plain
  Passed,
  /// It opened a scope, which ends with it
  Scope,
  /// Its subtree has this style
  Styled(u32),
}

struct Painter<'t> {
  tree: &'t Tree,
  scopes: Scopes,
  /// The style of each field of the classes the walk entered, by its `FIELD` node
  field_styles: HashMap<NodeId, u32>,
}

impl Painter<'_> {
  fn paint(&mut self, out: &mut Builder<'_>) {
    let tree = self.tree;
    let count = node_count(tree);
    let mut open: Vec<Open<()>> = Vec::new();
    let mut id = 0;
    while id < count {
      let node = NodeId(id);
      let around = close_before(&mut open, id, &mut self.scopes);
      if tree.is_doc_comment(node) {
        comment_lines(out, tree.range(node).start, text_of(tree, node), around);
        id = tree.subtree_end(node).0;
        continue;
      }
      if tree.is_leaf(node) {
        if let Some(class) = anonymous_body(tree, node) {
          let members = self.members(class);
          self.scopes.push_class(class.0 as usize, members);
          open.push(Open {
            end: tree.subtree_end(class).0,
            node: class,
            style: None,
            extra: (),
          });
        }
        if !tree.range(node).is_empty() {
          self.leaf(out, node, around);
        }
      } else {
        let style = match self.enter(node) {
          Entered::Passed => {
            id += 1;
            continue;
          }
          Entered::Scope => None,
          Entered::Styled(style) => Some(style),
        };
        open.push(Open {
          end: tree.subtree_end(node).0,
          node,
          style,
          extra: (),
        });
      }
      id += 1;
    }
  }

  /// Opens the scope of a composite and binds what it declares
  fn enter(&mut self, node: NodeId) -> Entered {
    let tree = self.tree;
    let Some(kind) = tree.node_kind(node) else { return Entered::Passed };
    match kind {
      NodeKind::ANNOTATION => Entered::Styled(Role::Annotation.style()),
      // the arguments of an annotation are code, the names of its elements too
      NodeKind::ANNOTATION_PARAMETER_LIST => Entered::Styled(Role::Foreground.style()),
      // the members of an anonymous class are seen from its `{`, not by its arguments (`anonymous_body`)
      kind if is_class(kind) && kind != NodeKind::ANONYMOUS_CLASS => {
        let members = self.members(node);
        self.scopes.push_class(node.0 as usize, members);
        Entered::Scope
      }
      kind if is_local_scope(kind) || is_pattern_scope(tree, node, kind) => {
        self.scopes.push_local(node.0 as usize);
        Entered::Scope
      }
      NodeKind::PARAMETER | NodeKind::LOCAL_VARIABLE | NodeKind::RESOURCE_VARIABLE | NodeKind::PATTERN_VARIABLE | NodeKind::DECONSTRUCTION_PATTERN_VARIABLE => {
        if let Some(name) = name_text(tree, node) {
          self.scopes.bind(name);
        }
        Entered::Passed
      }
      NodeKind::TYPE_PARAMETER => {
        if let Some(name) = name_text(tree, node) {
          self.scopes.bind_type_parameter(name);
        }
        Entered::Passed
      }
      _ => Entered::Passed,
    }
  }

  fn leaf(&self, out: &mut Builder<'_>, leaf: NodeId, around: u32) {
    let tree = self.tree;
    let start = tree.range(leaf).start;
    let text = text_of(tree, leaf);
    let paint = match tree.token_kind(leaf) {
      Some(TokenKind::C_STYLE_COMMENT | TokenKind::END_OF_LINE_COMMENT) => comment_paint(text),
      Some(
        TokenKind::STRING_LITERAL
        | TokenKind::TEXT_BLOCK_LITERAL
        | TokenKind::CHARACTER_LITERAL
        | TokenKind::STRING_TEMPLATE_BEGIN
        | TokenKind::STRING_TEMPLATE_MID
        | TokenKind::STRING_TEMPLATE_END
        | TokenKind::TEXT_BLOCK_TEMPLATE_BEGIN
        | TokenKind::TEXT_BLOCK_TEMPLATE_MID
        | TokenKind::TEXT_BLOCK_TEMPLATE_END,
      ) => {
        string(out, start, text, around);
        return;
      }
      Some(TokenKind::INTEGER_LITERAL | TokenKind::LONG_LITERAL | TokenKind::FLOAT_LITERAL | TokenKind::DOUBLE_LITERAL) => Paint::Leaf(Role::Number.style()),
      Some(TokenKind::TRUE_KEYWORD | TokenKind::FALSE_KEYWORD | TokenKind::NULL_KEYWORD) => Paint::Leaf(Role::Keyword.style()),
      Some(TokenKind::IDENTIFIER) => self.name_style(leaf, text).map_or(Paint::Inherit, Paint::Leaf),
      Some(token) if token.is_keyword() => Paint::Leaf(Role::Keyword.style()),
      _ => Paint::Inherit,
    };
    paint_leaf(out, start, text, paint, around);
  }

  /// The style of a name by the node it names or is a part of; `None` leaves it the style around it
  fn name_style(&self, leaf: NodeId, name: &str) -> Option<u32> {
    let tree = self.tree;
    let parent = tree.parent(leaf)?;
    match tree.node_kind(parent)? {
      NodeKind::METHOD | NodeKind::ANNOTATION_METHOD => Some(if has_modifier(tree, psi::get_modifier_list(tree, parent), Modifier::STATIC) {
        STATIC_METHOD
      } else {
        Role::FunctionDeclaration.style()
      }),
      NodeKind::FIELD => Some(self.field_styles.get(&parent).copied().unwrap_or(FIELD)),
      NodeKind::ENUM_CONSTANT => Some(STATIC_FIELD),
      NodeKind::RECORD_COMPONENT => Some(FIELD),
      NodeKind::TYPE_PARAMETER => Some(Role::TypeParameter.style()),
      NodeKind::REFERENCE_EXPRESSION => {
        let callee = is_callee(tree, parent);
        match qualifier(tree, parent) {
          // `Math.max(…)`: a call through a type name
          Some(object) if callee => {
            let on_type = tree.node_kind(object) == Some(NodeKind::REFERENCE_EXPRESSION) && qualifier(tree, object).is_none() && is_type_name(text_of(tree, object));
            on_type.then_some(STATIC_METHOD)
          }
          Some(object) => Some(if is_type_qualifier(tree, object) || is_constant_name(name) {
            STATIC_FIELD
          } else {
            FIELD
          }),
          None if callee => None,
          None => self.scopes.member(name),
        }
      }
      NodeKind::JAVA_CODE_REFERENCE => (is_type_reference(tree, parent) && self.scopes.is_type_parameter(name)).then_some(Role::TypeParameter.style()),
      _ => None,
    }
  }

  /// The fields of a class with their styles, its enum constants and its record components; keeps the style of each
  /// field for its declaration
  fn members(&mut self, class: NodeId) -> Vec<(String, u32)> {
    let tree = self.tree;
    let interface = class_keyword(tree, class) == Some(TokenKind::INTERFACE_KEYWORD);
    let mut names = Vec::new();
    // `static int a, b;`: PSI gives each name a `FIELD`, the later ones without the modifiers and the type of the first
    let mut heads = psi::field::declaration_heads(tree, class);
    for member in tree.children(class) {
      match tree.node_kind(member) {
        Some(NodeKind::FIELD) => {
          // the fields of the class in order, as `declaration_heads` pairs them
          let head = heads.next().map_or(member, |(_, head)| head);
          let head_static = has_modifier(tree, psi::field::get_modifier_list_with_head(tree, head), Modifier::STATIC);
          let style = if interface || head_static { STATIC_FIELD } else { FIELD };
          self.field_styles.insert(member, style);
          names.extend(name_text(tree, member).map(|name| (name.to_string(), style)));
        }
        Some(NodeKind::ENUM_CONSTANT) => names.extend(name_text(tree, member).map(|name| (name.to_string(), STATIC_FIELD))),
        Some(NodeKind::RECORD_HEADER) => {
          for component in psi::record_header::get_record_components(tree, member) {
            names.extend(name_text(tree, component).map(|name| (name.to_string(), FIELD)));
          }
        }
        _ => {}
      }
    }
    names
  }
}

/// Whether the qualifier of a field reads as a type (`Box` of `Box.LIMIT`): a name in the style of a type, not
/// `Outer.this` or `Outer.super`, whose fields are the instance's
fn is_type_qualifier(tree: &Tree, qualifier: NodeId) -> bool {
  !matches!(tree.node_kind(qualifier), Some(NodeKind::THIS_EXPRESSION | NodeKind::SUPER_EXPRESSION)) && is_type_name(text_of(tree, qualifier))
}

/// Paints a Javadoc: a `/** … */` comment as one, the `///` lines of a Markdown one each from its `///`, the
/// indentation between them in the style around them
fn comment_lines(out: &mut Builder<'_>, start: usize, text: &str, around: u32) {
  if !text.starts_with("///") {
    paint_leaf(out, start, text, comment_paint(text), around);
    return;
  }
  let mut offset = 0;
  for line in text.split_inclusive(['\n', '\r']) {
    let content = line.trim_end_matches(['\n', '\r']);
    let body = content.trim_start_matches([' ', '\t', '\u{c}']);
    if !body.is_empty() {
      let indent = content.len() - body.len();
      paint_leaf(out, start + offset + indent, body, comment_paint(body), around);
    }
    offset += line.len();
  }
}

/// Paints a string, a text block or a character literal: the literal in the colour of a string, its escapes in theirs
fn string(out: &mut Builder<'_>, start: usize, text: &str, around: u32) {
  paint_escapes(out, start, text, 0..text.len(), around, |bytes| {
    if bytes[0] == b'\\' && bytes.len() > 1 { escape_length(bytes) } else { 0 }
  });
}

/// The length of the escape at the start of `bytes`, a backslash and at least one more byte, as Java reads it:
/// `\uXXXX` with one `u` or more, an octal `\0` to `\377`, else the backslash and the character after it (`\n`, `\"`,
/// the line continuation of a text block)
fn escape_length(bytes: &[u8]) -> usize {
  match bytes[1] {
    b'u' => {
      let us = bytes[1..].iter().take_while(|&&it| it == b'u').count();
      let digits = bytes.get(1 + us..1 + us + 4);
      if digits.is_some_and(|it| it.iter().all(u8::is_ascii_hexdigit)) { 1 + us + 4 } else { 2 }
    }
    first @ b'0'..=b'7' => {
      let most = if first <= b'3' { 3 } else { 2 };
      1 + bytes[1..].iter().take(most).take_while(|it| matches!(it, b'0'..=b'7')).count()
    }
    _ => 2,
  }
}

/// A node that holds members: its fields, enum constants, record components and type parameters are seen in all of it
pub(super) const fn is_class(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::CLASS | NodeKind::ANONYMOUS_CLASS | NodeKind::ENUM_CONSTANT_INITIALIZER | NodeKind::IMPLICIT_CLASS
  )
}

/// The anonymous class whose body a leaf opens: its `{`, after its arguments (`new Base(name) { String name; }`), which
/// are the code around it and see none of its members
pub(super) fn anonymous_body(tree: &Tree, leaf: NodeId) -> Option<NodeId> {
  if tree.token_kind(leaf) != Some(TokenKind::LBRACE) {
    return None;
  }
  tree.parent(leaf).filter(|&it| tree.node_kind(it) == Some(NodeKind::ANONYMOUS_CLASS))
}

/// A node of code whose parameters and locals no name outside it sees: a method, a lambda, an initializer, a block, a
/// `for`, a `catch`, a `try` with resources, a switch rule
pub(super) const fn is_local_scope(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::METHOD
      | NodeKind::ANNOTATION_METHOD
      | NodeKind::LAMBDA_EXPRESSION
      | NodeKind::CLASS_INITIALIZER
      | NodeKind::CODE_BLOCK
      | NodeKind::FOR_STATEMENT
      | NodeKind::FOREACH_STATEMENT
      | NodeKind::FOREACH_PATTERN_STATEMENT
      | NodeKind::CATCH_SECTION
      | NodeKind::TRY_STATEMENT
      | NodeKind::SWITCH_LABELED_RULE
  )
}

/// The text of a node, empty when it is not UTF-8
pub(super) fn text_of(tree: &Tree, node: NodeId) -> &str {
  tree.utf8_text(node).unwrap_or_default()
}

/// The name of a declaration (its name identifier, a label's too) or of a code reference
pub(super) fn name_leaf(tree: &Tree, node: NodeId) -> Option<NodeId> {
  let name = match tree.node_kind(node)? {
    NodeKind::JAVA_CODE_REFERENCE => psi::code_reference::get_reference_name_element(tree, node),
    _ => psi::get_name_identifier(tree, node),
  };
  // a module's name is its `MODULE_REFERENCE`
  name.filter(|&it| tree.token_kind(it) == Some(TokenKind::IDENTIFIER))
}

fn name_text(tree: &Tree, node: NodeId) -> Option<&str> {
  name_leaf(tree, node).and_then(|it| tree.utf8_text(it))
}

/// The qualifier of a `REFERENCE_EXPRESSION` (`a` of `a.b`) or of a `JAVA_CODE_REFERENCE` (`a` of `a.B`)
pub(super) fn qualifier(tree: &Tree, reference: NodeId) -> Option<NodeId> {
  match tree.node_kind(reference)? {
    NodeKind::REFERENCE_EXPRESSION => psi::reference_expression::get_qualifier_expression(tree, reference),
    NodeKind::JAVA_CODE_REFERENCE => psi::code_reference::get_qualifier(tree, reference),
    _ => None,
  }
}

/// Whether a `REFERENCE_EXPRESSION` is the method a call calls (`foo` of `foo()`, `a.foo` of `a.foo()`)
pub(super) fn is_callee(tree: &Tree, reference: NodeId) -> bool {
  tree
    .parent(reference)
    .is_some_and(|call| tree.node_kind(call) == Some(NodeKind::METHOD_CALL_EXPRESSION) && psi::method_call_expression::get_method_expression(tree, call) == Some(reference))
}

/// Whether a modifier list has the modifier as a keyword
fn has_modifier(tree: &Tree, list: Option<NodeId>, modifier: Modifier) -> bool {
  list.is_some_and(|list| psi::modifier_list::has_explicit_modifier(tree, list, modifier))
}

/// The keyword a class is declared with (`class`, `interface`, also of `@interface`, `enum`, `record`)
fn class_keyword(tree: &Tree, class: NodeId) -> Option<TokenKind> {
  psi::class::class_or_interface_keyword(tree, class).and_then(|it| tree.token_kind(it))
}

/// The node a `JAVA_CODE_REFERENCE` is a part of: the parent of the outermost reference of its chain (`a.b.C`)
pub(super) fn reference_owner(tree: &Tree, reference: NodeId) -> Option<NodeId> {
  let mut current = reference;
  loop {
    let parent = tree.parent(current)?;
    if !matches!(tree.node_kind(parent), Some(NodeKind::JAVA_CODE_REFERENCE | NodeKind::IMPORT_STATIC_REFERENCE)) {
      return Some(parent);
    }
    current = parent;
  }
}

/// Whether a `JAVA_CODE_REFERENCE` names a type: not a package, an import, an annotation or a module
pub(super) fn is_type_reference(tree: &Tree, reference: NodeId) -> bool {
  !reference_owner(tree, reference).and_then(|it| tree.node_kind(it)).is_some_and(is_path_owner)
}

/// The nodes whose references are dotted names rather than types, as tree-sitter read them: the package, the imports,
/// the name of an annotation, the statements of a module, the qualifier of `Outer.this`
pub(super) const fn is_path_owner(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::PACKAGE_STATEMENT
      | NodeKind::IMPORT_STATEMENT
      | NodeKind::IMPORT_STATIC_STATEMENT
      | NodeKind::IMPORT_MODULE_STATEMENT
      | NodeKind::ANNOTATION
      | NodeKind::EXPORTS_STATEMENT
      | NodeKind::OPENS_STATEMENT
      | NodeKind::USES_STATEMENT
      | NodeKind::PROVIDES_STATEMENT
      | NodeKind::PROVIDES_WITH_LIST
      | NodeKind::THIS_EXPRESSION
      | NodeKind::SUPER_EXPRESSION
  )
}

#[cfg(test)]
#[path = "highlight_test.rs"]
mod tests;
