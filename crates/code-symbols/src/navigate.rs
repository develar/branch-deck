//! The name at a place of a file and what its syntax tells of it, for going to its declaration
//! (spec/commit/symbol-navigation.spec.md): the name, its qualifier (`Foo` of `Foo.bar`), whether it is called, a type
//! or a member, and its declaration in the same file when the scopes show one (a parameter, a local, a member of an
//! enclosing class, a declaration of the file). The rest is the caller's: the other files of a review, an index.
//!
//! The usages of a name for Find Usages are its names in the tree: those bound to the same declaration for a local
//! (a parameter, a local of a function), else every name of that text that no local binds, each with how it uses it.

use crate::highlight::Grammar;
use tree_sitter::Node;

/// A place in a text: a line from 0 and a UTF-16 column, as the view counts them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
  pub line: u32,
  pub column: u32,
}

/// What the syntax says the name is
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
  /// A value: a variable, a property, a constant
  Value,
  /// The callee of a call, unqualified
  Call,
  /// A member after a qualifier, `foo.bar`
  Member,
  /// A called member, `foo.bar()`
  MemberCall,
  /// A type
  Type,
  /// The name of a declaration itself
  Declaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceAt {
  pub name: String,
  /// The text before the dot or the `::`: `Foo` of `Foo.bar`, `a.b` of `a.b.c`
  pub qualifier: Option<String>,
  pub kind: ReferenceKind,
  /// Where the name starts and ends on its line
  pub start: Position,
  pub end_column: u32,
  /// Its declaration in this file, by the scopes around it: where the declared name starts
  pub local: Option<Position>,
}

/// The kinds of the names of each grammar
const NAMES: &[&str] = &[
  "simple_identifier",
  "type_identifier",
  "interpolated_identifier",
  "identifier",
  "field_identifier",
  "shorthand_field_identifier",
  "property_identifier",
  "shorthand_property_identifier",
  "shorthand_property_identifier_pattern",
];

/// The nodes that open a scope: a name declared inside is not visible outside
const SCOPES: &[&str] = &[
  // Kotlin
  "function_declaration",
  "lambda_literal",
  "anonymous_function",
  "class_declaration",
  "object_declaration",
  "companion_object",
  "object_literal",
  "secondary_constructor",
  "getter",
  "setter",
  "for_statement",
  "catch_block",
  "statements",
  // Java
  "interface_declaration",
  "enum_declaration",
  "record_declaration",
  "method_declaration",
  "constructor_declaration",
  "lambda_expression",
  "block",
  "enhanced_for_statement",
  "catch_clause",
  // Rust
  "function_item",
  "closure_expression",
  "impl_item",
  "trait_item",
  "mod_item",
  "match_arm",
  "for_expression",
  // TypeScript
  "method_definition",
  "arrow_function",
  "function_expression",
  "generator_function_declaration",
  "class",
  "statement_block",
  "for_in_statement",
];

/// How a usage uses its name, as IntelliJ labels it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
  Declaration,
  Import,
  Call,
  Write,
  Read,
}

impl Access {
  pub const fn name(self) -> &'static str {
    match self {
      Self::Declaration => "declaration",
      Self::Import => "import",
      Self::Call => "call",
      Self::Write => "write",
      Self::Read => "read",
    }
  }
}

/// A name in a text, for Find Usages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
  pub start: Position,
  pub end_column: u32,
  pub access: Access,
}

/// The usages of the name at a place
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsagesAt {
  pub name: String,
  /// The name is a local: a parameter or a local of a function, which no other file sees
  pub local: bool,
  /// In the order of the text
  pub usages: Vec<Usage>,
}

/// The node of the name at `offset`: the character at the place, else the one before it, as the end of a name is still
/// the name (`foo|`)
fn name_at<'tree>(root: Node<'tree>, text: &str, offset: usize) -> Option<Node<'tree>> {
  let at = |start: usize, end: usize| root.named_descendant_for_byte_range(start, end).filter(|it| NAMES.contains(&it.kind()));
  at(offset, (offset + 1).min(text.len())).or_else(|| offset.checked_sub(1).and_then(|before| at(before, offset)))
}

