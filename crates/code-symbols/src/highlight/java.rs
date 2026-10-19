//! The highlighting rules of Java (`tree-sitter-java`), after `JavaHighlightingColors` of IntelliJ: what its lexer and
//! its highlighting visitor colour that a syntax tree shows without resolving a name.

use crate::highlight::paint::{Cx, Paint, Rules, comment, label, type_parameter};
use crate::highlight::scopes::Scopes;
use crate::highlight::{FIELD, ITALIC, Role, STATIC_FIELD, is_constant_name, is_type_name};
use tree_sitter::Node;

pub(crate) struct Java;

/// A static method, declared or called: the colour of a declaration, italic, as IntelliJ's `STATIC_METHOD`
const STATIC_METHOD: u32 = Role::FunctionDeclaration.style() | ITALIC;

impl Rules for Java {
  fn node(&mut self, scopes: &mut Scopes, cx: &mut Cx<'_, '_>, node: Node<'_>) -> Option<Paint> {
    Some(match node.kind() {
      "line_comment" | "block_comment" => comment(cx, node),
      "string_literal" | "character_literal" => Paint::Style(Role::String.style()),
      "escape_sequence" => Paint::Leaf(Role::Escape.style()),
      "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal" | "binary_integer_literal" | "decimal_floating_point_literal" | "hex_floating_point_literal" => {
        Paint::Leaf(Role::Number.style())
      }
      "true" | "false" | "null_literal" | "this" | "super" | "integral_type" | "floating_point_type" | "boolean_type" | "void_type" => Paint::Leaf(Role::Keyword.style()),
      "marker_annotation" | "annotation" => Paint::Style(Role::Annotation.style()),
      // the arguments of an annotation are code, the names of its elements too
      "annotation_argument_list" => Paint::Style(Role::Foreground.style()),
      "class_declaration" | "interface_declaration" | "enum_declaration" | "record_declaration" | "annotation_type_declaration" | "object_creation_expression" => {
        scopes.push_class(node.id(), members(cx, node));
        Paint::Inherit
      }
      "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" | "lambda_expression" | "static_initializer" => {
        scopes.push_local(node.id());
        if let Some(name) = node.child_by_field_name("name")
          && node.kind() != "lambda_expression"
        {
          let mut cursor = node.walk();
          let is_static = node
            .children(&mut cursor)
            .any(|it| it.kind() == "modifiers" && cx.text_of(it).split_whitespace().any(|word| word == "static"));
          cx.mark(name, if is_static { STATIC_METHOD } else { Role::FunctionDeclaration.style() });
        }
        if node.kind() == "lambda_expression"
          && let Some(parameters) = node.child_by_field_name("parameters")
          && parameters.kind() == "identifier"
        {
          scopes.bind(cx.text_of(parameters));
        }
        Paint::Inherit
      }
      "block" | "for_statement" | "enhanced_for_statement" | "catch_clause" | "try_with_resources_statement" | "switch_block_statement_group" | "switch_rule" => {
        scopes.push_local(node.id());
        if node.kind() == "enhanced_for_statement"
          && let Some(name) = node.child_by_field_name("name")
        {
          scopes.bind(cx.text_of(name));
        }
        Paint::Inherit
      }
      "formal_parameter" | "spread_parameter" | "catch_formal_parameter" | "resource" | "type_pattern" => {
        if let Some(name) = node.child_by_field_name("name").or_else(|| last_identifier(node)) {
          scopes.bind(cx.text_of(name));
        }
        Paint::Inherit
      }
      "inferred_parameters" => {
        let mut cursor = node.walk();
        for name in node.named_children(&mut cursor) {
          scopes.bind(cx.text_of(name));
        }
        Paint::Inherit
      }
      "local_variable_declaration" => {
        let mut cursor = node.walk();
        for declarator in node.children_by_field_name("declarator", &mut cursor) {
          if let Some(name) = declarator.child_by_field_name("name") {
            scopes.bind(cx.text_of(name));
          }
        }
        Paint::Inherit
      }
      "field_declaration" | "constant_declaration" => {
        let style = field_style(cx, node);
        let mut cursor = node.walk();
        for declarator in node.children_by_field_name("declarator", &mut cursor) {
          if let Some(name) = declarator.child_by_field_name("name") {
            cx.mark(name, style);
          }
        }
        Paint::Inherit
      }
      "record_component" if node.child_by_field_name("name").is_some() => {
        if let Some(name) = node.child_by_field_name("name") {
          cx.mark(name, FIELD);
        }
        Paint::Inherit
      }
      "enum_constant" => {
        if let Some(name) = node.child_by_field_name("name") {
          cx.mark(name, STATIC_FIELD);
        }
        Paint::Inherit
      }
      "field_access" => {
        if let Some(field) = node.child_by_field_name("field") {
          let object = node.child_by_field_name("object");
          let on_type = object.is_some_and(|it| is_type_name(cx.text_of(it)));
          cx.mark(field, if on_type || is_constant_name(cx.text_of(field)) { STATIC_FIELD } else { FIELD });
        }
        Paint::Inherit
      }
      "method_invocation" => {
        if let Some(name) = node.child_by_field_name("name")
          && node
            .child_by_field_name("object")
            .is_some_and(|it| it.kind() == "identifier" && is_type_name(cx.text_of(it)))
        {
          cx.mark(name, STATIC_METHOD);
        }
        Paint::Inherit
      }
      "labeled_statement" | "break_statement" | "continue_statement" => label(cx, node.named_child(0)),
      "type_parameter" => type_parameter(scopes, cx, node.named_child(0).filter(|it| it.kind() == "type_identifier")),
      "identifier" => name_style(scopes, cx, node).map_or(Paint::Inherit, Paint::Leaf),
      _ => return None,
    })
  }
}

