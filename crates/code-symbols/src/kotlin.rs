//! Kotlin on `tree-sitter-kotlin-sg` (never `-kotlin-ng`: both export the C symbol `tree_sitter_kotlin`). The grammar
//! marks few fields, so most rules select children by kind.
//!
//! Grammar quirks handled here:
//! - **Misread declarations**: a declaration with two modifiers before `class` or `interface` and anything after it
//!   (`internal abstract class A { … }`, `private companion object { … }`), or a one-line `object A { … }`, is
//!   ambiguous to the grammar, which may read it as infix calls of its words with a lambda, without an ERROR node. In
//!   37 of 700 Kotlin files of a monorepo sample that dropped whole classes. `misread_declaration` declares the type
//!   and walks the lambda as its body.
//! - **Accessors beside their property**: the getter and setter on the lines below a member property are its siblings
//!   in the class body; they extend it and its references are inside it.
//!
//! Not handled: `fun interface` may parse as an ERROR, and in `: I by delegate(…) { … }` followed by another
//! declaration the body is a trailing lambda of the delegate, so its members are locals.

// jscpd:ignore-start: every language module implements one `Spec` with the same vocabulary
use crate::extract::{Binder, Child, DeclRule, Holder, LINE_COMMENT, Opens, Rule, SCOPE, Spec, TYPE_IDENTIFIER, Traits, Walk, binder, decl, function_decl, line_of, type_decl};
use crate::language::Language;
use crate::summary::{DeclKind, MemberKind, Reach, Role};
use tree_sitter::Node;
// jscpd:ignore-end

const TYPES: &[&str] = &["function_type", "not_nullable_type", "nullable_type", "parenthesized_type", "user_type"];
const VARIABLES: &[&str] = &["variable_declaration", "multi_variable_declaration"];
const NAVIGATION: Rule = Rule::Member {
  object: Child::Nth(0),
  name: Child::Kind(&["navigation_suffix"]),
  role: None,
};

const RULES: &[(&str, Rule)] = &[
  LINE_COMMENT,
  ("multiline_comment", Rule::Comment),
  ("shebang_line", Rule::Skip),
  ("label", Rule::Skip),
  ("simple_identifier", Rule::Name(Role::Value)),
  ("interpolated_identifier", Rule::Name(Role::Value)),
  TYPE_IDENTIFIER,
  ("this_expression", Rule::Keyword("this")),
  ("super_expression", Rule::Keyword("super")),
  ("package_header", Rule::Special(package)),
  ("import_header", Rule::Special(import)),
  ("navigation_expression", NAVIGATION),
  // `a.b = c` is no navigation expression
  ("directly_assignable_expression", NAVIGATION),
  (
    "callable_reference",
    Rule::Member {
      object: Child::Kind(&["type_identifier"]),
      name: Child::Kind(&["simple_identifier"]),
      role: None,
    },
  ),
  ("user_type", Rule::Path),
  (
    "call_expression",
    Rule::Call {
      callee: Child::Nth(0),
      role: Role::Call,
    },
  ),
  ("infix_expression", Rule::Special(infix)),
  // annotations the grammar puts before a misread declaration instead of in its modifiers
  ("annotation", Rule::Special(annotation)),
  ("prefix_expression", Rule::Special(prefix)),
  ("value_argument", Rule::Special(value_argument)),
  ("class_declaration", type_decl(Child::Kind(&["type_identifier"]))),
  ("object_declaration", Rule::Decl(decl(DeclKind::Type, Child::Kind(&["type_identifier"])))),
  (
    "companion_object",
    Rule::Decl(DeclRule {
      default_name: Some("Companion"),
      ..decl(DeclKind::Type, Child::Kind(&["type_identifier"]))
    }),
  ),
  (
    "type_alias",
    Rule::Decl(DeclRule {
      opens: Opens::Members,
      ..decl(DeclKind::TypeAlias, Child::Kind(&["type_identifier"]))
    }),
  ),
  ("function_declaration", function_decl(Child::Kind(&["simple_identifier"]))),
  (
    "property_declaration",
    Rule::Decl(DeclRule {
      late: true,
      pattern: Some(Child::Kind(VARIABLES)),
      ..decl(DeclKind::Value, Child::Kind(&["variable_declaration"]))
    }),
  ),
  (
    "enum_entry",
    Rule::Decl(DeclRule {
      local_body: Some(Child::Kind(&["class_body"])),
      ..decl(DeclKind::EnumEntry, Child::Kind(&["simple_identifier"]))
    }),
  ),
  ("class_parameter", Rule::Special(class_parameter)),
  ("parameter", Rule::Special(parameter)),
  ("parameter_with_optional_type", Rule::Bind(binder(false, &[Child::Kind(&["simple_identifier"])]))),
  ("type_parameter", Rule::Bind(binder(false, &[Child::Kind(&["type_identifier"])]))),
  ("variable_declaration", Rule::Unbind(&[Child::Kind(&["type_modifiers"]), Child::Kind(TYPES)])),
  (
    "lambda_literal",
    Rule::Bind(Binder {
      implicit: &["it"],
      ..binder(true, &[Child::Kind(&["lambda_parameters"])])
    }),
  ),
  ("anonymous_function", SCOPE),
  ("anonymous_initializer", SCOPE),
  ("secondary_constructor", SCOPE),
  ("getter", Rule::Special(accessor)),
  ("setter", Rule::Special(accessor)),
  ("control_structure_body", SCOPE),
  ("for_statement", Rule::Bind(binder(true, &[Child::Kind(VARIABLES)]))),
  ("catch_block", Rule::Bind(binder(true, &[Child::Kind(&["simple_identifier"])]))),
  ("when_expression", SCOPE),
  (
    "when_subject",
    Rule::Bind(Binder {
      late: true,
      ..binder(false, &[Child::Kind(&["variable_declaration"])])
    }),
  ),
  ("object_literal", SCOPE),
];

