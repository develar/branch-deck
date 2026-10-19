//! The highlighting rules of Rust (`tree-sitter-rust`), after `RsColor` of `RustRover`: what its lexer and its annotators
//! colour that a syntax tree shows without resolving a name.

use crate::highlight::paint::{Cx, Paint, Rules, comment, function};
use crate::highlight::scopes::Scopes;
use crate::highlight::{FIELD, ITALIC, Role, UNDERLINE, is_constant_name, named_child};
use tree_sitter::Node;

pub(crate) struct Rust;

const CONSTANT: u32 = Role::Field.style() | ITALIC;
/// A `mut` binding, underlined as in `RustRover`
const MUTABLE: u32 = Role::Foreground.style() | UNDERLINE;

impl Rules for Rust {
  fn node(&mut self, scopes: &mut Scopes, cx: &mut Cx<'_, '_>, node: Node<'_>) -> Option<Paint> {
    Some(match node.kind() {
      "line_comment" | "block_comment" => comment(cx, node),
      "string_literal" | "raw_string_literal" | "char_literal" => Paint::Style(Role::String.style()),
      "escape_sequence" => Paint::Leaf(Role::Escape.style()),
      "integer_literal" | "float_literal" => Paint::Leaf(Role::Number.style()),
      "boolean_literal" | "crate" | "super" | "primitive_type" | "mutable_specifier" => Paint::Leaf(Role::Keyword.style()),
      "self" => Paint::Leaf(Role::SelfValue.style()),
      "?" => Paint::Leaf(Role::QuestionOperator.style()),
      // RustRover colours a call as the function it calls: `f()`, `item.clone()`, `Vec::new()`
      "call_expression" => {
        if let Some(name) = node.child_by_field_name("function").and_then(callee_name) {
          cx.mark(name, Role::FunctionDeclaration.style());
        }
        Paint::Inherit
      }
      "lifetime" => Paint::Leaf(Role::Lifetime.style()),
      "attribute_item" | "inner_attribute_item" => Paint::Style(Role::Annotation.style()),
      "macro_invocation" => {
        if let Some(name) = node.child_by_field_name("macro") {
          cx.mark(name, Role::Macro.style());
          if let Some(bang) = name.next_sibling().filter(|it| it.kind() == "!") {
            cx.mark(bang, Role::Macro.style());
          }
        }
        Paint::Inherit
      }
      "macro_definition" => {
        if let Some(name) = node.child_by_field_name("name") {
          cx.mark(name, Role::Macro.style());
        }
        Paint::Inherit
      }
      "function_item" | "function_signature_item" | "closure_expression" => function(scopes, cx, node, node.child_by_field_name("name")),
      "block" | "for_expression" | "match_arm" | "if_expression" | "while_expression" => {
        scopes.push_local(node.id());
        Paint::Inherit
      }
      "parameter" | "let_declaration" | "closure_parameters" => {
        let pattern = node.child_by_field_name("pattern").unwrap_or(node);
        // `let mut x`, `mut x: T`: the binding is mutable
        let mutable = named_child(node, "mutable_specifier").is_some();
        if mutable && pattern.kind() == "identifier" {
          scopes.bind_styled(cx.text_of(pattern), MUTABLE);
          cx.mark(pattern, MUTABLE);
        } else {
          bind_pattern(scopes, cx, pattern);
        }
        Paint::Inherit
      }
      // a method call is plain: `items.push(item)`
      "field_identifier"
        if cx
          .grandparent()
          .is_some_and(|call| call.kind() == "call_expression" && call.child_by_field_name("function") == cx.parent()) =>
      {
        Paint::Inherit
      }
      "field_identifier" | "shorthand_field_identifier" => Paint::Leaf(FIELD),
      // a constant is italic, a static not, as in RustRover
      "const_item" | "static_item" | "enum_variant" => {
        if let Some(name) = node.child_by_field_name("name") {
          let style = match node.kind() {
            "const_item" => CONSTANT,
            "static_item" => FIELD,
            _ => Role::EnumVariant.style(),
          };
          cx.mark(name, style);
        }
        Paint::Inherit
      }
      "type_parameters" => {
        let mut cursor = node.walk();
        for parameter in node.named_children(&mut cursor) {
          let name = if parameter.kind() == "type_identifier" {
            Some(parameter)
          } else {
            parameter.child_by_field_name("name").or_else(|| parameter.named_child(0))
          };
          if let Some(name) = name.filter(|it| it.kind() == "type_identifier") {
            scopes.bind_type_parameter(cx.text_of(name));
          }
        }
        Paint::Inherit
      }
      "label" => Paint::Leaf(Role::Label.style()),
      "identifier" => name_style(scopes, cx, node).map_or(Paint::Inherit, Paint::Leaf),
      _ => return None,
    })
  }
}

