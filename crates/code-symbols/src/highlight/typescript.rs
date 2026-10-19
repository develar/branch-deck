//! The highlighting rules of TypeScript, TSX and JavaScript (`tree-sitter-typescript`), after the JavaScript plugin of
//! IntelliJ: what its lexer and its annotators colour that a syntax tree shows without resolving a name.

use crate::highlight::paint::{Cx, Paint, Rules, comment, function, label, type_parameter};
use crate::highlight::scopes::Scopes;
use crate::highlight::{FIELD, ITALIC, Role, STATIC_FIELD, is_constant_name, is_type_name};
use tree_sitter::Node;

pub(crate) struct TypeScript;

impl Rules for TypeScript {
  fn node(&mut self, scopes: &mut Scopes, cx: &mut Cx<'_, '_>, node: Node<'_>) -> Option<Paint> {
    Some(match node.kind() {
      "comment" | "html_comment" | "hash_bang_line" => comment(cx, node),
      "string" | "template_string" => Paint::Style(Role::String.style()),
      "regex" => Paint::Leaf(Role::String.style()),
      "escape_sequence" => Paint::Leaf(Role::Escape.style()),
      // the `${` and `}` of a template are keywords, its code is code
      "template_substitution" => Paint::Style(Role::Foreground.style()),
      "${" | "}" if cx.parent().is_some_and(|parent| parent.kind() == "template_substitution") => Paint::Leaf(Role::Keyword.style()),
      "number" => Paint::Leaf(Role::Number.style()),
      "true" | "false" | "null" | "undefined" | "this" | "super" | "predefined_type" => Paint::Leaf(Role::Keyword.style()),
      "decorator" => Paint::Style(Role::Annotation.style()),
      "arguments" if cx.parent().is_some_and(|parent| parent.kind() == "call_expression") && cx.grandparent().is_some_and(|it| it.kind() == "decorator") => {
        Paint::Style(Role::Foreground.style())
      }
      "function_declaration" | "generator_function_declaration" | "method_definition" | "method_signature" | "abstract_method_signature" | "function_signature" => {
        function(scopes, cx, node, node.child_by_field_name("name"))
      }
      "function_expression" | "arrow_function" | "statement_block" | "for_statement" | "for_in_statement" | "catch_clause" => {
        scopes.push_local(node.id());
        if node.kind() == "arrow_function"
          && let Some(parameter) = node.child_by_field_name("parameter")
        {
          scopes.bind(cx.text_of(parameter));
        }
        Paint::Inherit
      }
      "required_parameter" | "optional_parameter" => {
        if let Some(pattern) = node.child_by_field_name("pattern") {
          bind_pattern(scopes, cx, pattern);
        }
        Paint::Inherit
      }
      "variable_declarator" => {
        if let Some(name) = node.child_by_field_name("name") {
          bind_pattern(scopes, cx, name);
        }
        Paint::Inherit
      }
      "public_field_definition" | "property_signature" => {
        if let Some(name) = node.child_by_field_name("name") {
          let mut cursor = node.walk();
          let is_static = node.children(&mut cursor).any(|it| it.kind() == "static");
          cx.mark(name, if is_static { STATIC_FIELD } else { FIELD });
        }
        Paint::Inherit
      }
      "member_expression" => {
        if let Some(property) = node.child_by_field_name("property") {
          let callee = cx
            .parent()
            .is_some_and(|parent| parent.kind() == "call_expression" && parent.child_by_field_name("function") == Some(node));
          let on_type = node
            .child_by_field_name("object")
            .is_some_and(|it| it.kind() == "identifier" && is_type_name(cx.text_of(it)));
          let name = cx.text_of(property);
          if !callee {
            cx.mark(property, if on_type || is_constant_name(name) { STATIC_FIELD } else { FIELD });
          } else if on_type {
            cx.mark(property, Role::Foreground.style() | ITALIC);
          }
        }
        Paint::Inherit
      }
      "pair" => {
        if let Some(key) = node.child_by_field_name("key").filter(|it| it.kind() == "property_identifier") {
          cx.mark(key, FIELD);
        }
        Paint::Inherit
      }
      // `{ text }` in an object reads a variable into a property, `{ start }` in a pattern binds a local
      "shorthand_property_identifier" => Paint::Leaf(FIELD),
      "jsx_opening_element" | "jsx_closing_element" | "jsx_self_closing_element" => {
        if let Some(name) = node.child_by_field_name("name") {
          cx.mark(name, Role::Tag.style());
        }
        Paint::Inherit
      }
      "jsx_attribute" => {
        if let Some(name) = node.named_child(0) {
          cx.mark(name, Role::Attribute.style());
        }
        Paint::Inherit
      }
      "type_parameter" => type_parameter(scopes, cx, node.child_by_field_name("name")),
      "labeled_statement" | "break_statement" | "continue_statement" => label(cx, node.child_by_field_name("label")),
      "statement_identifier" => Paint::Leaf(Role::Label.style()),
      _ => return None,
    })
  }
}

fn bind_pattern(scopes: &mut Scopes, cx: &Cx<'_, '_>, pattern: Node<'_>) {
  let mut stack = vec![pattern];
  while let Some(node) = stack.pop() {
    if node.kind() == "identifier" || node.kind() == "shorthand_property_identifier_pattern" {
      scopes.bind(cx.text_of(node));
    } else if node.kind() != "type_annotation" {
      let mut cursor = node.walk();
      stack.extend(node.named_children(&mut cursor));
    }
  }
}