pub(crate) static SPEC: Spec = Spec {
  language: Language::Kotlin,
  grammar: || tree_sitter_kotlin_sg::LANGUAGE.into(),
  node_types: tree_sitter_kotlin_sg::NODE_TYPES,
  rules: RULES,
  binding_names: &["simple_identifier"],
  reserved: &[],
  kinds: &[
    "modifiers",
    "enum_class_body",
    "binding_pattern_kind",
    "function_value_parameters",
    "identifier",
    "wildcard_import",
    "import_alias",
    "lambda_literal",
    "call_suffix",
    "annotated_lambda",
    "nullable_type",
  ],
  tokens: &["interface", "="],
  fields: &["receiver"],
  traits,
  binds: |_| true,
  hoist: None,
};

fn package<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(name) = w.child(node, Child::Kind(&["identifier"])) {
    let segments = segments(w, name);
    w.set_package(segments.join("."));
  }
  false
}

/// A parameter of a function type names nothing
fn parameter<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(name) = w.child(node, Child::Kind(&["simple_identifier"])) {
    if w.ancestor(1).is_some_and(|it| w.is(it, "function_value_parameters")) {
      w.bind_node(name);
    } else {
      w.claim(name);
    }
  }
  true
}

/// A getter or a setter; those of a member property are its siblings in this grammar
fn accessor<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  w.open_scope();
  w.bind_text("field");
  if let Some(property) = w.previous_declaration().filter(|it| w.declaration_kind(*it) == Some(DeclKind::Value))
    && !w.ancestor(1).is_some_and(|it| w.is(it, "property_declaration"))
  {
    w.extend_declaration(property, node);
    w.enter_container(Some(property), Holder::Leaf, None);
  }
  true
}

/// `name = value`: the name is a label
fn value_argument<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if w.has_token(node, "=")
    && let Some(name) = node.named_child(0).filter(|it| w.is(*it, "simple_identifier"))
  {
    w.claim(name);
  }
  true
}

/// An annotation the grammar puts before a misread declaration instead of in its modifiers extends it
fn annotation<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  w.extend_leading(node);
  true
}

fn prefix<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if node.named_child(0).is_some_and(|it| w.is(it, "annotation")) {
    w.pass_leading(node);
  }
  true
}

fn segments<'t>(w: &Walk<'t>, identifier: Node<'t>) -> Vec<&'t str> {
  let mut cursor = identifier.walk();
  identifier.named_children(&mut cursor).filter_map(|it| w.text(it)).collect()
}

fn import<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let Some(identifier) = w.child(node, Child::Kind(&["identifier"])) else { return false };
  let segments = segments(w, identifier);
  let line = line_of(node);
  if w.child(node, Child::Kind(&["wildcard_import"])).is_some() {
    w.import_all(&segments, false, line);
    return false;
  }
  let Some((name, path)) = segments.split_last() else { return false };
  let alias = w.child(node, Child::Kind(&["import_alias"])).and_then(|it| it.named_child(0));
  let alias_text = alias.and_then(|it| w.text(it));
  let name_line = identifier
    .named_child(identifier.named_child_count().saturating_sub(1).try_into().unwrap_or(0))
    .map_or(line, line_of);
  w.import_name(path, name, alias_text, false, line, name_line);
  if let (Some(alias), Some(alias_text)) = (alias, alias_text) {
    w.add_binding(node, alias, alias_text, Reach::File);
  }
  false
}