/// The declaration a name is bound to by the scopes: the name itself when it is the name of a declaration; none for a
/// member after a qualifier, which the scopes do not bind
fn binding_of<'tree>(node: Node<'tree>, kind: ReferenceKind, name: &str, text: &str) -> Option<Node<'tree>> {
  match kind {
    ReferenceKind::Declaration => Some(node),
    ReferenceKind::Value | ReferenceKind::Call | ReferenceKind::Type => local_declaration(node, name, text),
    ReferenceKind::Member | ReferenceKind::MemberCall => None,
  }
}

/// The node of the name at `position` of a parse of `text`, its text and what the syntax says it is, for `answer`
fn with_name_at<T>(grammar: Grammar, text: &str, position: Position, answer: impl FnOnce(Node<'_>, String, ReferenceKind, Option<String>) -> T) -> Option<T> {
  let offset = byte_offset(text, position)?;
  let tree = crate::highlight::parse_text(grammar, text)?;
  let node = name_at(tree.root_node(), text, offset)?;
  let name = text.get(node.byte_range())?.to_string();
  let (kind, qualifier) = kind_of(node, text);
  Some(answer(node, name, kind, qualifier))
}

/// The name at `position` of `text` and what its syntax says; `None` where no name is
pub fn reference_at(grammar: Grammar, text: &str, position: Position) -> Option<ReferenceAt> {
  with_name_at(grammar, text, position, |node, name, kind, qualifier| reference_of(node, name, kind, qualifier, text))
}

fn reference_of(node: Node<'_>, name: String, kind: ReferenceKind, qualifier: Option<String>, text: &str) -> ReferenceAt {
  let local = match kind {
    ReferenceKind::Declaration => None,
    _ => binding_of(node, kind, &name, text).map(|it| position_of(text, it.start_byte())),
  };
  let start = position_of(text, node.start_byte());
  let end_column = start.column + utf16_length(&name);
  ReferenceAt {
    name,
    qualifier,
    kind,
    start,
    end_column,
    local,
  }
}

/// The usages of the name at `position`: those of its declaration for a local, else those of every name of that text that
/// no local binds; `None` where no name is
pub fn usages_at(grammar: Grammar, text: &str, position: Position) -> Option<UsagesAt> {
  with_name_at(grammar, text, position, |node, name, kind, _| usages_of_name(node, name, kind, text))
}

fn usages_of_name(node: Node<'_>, name: String, kind: ReferenceKind, text: &str) -> UsagesAt {
  let mut root = node;
  while let Some(parent) = root.parent() {
    root = parent;
  }
  let local = binding_of(node, kind, &name, text).filter(|it| is_local(*it));
  let usages = names_in(root, text, &name, |binding| match (local, binding) {
    (Some(local), Some(binding)) => binding.id() == local.id(),
    (Some(_), None) => false,
    (None, binding) => binding.is_none_or(|it| !is_local(it)),
  });
  UsagesAt {
    name,
    local: local.is_some(),
    usages,
  }
}

/// The usages of `name` in a text that is not the one of the name: every name of that text that no local binds
pub fn usages_of(grammar: Grammar, text: &str, name: &str) -> Vec<Usage> {
  // a cheap test first: most files of a review do not have the word
  if !text.contains(name) {
    return Vec::new();
  }
  crate::highlight::parse_text(grammar, text).map_or_else(Vec::new, |tree| names_in(tree.root_node(), text, name, |binding| binding.is_none_or(|it| !is_local(it))))
}

/// The names `name` of the tree whose binding `keep` takes, with how each uses it
fn names_in<'tree>(root: Node<'tree>, text: &str, name: &str, keep: impl Fn(Option<Node<'tree>>) -> bool) -> Vec<Usage> {
  let mut usages = Vec::new();
  let mut cursor = root.walk();
  let mut descend = true;
  loop {
    let node = cursor.node();
    if descend && NAMES.contains(&node.kind()) && text.get(node.byte_range()) == Some(name) {
      let (kind, _) = kind_of(node, text);
      if keep(binding_of(node, kind, name, text)) {
        let start = position_of(text, node.start_byte());
        usages.push(Usage {
          start,
          end_column: start.column + utf16_length(name),
          access: access_of(node, kind),
        });
      }
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
  usages
}

/// The nodes of a function, whose names no other function sees
const FUNCTIONS: &[&str] = &[
  "function_declaration",
  "anonymous_function",
  "lambda_literal",
  "secondary_constructor",
  "getter",
  "setter",
  "method_declaration",
  "constructor_declaration",
  "lambda_expression",
  "function_item",
  "closure_expression",
  "method_definition",
  "arrow_function",
  "function_expression",
  "generator_function_declaration",
];

/// Whether the name of a declaration is a local: a function around the declaration, not the function it names
fn is_local(declared: Node<'_>) -> bool {
  let mut current = declared.parent().and_then(|it| it.parent());
  while let Some(node) = current {
    if FUNCTIONS.contains(&node.kind()) {
      return true;
    }
    current = node.parent();
  }
  false
}

/// How a name is used: declared, imported, called, written (the end of the left side of an assignment), else read
fn access_of(node: Node<'_>, kind: ReferenceKind) -> Access {
  match kind {
    ReferenceKind::Declaration => return Access::Declaration,
    ReferenceKind::Call | ReferenceKind::MemberCall => return Access::Call,
    _ => {}
  }
  let mut current = node.parent();
  while let Some(parent) = current {
    match parent.kind() {
      "import_header" | "import_declaration" | "use_declaration" | "import_statement" => return Access::Import,
      "assignment" | "assignment_expression" | "augmented_assignment_expression" | "compound_assignment_expr" => {
        // Kotlin's left side is its first child, the others name it `left`
        let left = parent.child_by_field_name("left").or_else(|| parent.named_child(0));
        return if left.is_some_and(|it| it.end_byte() == node.end_byte() && it.start_byte() <= node.start_byte()) {
          Access::Write
        } else {
          Access::Read
        };
      }
      "update_expression" | "postfix_expression" | "prefix_expression" if parent.end_byte() >= node.end_byte() => {
        let operator = parent
          .child(0)
          .filter(|it| !it.is_named())
          .or_else(|| parent.child(parent.child_count().saturating_sub(1)).filter(|it| !it.is_named()));
        if operator.is_some_and(|it| matches!(it.kind(), "++" | "--")) && parent.named_child(0).is_some_and(|it| it.end_byte() == node.end_byte()) {
          return Access::Write;
        }
      }
      kind if SCOPES.contains(&kind) => return Access::Read,
      _ => {}
    }
    current = parent.parent();
  }
  Access::Read
}

/// The kind of a name by its parent, and the qualifier before it
fn kind_of(node: Node<'_>, text: &str) -> (ReferenceKind, Option<String>) {
  let Some(parent) = node.parent() else { return (ReferenceKind::Value, None) };
  let text_of = |it: Node<'_>| text.get(it.byte_range()).unwrap_or_default().to_string();
  let is_callee = |expression: Node<'_>| {
    expression.parent().is_some_and(|call| {
      // Kotlin's call has no fields: its callee is its first child; the receiver of a Java call is not its callee
      call.kind() == "call_expression" && (call.child_by_field_name("function") == Some(expression) || call.named_child(0) == Some(expression))
    })
  };
  if declared_name(parent).is_some_and(|it| it == node) || is_pattern_name(node) {
    return (ReferenceKind::Declaration, None);
  }
  match parent.kind() {
    // Kotlin `a.b`
    "navigation_suffix" => {
      let expression = parent.parent();
      let qualifier = expression.and_then(|it| it.named_child(0)).map(text_of);
      let kind = if expression.is_some_and(is_callee) {
        ReferenceKind::MemberCall
      } else {
        ReferenceKind::Member
      };
      (kind, qualifier)
    }
    // Java `a.b()` and `a.b`
    "method_invocation" if parent.child_by_field_name("name") == Some(node) => match parent.child_by_field_name("object") {
      Some(object) => (ReferenceKind::MemberCall, Some(text_of(object))),
      None => (ReferenceKind::Call, None),
    },
    "field_access" if parent.child_by_field_name("field") == Some(node) => (ReferenceKind::Member, parent.child_by_field_name("object").map(text_of)),
    // Rust `a.b` and TypeScript `a.b`
    "field_expression" | "member_expression" if parent.child_by_field_name("field").or_else(|| parent.child_by_field_name("property")) == Some(node) => {
      let object = parent.child_by_field_name("value").or_else(|| parent.child_by_field_name("object"));
      let kind = if is_callee(parent) { ReferenceKind::MemberCall } else { ReferenceKind::Member };
      (kind, object.map(text_of))
    }
    // Rust `a::b`
    "scoped_identifier" | "scoped_type_identifier" if parent.child_by_field_name("name") == Some(node) => {
      let kind = if parent.kind() == "scoped_type_identifier" {
        ReferenceKind::Type
      } else if is_callee(parent) {
        ReferenceKind::MemberCall
      } else {
        ReferenceKind::Member
      };
      (kind, parent.child_by_field_name("path").map(text_of))
    }
    _ if node.kind() == "type_identifier" => (ReferenceKind::Type, None),
    _ if is_callee(node) => (ReferenceKind::Call, None),
    _ => (ReferenceKind::Value, None),
  }
}

/// The declaration of `name` in the scopes around `node`: the nearest scope that declares it, the last declaration before
/// the name in it, else the first (a member declared after its use, a hoisted function)
fn local_declaration<'tree>(node: Node<'tree>, name: &str, text: &str) -> Option<Node<'tree>> {
  let mut scope = node.parent();
  while let Some(current) = scope {
    if SCOPES.contains(&current.kind()) || current.parent().is_none() || current.kind().ends_with("_body") {
      let mut found: Vec<Node<'tree>> = Vec::new();
      collect_declarations(current, name, text, &mut found);
      let before = found.iter().rev().find(|it| it.start_byte() <= node.start_byte()).copied();
      if let Some(declaration) = before.or_else(|| found.first().copied()) {
        return Some(declaration);
      }
    }
    scope = current.parent();
  }
  None
}