/// An unqualified name: a field of an enclosing class with no local or parameter of that name in between
fn name_style(scopes: &Scopes, cx: &Cx<'_, '_>, node: Node<'_>) -> Option<u32> {
  let parent = cx.parent()?;
  let callee = parent.kind() == "method_invocation" && parent.child_by_field_name("name") == Some(node);
  if callee
    || matches!(
      parent.kind(),
      "scoped_identifier" | "import_declaration" | "package_declaration" | "scoped_type_identifier" | "annotation" | "marker_annotation"
    )
  {
    return None;
  }
  scopes.member(cx.text_of(node))
}

/// The style of the fields of a declaration: static ones are italic, as IntelliJ's static fields
fn field_style(cx: &Cx<'_, '_>, declaration: Node<'_>) -> u32 {
  let mut cursor = declaration.walk();
  let is_static = declaration.kind() == "constant_declaration"
    || declaration
      .children(&mut cursor)
      .any(|it| it.kind() == "modifiers" && cx.text_of(it).split_whitespace().any(|word| word == "static"))
    || cx.ancestors().nth(1).is_some_and(|it| it.kind() == "interface_declaration");
  if is_static { STATIC_FIELD } else { FIELD }
}

/// The fields of a class with their styles, its enum constants and its record components
fn members(cx: &Cx<'_, '_>, class: Node<'_>) -> Vec<(String, u32)> {
  let mut names = Vec::new();
  if let Some(parameters) = class.child_by_field_name("parameters") {
    let mut cursor = parameters.walk();
    for component in parameters.named_children(&mut cursor) {
      if let Some(name) = component.child_by_field_name("name") {
        names.push((cx.text_of(name).to_string(), FIELD));
      }
    }
  }
  let Some(body) = class.child_by_field_name("body") else { return names };
  let mut cursor = body.walk();
  for member in body.named_children(&mut cursor) {
    match member.kind() {
      "field_declaration" | "constant_declaration" => {
        let style = field_style(cx, member);
        let mut inner = member.walk();
        for declarator in member.children_by_field_name("declarator", &mut inner) {
          if let Some(name) = declarator.child_by_field_name("name") {
            names.push((cx.text_of(name).to_string(), style));
          }
        }
      }
      "enum_constant" => names.extend(member.child_by_field_name("name").map(|name| (cx.text_of(name).to_string(), STATIC_FIELD))),
      "enum_body_declarations" => {
        let mut inner = member.walk();
        for declaration in member.named_children(&mut inner).filter(|it| it.kind() == "field_declaration") {
          let style = field_style(cx, declaration);
          let mut declarators = declaration.walk();
          for declarator in declaration.children_by_field_name("declarator", &mut declarators) {
            if let Some(name) = declarator.child_by_field_name("name") {
              names.push((cx.text_of(name).to_string(), style));
            }
          }
        }
      }
      _ => {}
    }
  }
  names
}

fn last_identifier(node: Node<'_>) -> Option<Node<'_>> {
  let mut cursor = node.walk();
  node.named_children(&mut cursor).filter(|it| it.kind() == "identifier").last()
}