/// A `val`/`var` constructor parameter is a property; a plain one is visible in the class body
fn class_parameter<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let Some(name) = w.child(node, Child::Kind(&["simple_identifier"])) else { return true };
  if w.child(node, Child::Kind(&["binding_pattern_kind"])).is_none() || w.is_local() {
    w.bind_node(name);
    return true;
  }
  let modifiers = modifier_texts(w, node);
  let traits = Traits {
    kind: DeclKind::Value,
    reach: reach(&modifiers, true),
    member: MemberKind::Instance,
    receiver: None,
    holder: Holder::Leaf,
  };
  w.add_declaration(node, name, &traits);
  true
}

/// `a to b` calls `to` on `a`, unless the infix chain is a declaration the grammar misread
fn infix<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if !w.is_local() && misread_declaration(w, node) {
    return true;
  }
  if let (Some(left), Some(name)) = (node.named_child(0), node.named_child(1))
    && w.is(name, "simple_identifier")
  {
    let qualifier = w.qualifier_of(left);
    w.emit(name, Role::Call, Some(qualifier));
  }
  true
}

/// The words before `class`, `interface` or `object` in a misread declaration
const CLASS_MODIFIERS: &[&str] = &[
  "public",
  "private",
  "protected",
  "internal",
  "sealed",
  "data",
  "abstract",
  "open",
  "inner",
  "enum",
  "annotation",
  "value",
  "inline",
  "expect",
  "actual",
  "final",
  "companion",
  "external",
  "fun",
];

/// Where the whole file is ambiguous, the grammar may read a declaration as infix calls of its words, with no ERROR:
/// `internal sealed interface Name { … }` as identifiers and a lambda, `class Name { … }` as a call of `Name` with a
/// trailing lambda, `private companion object { … }` as identifiers and an object literal, a one-line `object Name { … }`
/// as an empty object literal, a name and a lambda. Such a chain declares the type, and its lambda is the class body.
fn misread_declaration<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let (items, inner) = infix_items(w, node);
  let keyword = |it: &Node<'t>| w.is(*it, "object_literal") || (w.is(*it, "simple_identifier") && matches!(w.text(*it), Some("class" | "interface" | "object")));
  let Some(at) = items.iter().position(keyword) else {
    return false;
  };
  let mut modifiers = Vec::new();
  for item in &items[..at] {
    match w.text(*item) {
      Some(text) if w.is(*item, "simple_identifier") && CLASS_MODIFIERS.contains(&text) => modifiers.push(text),
      _ => return false,
    }
  }
  let keyword = items[at];
  let object_literal = w.is(keyword, "object_literal");
  let (name, call, body) = match &items[at + 1..] {
    [] if object_literal && keyword.named_child_count() > 0 && modifiers.contains(&"companion") => (None, None, Some(keyword)),
    [name] if w.is(*name, "simple_identifier") => (Some(*name), None, None),
    [name, body] if w.is(*name, "simple_identifier") && w.is(*body, "lambda_literal") => (Some(*name), None, Some(*body)),
    [call] if w.is(*call, "call_expression") => {
      let name = call.named_child(0).filter(|it| w.is(*it, "simple_identifier"));
      (name, Some(*call), trailing_lambda(w, *call))
    }
    _ => return false,
  };
  if object_literal && keyword.named_child_count() > 0 && name.is_some() {
    return false;
  }
  let holder = if object_literal || w.text(keyword) == Some("object") {
    Holder::Object
  } else if w.text(keyword) == Some("interface") {
    Holder::Interface
  } else if modifiers.contains(&"enum") {
    Holder::Enum
  } else if modifiers.contains(&"annotation") {
    Holder::Annotation
  } else {
    Holder::Class
  };
  let member = if modifiers.contains(&"inner") {
    MemberKind::Instance
  } else {
    member_in(w.holder(), true)
  };
  let traits = Traits {
    kind: DeclKind::Type,
    reach: reach(&modifiers, w.holder().is_some()),
    member,
    receiver: None,
    holder,
  };
  let (name_node, text) = match name.and_then(|it| Some((it, w.text(it)?))) {
    Some(named) => named,
    None => (keyword, "Companion"),
  };
  let words: Vec<Node<'t>> = items[..=at].iter().chain(&name).copied().filter(|it| w.is(*it, "simple_identifier")).collect();
  for word in words {
    w.claim(word);
  }
  for infix in inner {
    w.mark_plain(infix);
  }
  if let Some(call) = call {
    w.mark_plain(call);
  }
  if let Some(body) = body {
    w.mark_plain(body);
  }
  let id = w.declare_misread(node, name_node, text, &traits);
  w.enter_container(Some(id), holder, None);
  true
}