/// The names `scope` declares as `name`, without those of the scopes inside it but with their own names (a function
/// in a class declares its name in the class)
fn collect_declarations<'tree>(scope: Node<'tree>, name: &str, text: &str, found: &mut Vec<Node<'tree>>) {
  let mut stack: Vec<Node<'tree>> = Vec::new();
  let mut cursor = scope.walk();
  stack.extend(scope.named_children(&mut cursor));
  match parameters_of(scope) {
    // the one parameter of an arrow function is the name itself
    Some(own) if NAMES.contains(&own.kind()) => {
      if text.get(own.byte_range()) == Some(name) {
        found.push(own);
      }
    }
    Some(own) => stack.push(own),
    None => {}
  }
  while let Some(node) = stack.pop() {
    if let Some(declared) = declared_name(node)
      && text.get(declared.byte_range()) == Some(name)
    {
      found.push(declared);
    }
    for pattern_name in pattern_names(node) {
      if text.get(pattern_name.byte_range()) == Some(name) {
        found.push(pattern_name);
      }
    }
    if SCOPES.contains(&node.kind()) {
      continue;
    }
    let mut inner = node.walk();
    stack.extend(node.named_children(&mut inner));
  }
  found.sort_by_key(Node::start_byte);
  found.dedup_by_key(|it| it.id());
}

