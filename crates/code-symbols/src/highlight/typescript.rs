//! The highlighting rules of TypeScript, TSX and JavaScript (`tree-sitter-typescript`), after the JavaScript plugin of
//! IntelliJ: what its lexer colours, and what its semantic visitor colours by the declaration of a name, as far as the
//! scopes of the file show it (`JSSemanticHighlightingVisitor`):
//!
//! - **A variable** the file declares and does not export is local, an exported or imported one global, a name the
//!   file does not declare (`console`, `window`) global too; a parameter has a colour of its own.
//! - **A function** the file declares is local, an exported or imported one global, as a called name it does not
//!   declare (`setTimeout`); a method is a method, also at a call (`.f(`). An imported name is a function when the file
//!   calls it, a class when it is capitalised, else a variable: the import itself does not say.
//! - **A capitalised name** the file does not declare (`Math`, `Promise`) stays plain, as a class.

use crate::highlight::paint::{Cx, Paint, Rules, comment, label, type_parameter};
use crate::highlight::scopes::Scopes;
use crate::highlight::{FIELD, ITALIC, Role, STATIC_FIELD, is_constant_name, is_type_name};
use std::collections::HashSet;
use tree_sitter::Node;

const LOCAL_VARIABLE: u32 = Role::LocalVariable.style();
const GLOBAL_VARIABLE: u32 = Role::GlobalVariable.style();
const PARAMETER: u32 = Role::Parameter.style();
const LOCAL_FUNCTION: u32 = Role::FunctionDeclaration.style();
const GLOBAL_FUNCTION: u32 = Role::GlobalFunction.style();
const METHOD: u32 = Role::Method.style();
/// A static method: `DEFAULT_STATIC_METHOD`, the colour of a function declaration in italic
const STATIC_METHOD: u32 = Role::FunctionDeclaration.style() | ITALIC;
/// A class, an interface, an enum: plain
const CLASS: u32 = Role::Foreground.style();

#[derive(Default)]
pub(crate) struct TypeScript {
  /// The names the file calls (`f(…)`), for the colour of an import
  called: HashSet<String>,
}