/// The name a call shows: the function, the method of a field expression, the last segment of a path
fn callee_name(callee: Node<'_>) -> Option<Node<'_>> {
  match callee.kind() {
    "identifier" => Some(callee),
    "field_expression" => callee.child_by_field_name("field"),
    "scoped_identifier" => callee.child_by_field_name("name"),
    "generic_function" => callee.child_by_field_name("function").and_then(callee_name),
    _ => None,
  }
}

/// Binds the names of a pattern; a `mut` one is underlined wherever it is used
fn bind_pattern(scopes: &mut Scopes, cx: &mut Cx<'_, '_>, pattern: Node<'_>) {
  let mut stack = vec![pattern];
  while let Some(node) = stack.pop() {
    match node.kind() {
      "identifier" => scopes.bind(cx.text_of(node)),
      "mut_pattern" => {
        if let Some(name) = node.named_child(1).or_else(|| node.named_child(0)).filter(|it| it.kind() == "identifier") {
          scopes.bind_styled(cx.text_of(name), MUTABLE);
          cx.mark(name, MUTABLE);
        }
      }
      // the type of a parameter and the value of a `let` bind nothing
      "parameter" | "let_declaration" => {
        if let Some(inner) = node.child_by_field_name("pattern") {
          stack.push(inner);
        }
      }
      _ => {
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor).filter(|it| {
          !matches!(
            it.kind(),
            "type_identifier" | "scoped_type_identifier" | "generic_type" | "reference_type" | "primitive_type"
          )
        }));
      }
    }
  }
}

/// An unqualified name: a `mut` binding is underlined, a name in the style of a constant is one
fn name_style(scopes: &Scopes, cx: &Cx<'_, '_>, node: Node<'_>) -> Option<u32> {
  let name = cx.text_of(node);
  if let Some(style) = scopes.local(name) {
    return (style != 0).then_some(style);
  }
  let parent = cx.parent()?;
  if matches!(
    parent.kind(),
    "use_declaration" | "scoped_use_list" | "use_as_clause" | "use_list" | "attribute" | "macro_invocation"
  ) {
    return None;
  }
  // the last segment of `Kind::Small` in an expression or a pattern: an enum variant or an associated constant
  let segment_of_value = parent.kind() == "scoped_identifier"
    && parent.child_by_field_name("name") == Some(node)
    && name.bytes().next().is_some_and(|it| it.is_ascii_uppercase())
    && parent.child_by_field_name("path").is_some_and(|path| {
      cx.text_of(path)
        .rsplit("::")
        .next()
        .is_some_and(|it| it.bytes().next().is_some_and(|first| first.is_ascii_uppercase()))
    })
    && !cx
      .grandparent()
      .is_some_and(|it| matches!(it.kind(), "use_declaration" | "scoped_use_list" | "use_list" | "call_expression"));
  if segment_of_value || (is_constant_name(name) && parent.kind() != "scoped_identifier") {
    return Some(CONSTANT);
  }
  None
}
