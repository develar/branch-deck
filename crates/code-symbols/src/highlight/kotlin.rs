//! The highlighting rules of Kotlin (`tree-sitter-kotlin-sg`), after `KotlinHighlightingColors` of the IntelliJ Kotlin
//! plugin: what its lexer and its annotators colour that a syntax tree shows without resolving a name.

use crate::highlight::lex::Lex;
use crate::highlight::paint::{Cx, Paint, Rules, comment, type_parameter};
use crate::highlight::scopes::Scopes;
use crate::highlight::{COMMENT, FIELD, Role, STATIC_FIELD, UNDERLINE, is_constant_name, named_child};
use tree_sitter::Node;

pub(crate) struct Kotlin;

impl Rules for Kotlin {
  fn node(&mut self, scopes: &mut Scopes, cx: &mut Cx<'_, '_>, node: Node<'_>) -> Option<Paint> {
    Some(match node.kind() {
      "line_comment" | "shebang_line" => Paint::Leaf(COMMENT),
      "multiline_comment" => comment(cx, node),
      "string_literal" | "character_literal" => Paint::Style(Role::String.style()),
      "string_content" => Paint::Lexed(Role::String.style(), Lex::Escapes),
      "character_escape_seq" => Paint::Leaf(Role::Escape.style()),
      // the `$`, `${` and `}` of a template are keywords, its code is code
      "interpolation_identifier_start" | "interpolation_expression_start" | "interpolation_expression_end" | "boolean_literal" | "null_literal" => {
        Paint::Leaf(Role::Keyword.style())
      }
      "interpolated_expression" => Paint::Style(Role::Foreground.style()),
      "interpolated_identifier" => name_style(scopes, cx, node).map_or(Paint::Leaf(Role::Foreground.style()), Paint::Leaf),
      "integer_literal" | "long_literal" | "hex_literal" | "bin_literal" | "real_literal" | "unsigned_literal" => Paint::Leaf(Role::Number.style()),
      "label" => Paint::Leaf(Role::Label.style()),
      "@"
        if cx
          .parent()
          .is_some_and(|parent| matches!(parent.kind(), "jump_expression" | "this_expression" | "super_expression")) =>
      {
        Paint::Leaf(Role::Label.style())
      }
      "annotation" | "file_annotation" => Paint::Style(Role::Annotation.style()),
      "use_site_target" => Paint::Leaf(Role::Annotation.style()),
      // the arguments of an annotation are code
      "value_arguments" if cx.ancestors().nth(1).is_some_and(|it| it.kind() == "annotation") => Paint::Style(Role::Foreground.style()),
      // the properties of a file, then of each class, are visible in their whole body
      "source_file" | "class_declaration" | "object_declaration" | "companion_object" | "object_literal" => {
        let style = if matches!(node.kind(), "class_declaration" | "object_literal") {
          FIELD
        } else {
          STATIC_FIELD
        };
        scopes.push_class(node.id(), members(cx, node, style));
        Paint::Inherit
      }
      "function_declaration" | "secondary_constructor" | "anonymous_function" | "getter" | "setter" | "anonymous_initializer" => {
        scopes.push_local(node.id());
        if node.kind() == "function_declaration"
          && let Some(name) = named_child(node, "simple_identifier")
        {
          cx.mark(name, Role::FunctionDeclaration.style());
        }
        Paint::Inherit
      }
      "lambda_literal" => {
        scopes.push_local(node.id());
        if named_child(node, "lambda_parameters").is_none() {
          scopes.bind("it");
        }
        Paint::Inherit
      }
      "statements" | "for_statement" | "catch_block" | "when_expression" => {
        scopes.push_local(node.id());
        Paint::Inherit
      }
      "property_declaration" => {
        let member = !matches!(cx.parent().map(|it| it.kind()), Some("statements" | "control_structure_body"));
        let style = if !member {
          None
        } else if scopes.in_class_body() && !in_object(cx) && !is_const(cx, node) {
          Some(FIELD)
        } else {
          Some(STATIC_FIELD)
        };
        // a local `var` is underlined wherever it is used, as IntelliJ's mutable variables
        let mutable = named_child(node, "binding_pattern_kind").is_some_and(|it| cx.text_of(it) == "var");
        for name in declared_names(node) {
          if let Some(style) = style {
            cx.mark(name, style);
          } else if mutable {
            scopes.bind_styled(cx.text_of(name), UNDERLINE);
            cx.mark(name, UNDERLINE);
          } else {
            scopes.bind(cx.text_of(name));
          }
        }
        Paint::Inherit
      }
      "class_parameter" => {
        if named_child(node, "binding_pattern_kind").is_some()
          && let Some(name) = named_child(node, "simple_identifier")
        {
          cx.mark(name, FIELD);
        }
        Paint::Inherit
      }
      "parameter" | "parameter_with_optional_type" => {
        if let Some(name) = named_child(node, "simple_identifier") {
          scopes.bind(cx.text_of(name));
        }
        Paint::Inherit
      }
      "variable_declaration"
        if matches!(
          cx.parent().map(|it| it.kind()),
          Some("lambda_parameters" | "for_statement" | "multi_variable_declaration" | "when_subject")
        ) =>
      {
        if let Some(name) = named_child(node, "simple_identifier") {
          scopes.bind(cx.text_of(name));
        }
        Paint::Inherit
      }
      "value_argument" => {
        // a named argument: `name = value`
        if let Some(name) = node.named_child(0)
          && name.kind() == "simple_identifier"
          && name.next_sibling().is_some_and(|it| it.kind() == "=")
        {
          cx.mark(name, Role::NamedArgument.style());
          if let Some(equals) = name.next_sibling() {
            cx.mark(equals, Role::NamedArgument.style());
          }
        }
        Paint::Inherit
      }
      "type_parameter" => type_parameter(scopes, cx, named_child(node, "type_identifier")),
      "navigation_suffix" => {
        if let Some(name) = named_child(node, "simple_identifier") {
          let callee = cx.parent().is_some_and(|expression| is_callee(cx, expression));
          if !callee {
            cx.mark(name, if is_constant_name(cx.text_of(name)) { STATIC_FIELD } else { FIELD });
          }
        }
        Paint::Inherit
      }
      "type_identifier" if !scopes.is_type_parameter(cx.text_of(node)) => Paint::Opaque,
      "simple_identifier" => name_style(scopes, cx, node).map_or(Paint::Opaque, Paint::Leaf),
      // `break@`, `continue@`, `return@`, `this@`, `super@`: one token, a keyword and the `@` of its label
      kind if !node.is_named() && kind.len() > 1 && kind.ends_with('@') => Paint::Lexed(Role::Keyword.style(), Lex::KeywordAt),
      _ => return None,
    })
  }
}