impl Rules for TypeScript {
  fn node(&mut self, scopes: &mut Scopes, cx: &mut Cx<'_, '_>, node: Node<'_>) -> Option<Paint> {
    Some(match node.kind() {
      "comment" | "html_comment" | "hash_bang_line" => comment(cx, node),
      "string" | "template_string" => Paint::Style(Role::String.style()),
      "regex" => Paint::Leaf(Role::Regexp.style()),
      "escape_sequence" => Paint::Leaf(Role::Escape.style()),
      // the `${` and `}` of a template are braces, its code is code (`JS.TEMPLATE_LITERAL_PLACEHOLDER_DELIMITERS`)
      "template_substitution" => Paint::Style(Role::Foreground.style()),
      "number" => Paint::Leaf(Role::Number.style()),
      "true" | "false" | "null" | "undefined" | "this" | "super" | "predefined_type" => Paint::Leaf(Role::Keyword.style()),
      "decorator" => Paint::Style(Role::Annotation.style()),
      "arguments" if cx.parent().is_some_and(|parent| parent.kind() == "call_expression") && cx.grandparent().is_some_and(|it| it.kind() == "decorator") => {
        Paint::Style(Role::Foreground.style())
      }
      "program" => {
        self.called = called_names(cx, node);
        scopes.push_local(node.id());
        self.bind_declarations(scopes, cx, node, true);
        Paint::Inherit
      }
      "statement_block" => {
        scopes.push_local(node.id());
        self.bind_declarations(scopes, cx, node, false);
        Paint::Inherit
      }
      "function_declaration" | "generator_function_declaration" | "function_signature" => {
        scopes.push_local(node.id());
        if let Some(name) = node.child_by_field_name("name") {
          cx.mark(name, if is_exported(node) { GLOBAL_FUNCTION } else { LOCAL_FUNCTION });
        }
        Paint::Inherit
      }
      "method_definition" | "method_signature" | "abstract_method_signature" => {
        scopes.push_local(node.id());
        if let Some(name) = node.child_by_field_name("name") {
          let mut cursor = node.walk();
          let is_static = node.children(&mut cursor).any(|it| it.kind() == "static");
          cx.mark(name, if is_static { STATIC_METHOD } else { METHOD });
        }
        Paint::Inherit
      }
      "function_expression" | "arrow_function" | "for_statement" | "catch_clause" => {
        scopes.push_local(node.id());
        if let Some(parameter) = node.child_by_field_name("parameter") {
          bind_pattern(scopes, cx, parameter, PARAMETER);
        }
        Paint::Inherit
      }
      "for_in_statement" => {
        scopes.push_local(node.id());
        if let Some(left) = node.child_by_field_name("left") {
          bind_pattern(scopes, cx, left, LOCAL_VARIABLE);
        }
        Paint::Inherit
      }
      "required_parameter" | "optional_parameter" => {
        if let Some(pattern) = node.child_by_field_name("pattern") {
          bind_pattern(scopes, cx, pattern, PARAMETER);
        }
        Paint::Inherit
      }
      "variable_declarator" => {
        if let Some(name) = node.child_by_field_name("name") {
          let exported = cx.grandparent().is_some_and(|it| it.kind() == "export_statement");
          bind_pattern(scopes, cx, name, if exported { GLOBAL_VARIABLE } else { LOCAL_VARIABLE });
        }
        Paint::Inherit
      }
      // a name by its declaration; a name in a decorator is the decorator's
      "identifier" | "shorthand_property_identifier" | "shorthand_property_identifier_pattern" => {
        if cx.ancestors().any(|it| it.kind() == "decorator") {
          return Some(Paint::Inherit);
        }
        Paint::Leaf(name_style(scopes, cx, node))
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
          if is_callee(cx.parent(), node) {
            cx.mark(property, METHOD);
          } else {
            let on_type = node
              .child_by_field_name("object")
              .is_some_and(|it| it.kind() == "identifier" && is_type_name(cx.text_of(it)));
            cx.mark(property, if on_type || is_constant_name(cx.text_of(property)) { STATIC_FIELD } else { FIELD });
          }
        }
        Paint::Inherit
      }
      // a key whose value is a function is a method
      "pair" => {
        if let Some(key) = node.child_by_field_name("key").filter(|it| it.kind() == "property_identifier") {
          let function = node
            .child_by_field_name("value")
            .is_some_and(|it| matches!(it.kind(), "arrow_function" | "function_expression"));
          cx.mark(key, if function { METHOD } else { FIELD });
        }
        Paint::Inherit
      }
      // the members of an enum are static (`TS.ENUM_MEMBER`)
      "enum_body" | "enum_assignment" => {
        let mut cursor = node.walk();
        let names: Vec<Node<'_>> = node.named_children(&mut cursor).filter(|it| it.kind() == "property_identifier").collect();
        for name in names {
          cx.mark(name, STATIC_FIELD);
        }
        Paint::Inherit
      }
      "jsx_opening_element" | "jsx_closing_element" | "jsx_self_closing_element" => {
        if let Some(name) = node.child_by_field_name("name") {
          mark_tag(cx, name);
        }
        Paint::Inherit
      }
      // the brackets of an element: `<`, `>`, `</`, `/>` (`XML_TAG`)
      "<" | ">" | "/" | "</" | "/>"
        if cx
          .parent()
          .is_some_and(|parent| matches!(parent.kind(), "jsx_opening_element" | "jsx_closing_element" | "jsx_self_closing_element")) =>
      {
        Paint::Leaf(Role::TagBracket.style())
      }
      "jsx_attribute" => {
        if let Some(name) = node.named_child(0) {
          cx.mark(name, Role::Attribute.style());
        }
        Paint::Inherit
      }
      "type_parameter" => type_parameter(scopes, cx, node.child_by_field_name("name")),
      "labeled_statement" | "break_statement" | "continue_statement" => label(cx, node.child_by_field_name("label"), Role::DefaultLabel.style()),
      "statement_identifier" => Paint::Leaf(Role::DefaultLabel.style()),
      _ => return None,
    })
  }
}

impl TypeScript {
  /// The declarations of a file or a block, visible in all of it: its functions and classes, and of a file its variables
  /// and imports too, which a function declared above them uses
  fn bind_declarations(&self, scopes: &mut Scopes, cx: &Cx<'_, '_>, scope: Node<'_>, file: bool) {
    let mut cursor = scope.walk();
    for statement in scope.named_children(&mut cursor) {
      let (declaration, exported) = if statement.kind() == "export_statement" {
        (statement.child_by_field_name("declaration"), true)
      } else {
        (Some(statement), false)
      };
      let Some(declaration) = declaration else { continue };
      match declaration.kind() {
        "function_declaration" | "generator_function_declaration" => {
          if let Some(name) = declaration.child_by_field_name("name") {
            scopes.bind_styled(cx.text_of(name), if exported { GLOBAL_FUNCTION } else { LOCAL_FUNCTION });
          }
        }
        "class_declaration" | "abstract_class_declaration" | "enum_declaration" => {
          if let Some(name) = declaration.child_by_field_name("name") {
            scopes.bind_styled(cx.text_of(name), CLASS);
          }
        }
        "lexical_declaration" | "variable_declaration" if file => {
          let mut declarators = declaration.walk();
          for declarator in declaration.named_children(&mut declarators).filter(|it| it.kind() == "variable_declarator") {
            if let Some(name) = declarator.child_by_field_name("name") {
              bind_pattern(scopes, cx, name, if exported { GLOBAL_VARIABLE } else { LOCAL_VARIABLE });
            }
          }
        }
        "import_statement" if file => self.bind_imports(scopes, cx, declaration),
        _ => {}
      }
    }
  }