/// The operands and operators of a left-nested infix chain in source order, and the infix nodes inside it
fn infix_items<'t>(w: &Walk<'t>, node: Node<'t>) -> (Vec<Node<'t>>, Vec<Node<'t>>) {
  let mut items = Vec::new();
  let mut inner = Vec::new();
  let mut node = node;
  loop {
    let mut cursor = node.walk();
    let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
    let Some((first, rest)) = children.split_first() else { break };
    items.splice(0..0, rest.iter().copied());
    if w.is(*first, "infix_expression") {
      inner.push(*first);
      node = *first;
    } else {
      items.insert(0, *first);
      break;
    }
  }
  (items, inner)
}

/// The lambda of `Name { … }`
fn trailing_lambda<'t>(w: &Walk<'t>, call: Node<'t>) -> Option<Node<'t>> {
  let suffix = w.child(call, Child::Kind(&["call_suffix"]))?;
  let annotated = w.child(suffix, Child::Kind(&["annotated_lambda"]))?;
  w.child(annotated, Child::Kind(&["lambda_literal"]))
}

fn modifier_texts<'t>(w: &Walk<'t>, node: Node<'t>) -> Vec<&'t str> {
  let Some(modifiers) = w.child(node, Child::Kind(&["modifiers"])) else {
    return Vec::new();
  };
  let mut cursor = modifiers.walk();
  modifiers
    .named_children(&mut cursor)
    .filter(|it| !w.is(*it, "annotation"))
    .filter_map(|it| w.text(it))
    .collect()
}

fn reach(modifiers: &[&str], member: bool) -> Reach {
  if modifiers.contains(&"private") {
    if member { Reach::Container } else { Reach::File }
  } else if modifiers.contains(&"internal") {
    Reach::Module
  } else {
    Reach::Public
  }
}

const fn member_in(holder: Option<Holder>, nested_type: bool) -> MemberKind {
  match holder {
    None => MemberKind::TopLevel,
    Some(Holder::Object) => MemberKind::Static,
    Some(_) if nested_type => MemberKind::Static,
    Some(_) => MemberKind::Instance,
  }
}

#[expect(clippy::unnecessary_wraps, reason = "the signature of `Spec::traits`, whose TypeScript one declines declarations")]
fn traits<'t>(w: &mut Walk<'t>, node: Node<'t>, rule: &DeclRule) -> Option<Traits> {
  let modifiers = modifier_texts(w, node);
  let holder = w.holder();
  let member = holder.is_some();
  let mut kind = rule.kind;
  let mut own = Holder::Leaf;
  let mut member_kind = member_in(holder, false);
  match node.kind() {
    "class_declaration" => {
      own = if w.has_token(node, "interface") {
        Holder::Interface
      } else if w.child(node, Child::Kind(&["enum_class_body"])).is_some() {
        Holder::Enum
      } else if modifiers.contains(&"annotation") {
        Holder::Annotation
      } else {
        Holder::Class
      };
      member_kind = if modifiers.contains(&"inner") { MemberKind::Instance } else { member_in(holder, true) };
    }
    "object_declaration" | "companion_object" => {
      own = Holder::Object;
      member_kind = member_in(holder, true);
    }
    "type_alias" => member_kind = member_in(holder, true),
    "enum_entry" => member_kind = MemberKind::Static,
    "property_declaration" if modifiers.contains(&"const") => kind = DeclKind::Constant,
    _ => {}
  }
  let receiver = w.field(node, "receiver").and_then(|it| receiver_name(w, it)).map(|it| w.intern(it));
  let reach = if kind == DeclKind::EnumEntry { Reach::Public } else { reach(&modifiers, member) };
  Some(Traits {
    kind,
    reach,
    member: member_kind,
    receiver,
    holder: own,
  })
}

/// The type name of a simple receiver (`String`, `a.Foo<T>`, `Foo?`)
fn receiver_name<'t>(w: &Walk<'t>, receiver: Node<'t>) -> Option<&'t str> {
  let mut node = w.child(receiver, Child::Kind(&["user_type", "nullable_type"]))?;
  if w.is(node, "nullable_type") {
    node = w.child(node, Child::Kind(&["user_type"]))?;
  }
  let mut cursor = node.walk();
  let name = node.named_children(&mut cursor).filter(|it| w.is(*it, "type_identifier")).last()?;
  w.text(name)
}