/// An unqualified name: a property of an enclosing class with no local of that name in between is a field, the
/// backing `field` of an accessor too; the callee of a call and anything else is plain
fn name_style(scopes: &Scopes, cx: &Cx<'_, '_>, node: Node<'_>) -> Option<u32> {
  let name = cx.text_of(node);
  let parent = cx.parent();
  if parent.is_some_and(|parent| is_callee_name(parent, node) || matches!(parent.kind(), "identifier" | "import_alias" | "infix_expression")) {
    return None;
  }
  if let Some(style) = scopes.local(name) {
    return (style != 0).then_some(style);
  }
  if name == "field" && cx.ancestors().any(|it| matches!(it.kind(), "getter" | "setter")) {
    return Some(FIELD);
  }
  scopes.member(name)
}

/// The properties of a class with their styles: those of its body and the `val`/`var` parameters of its primary
/// constructor; of a file, its top-level properties. A `const val` and an enum entry are static
fn members(cx: &Cx<'_, '_>, class: Node<'_>, style: u32) -> Vec<(String, u32)> {
  let mut names = Vec::new();
  let add_properties = |names: &mut Vec<(String, u32)>, property: Node<'_>| {
    let style = if is_const(cx, property) { STATIC_FIELD } else { style };
    names.extend(declared_names(property).into_iter().map(|name| (cx.text_of(name).to_string(), style)));
  };
  let mut cursor = class.walk();
  for child in class.named_children(&mut cursor) {
    match child.kind() {
      "primary_constructor" => {
        let mut inner = child.walk();
        for parameter in child.named_children(&mut inner) {
          if parameter.kind() == "class_parameter"
            && named_child(parameter, "binding_pattern_kind").is_some()
            && let Some(name) = named_child(parameter, "simple_identifier")
          {
            names.push((cx.text_of(name).to_string(), style));
          }
        }
      }
      "class_body" | "enum_class_body" => {
        let mut inner = child.walk();
        for member in child.named_children(&mut inner) {
          match member.kind() {
            "property_declaration" => add_properties(&mut names, member),
            "enum_entry" => names.extend(named_child(member, "simple_identifier").map(|name| (cx.text_of(name).to_string(), STATIC_FIELD))),
            // the members of a companion object are visible in the class without a qualifier
            "companion_object" => names.extend(members(cx, member, STATIC_FIELD)),
            _ => {}
          }
        }
      }
      "property_declaration" => add_properties(&mut names, child),
      _ => {}
    }
  }
  names
}

/// The names a property declaration declares: one, or those of a destructuring
fn declared_names(property: Node<'_>) -> Vec<Node<'_>> {
  let mut cursor = property.walk();
  property
    .named_children(&mut cursor)
    .flat_map(|declaration| match declaration.kind() {
      "variable_declaration" => named_child(declaration, "simple_identifier").into_iter().collect::<Vec<_>>(),
      "multi_variable_declaration" => {
        let mut inner = declaration.walk();
        declaration.named_children(&mut inner).filter_map(|it| named_child(it, "simple_identifier")).collect()
      }
      _ => Vec::new(),
    })
    .collect()
}

/// Whether `expression` is the callee of a call: `a.b()`, `a.b { }`
fn is_callee(cx: &Cx<'_, '_>, expression: Node<'_>) -> bool {
  cx.grandparent()
    .is_some_and(|call| call.kind() == "call_expression" && call.named_child(0) == Some(expression))
}

/// Whether `name` is the callee of the call `parent`: `f()`
fn is_callee_name(parent: Node<'_>, name: Node<'_>) -> bool {
  parent.kind() == "call_expression" && parent.named_child(0) == Some(name)
}

/// A property of an object, a companion object or a file: no instance property
fn in_object(cx: &Cx<'_, '_>) -> bool {
  cx.ancestors()
    .find(|it| {
      matches!(
        it.kind(),
        "class_declaration" | "object_declaration" | "companion_object" | "object_literal" | "source_file"
      )
    })
    .is_some_and(|it| matches!(it.kind(), "object_declaration" | "companion_object" | "source_file"))
}

fn is_const(cx: &Cx<'_, '_>, property: Node<'_>) -> bool {
  named_child(property, "modifiers").is_some_and(|modifiers| cx.text_of(modifiers).split_whitespace().any(|it| it == "const"))
}