  /// The names an import binds: a function when the file calls it, a class when it is capitalised, else a variable
  fn bind_imports(&self, scopes: &mut Scopes, cx: &Cx<'_, '_>, import: Node<'_>) {
    let mut stack = vec![import];
    while let Some(node) = stack.pop() {
      let bound = match node.kind() {
        "import_specifier" => node.child_by_field_name("alias").or_else(|| node.child_by_field_name("name")),
        "identifier" => Some(node),
        "string" => None,
        _ => {
          let mut cursor = node.walk();
          stack.extend(node.named_children(&mut cursor));
          None
        }
      };
      if let Some(name) = bound.map(|it| cx.text_of(it)) {
        let style = if self.called.contains(name) {
          GLOBAL_FUNCTION
        } else if is_type_name(name) {
          CLASS
        } else {
          GLOBAL_VARIABLE
        };
        scopes.bind_styled(name, style);
      }
    }
  }
}

/// The colour of a name by its declaration in the scopes; a name the file does not declare is global, a capitalised
/// one a class
fn name_style(scopes: &Scopes, cx: &Cx<'_, '_>, node: Node<'_>) -> u32 {
  let name = cx.text_of(node);
  if let Some(style) = scopes.local(name) {
    return style;
  }
  if is_type_name(name) {
    CLASS
  } else if is_callee(cx.parent(), node) {
    GLOBAL_FUNCTION
  } else {
    GLOBAL_VARIABLE
  }
}

/// Whether `node` is what `parent`, a call, calls
fn is_callee(parent: Option<Node<'_>>, node: Node<'_>) -> bool {
  parent.is_some_and(|parent| parent.kind() == "call_expression" && parent.child_by_field_name("function") == Some(node))
}

/// The names the file calls: a function of the file, an import or a global
fn called_names(cx: &Cx<'_, '_>, root: Node<'_>) -> HashSet<String> {
  let mut called = HashSet::new();
  let mut cursor = root.walk();
  let mut descend = true;
  loop {
    let node = cursor.node();
    if descend
      && node.kind() == "call_expression"
      && let Some(function) = node.child_by_field_name("function").filter(|it| it.kind() == "identifier")
    {
      called.insert(cx.text_of(function).to_string());
    }
    if descend && cursor.goto_first_child() {
      continue;
    }
    if cursor.goto_next_sibling() {
      descend = true;
      continue;
    }
    if !cursor.goto_parent() {
      break;
    }
    descend = false;
  }
  called
}

/// Whether a declaration is exported: `export function f`, `export default function f`
fn is_exported(declaration: Node<'_>) -> bool {
  declaration.parent().is_some_and(|it| it.kind() == "export_statement")
}

/// The name of a JSX element: an HTML tag (`div`), else a component (`Button`, `Menu.Item`, `XML_CUSTOM_TAG_NAME`)
fn mark_tag(cx: &mut Cx<'_, '_>, name: Node<'_>) {
  match name.kind() {
    "identifier" if !cx.text_of(name).starts_with(|it: char| it.is_ascii_uppercase()) => cx.mark(name, Role::Tag.style()),
    "member_expression" | "nested_identifier" => {
      let mut cursor = name.walk();
      let parts: Vec<Node<'_>> = name.named_children(&mut cursor).collect();
      for part in parts {
        if matches!(part.kind(), "member_expression" | "nested_identifier") {
          mark_tag(cx, part);
        } else {
          cx.mark(part, Role::ComponentTag.style());
        }
      }
    }
    _ => cx.mark(name, Role::ComponentTag.style()),
  }
}

/// Binds the names of a pattern in `style`: a variable, a parameter, the names of a destructuring
fn bind_pattern(scopes: &mut Scopes, cx: &Cx<'_, '_>, pattern: Node<'_>, style: u32) {
  let mut stack = vec![pattern];
  while let Some(node) = stack.pop() {
    match node.kind() {
      "identifier" | "shorthand_property_identifier_pattern" => scopes.bind_styled(cx.text_of(node), style),
      "type_annotation" => {}
      // the key of `{ key: name }` names a property, its value binds; a default value is code
      "pair_pattern" => stack.extend(node.child_by_field_name("value")),
      "assignment_pattern" | "object_assignment_pattern" => stack.extend(node.child_by_field_name("left")),
      _ => {
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
      }
    }
  }
}