/// The parameters of a function scope, which lie beside its body: a list, or the one parameter of a TypeScript arrow
/// function without parentheses (`item => item.size`)
fn parameters_of(scope: Node<'_>) -> Option<Node<'_>> {
  scope.child_by_field_name("parameters").or_else(|| scope.child_by_field_name("parameter"))
}

/// The name a declaration node declares
fn declared_name(node: Node<'_>) -> Option<Node<'_>> {
  let first_of = |kind: &str| {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|it| it.kind() == kind)
  };
  match node.kind() {
    // Kotlin, whose grammar has few fields
    "function_declaration" if first_of("simple_identifier").is_some() && node.child_by_field_name("name").is_none() => first_of("simple_identifier"),
    "class_declaration" | "object_declaration" | "type_alias" | "type_parameter" if node.child_by_field_name("name").is_none() => first_of("type_identifier"),
    "variable_declaration" | "class_parameter" | "parameter" | "enum_entry" | "parameter_with_optional_type" if node.child_by_field_name("name").is_none() => {
      first_of("simple_identifier")
    }
    kind if is_declaration_kind(kind) => node.child_by_field_name("name").filter(|it| NAMES.contains(&it.kind())),
    _ => None,
  }
}

/// A node that declares the name in its `name` field: not a call or a path, which have one too
fn is_declaration_kind(kind: &str) -> bool {
  let suffix = [
    "_declaration",
    "_declarator",
    "_definition",
    "_signature",
    "parameter",
    "_constant",
    "_variant",
    "_component",
  ]
  .iter()
  .any(|it| kind.ends_with(it));
  suffix || (kind.ends_with("_item") && !matches!(kind, "use_item" | "macro_invocation"))
}

/// Whether a name is one that a pattern around it binds (`total` of `let mut total`, of `let (total, _)`), or the one
/// parameter of an arrow function
fn is_pattern_name(node: Node<'_>) -> bool {
  let mut current = node.parent();
  for _ in 0..6 {
    let Some(ancestor) = current else { return false };
    if pattern_names(ancestor).contains(&node) || (ancestor.kind() == "arrow_function" && ancestor.child_by_field_name("parameter") == Some(node)) {
      return true;
    }
    current = ancestor.parent();
  }
  false
}

/// The names a pattern binds: a Rust `let`, a parameter, a TypeScript destructuring
fn pattern_names(node: Node<'_>) -> Vec<Node<'_>> {
  let pattern = match node.kind() {
    "let_declaration" | "required_parameter" | "optional_parameter" | "parameter" => node.child_by_field_name("pattern"),
    _ => None,
  };
  let Some(pattern) = pattern else { return Vec::new() };
  let mut names = Vec::new();
  let mut stack = vec![pattern];
  while let Some(it) = stack.pop() {
    if matches!(it.kind(), "identifier" | "shorthand_property_identifier_pattern") {
      names.push(it);
    } else if !matches!(it.kind(), "type_annotation" | "type_identifier" | "primitive_type") {
      let mut cursor = it.walk();
      stack.extend(it.named_children(&mut cursor));
    }
  }
  names
}

/// The byte offset of a line and UTF-16 column, with the lines as the view splits them
fn byte_offset(text: &str, position: Position) -> Option<usize> {
  let bytes = text.as_bytes();
  let mut line = 0;
  let mut index = 0;
  while line < position.line {
    let next = index + bytes.get(index..)?.iter().position(|it| *it == b'\n' || *it == b'\r')?;
    index = next + if bytes[next] == b'\r' && bytes.get(next + 1) == Some(&b'\n') { 2 } else { 1 };
    line += 1;
  }
  let mut column = 0;
  for (offset, character) in text.get(index..)?.char_indices() {
    if column >= position.column || character == '\n' || character == '\r' {
      return Some(index + offset);
    }
    column += utf16_length_of(character);
  }
  Some(text.len())
}

/// The line and UTF-16 column of a byte offset
fn position_of(text: &str, offset: usize) -> Position {
  let before = text.get(..offset).unwrap_or(text);
  let bytes = before.as_bytes();
  let mut line = 0u32;
  let mut start = 0;
  let mut index = 0;
  while index < bytes.len() {
    if bytes[index] == b'\n' || bytes[index] == b'\r' {
      index += if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') { 2 } else { 1 };
      line += 1;
      start = index;
    } else {
      index += 1;
    }
  }
  Position {
    line,
    column: utf16_length(before.get(start..).unwrap_or_default()),
  }
}

fn utf16_length(text: &str) -> u32 {
  u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

const fn utf16_length_of(character: char) -> u32 {
  if character.len_utf16() == 2 { 2 } else { 1 }
}
