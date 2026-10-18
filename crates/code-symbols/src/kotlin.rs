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
//! - **Object bodies read as lambdas**: a one-line object expression `object : Foo() { override … }` parses as a call of
//!   the literal with a trailing lambda. A lambda hides the supertypes of the class around it, so these overrides are
//!   no references, rather than overrides of that class.
//! - **A class body read as the delegate's lambda**: in `: I by d { … }` (or `by delegate(…) { … }`) followed by
//!   another declaration, the body is a trailing lambda of the delegate. Its members are locals, but its overrides
//!   still override the supertypes of the class (`delegated_body`).
//!
//! Not handled: `fun interface` may parse as an ERROR, and the members of a class body read as the delegate's lambda
//! are no declarations.

// jscpd:ignore-start: every language module implements one `Spec` with the same vocabulary
use crate::extract::{
  Binder, CallArguments, Child, DeclRule, Holder, JvmFields, JvmHooks, LINE_COMMENT, Modality, Opens, Rule, SCOPE, Spec, TYPE_IDENTIFIER, Traits, Walk, binder, decl,
  function_decl, line_of, type_decl,
};
use crate::language::Language;
use crate::summary::{Argument, Constructor, DeclKind, MemberKind, Parameter, Reach, Role, Signature};
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
      lambda: true,
      ..binder(true, &[Child::Kind(&["lambda_parameters"])])
    }),
  ),
  // a function literal, as a lambda: it may have a receiver
  (
    "anonymous_function",
    Rule::Bind(Binder {
      lambda: true,
      ..binder(true, &[])
    }),
  ),
  ("anonymous_initializer", SCOPE),
  ("secondary_constructor", Rule::Special(secondary_constructor)),
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
  ("object_literal", Rule::Special(object_literal)),
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
    "delegation_specifier",
    "constructor_invocation",
    "explicit_delegation",
    "primary_constructor",
    "type_parameters",
    "parameter_modifiers",
    "parameter_modifier",
    "value_arguments",
    "spread_expression",
    "string_literal",
    "integer_literal",
    "hex_literal",
    "bin_literal",
    "boolean_literal",
  ],
  tokens: &["interface", "=", "*", "-", "+"],
  fields: &["receiver"],
  traits,
  binds: |_| true,
  hoist: None,
  jvm: Some(JvmHooks {
    signature,
    overrides,
    supertypes,
    primitives: &[],
    constructors,
    implicit_constructor: |_| Reach::Public,
  }),
  call_shape: Some(call_shape),
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
  let property = w.child(node, Child::Kind(&["binding_pattern_kind"])).is_some();
  // an `override val` of a local class overrides too
  let fields = if property {
    w.member_jvm_fields(node, name, DeclKind::Value)
  } else {
    JvmFields::default()
  };
  if !property || w.is_local() {
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
  w.add_declaration(node, name, &traits, fields);
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

/// An object expression: a scope whose members override those of its supertypes
fn object_literal<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  w.open_scope();
  let supertypes = supertypes(w, node).unwrap_or_default();
  w.push_supertypes(supertypes);
  true
}

/// `override`, `open`, `abstract`, `final` and `@JvmStatic` of a function or a property, and its extension receiver
fn overrides<'t>(w: &mut Walk<'t>, node: Node<'t>) -> Modality {
  let modifiers = modifier_texts(w, node);
  let has = |modifier: &str| modifiers.contains(&modifier);
  let overrides = has("override");
  let holder = w.holder();
  // a member of an object or a companion, or a local, can't be overridden
  let member = matches!(holder, Some(Holder::Class | Holder::Interface | Holder::Enum)) && !w.is_local();
  let open = has("open") || has("abstract") || (overrides && !has("final")) || holder == Some(Holder::Interface);
  let receiver = w.field(node, "receiver").and_then(|it| receiver_name(w, it)).map(|it| w.intern(it));
  Modality {
    overrides,
    overridable: member && open && !has("private"),
    jvm_static: annotated(w, node, &["JvmStatic"]),
    receiver,
  }
}

/// Whether an annotation among the modifiers of `node` names `path` (by its last name, or with `kotlin.jvm`)
fn annotated(w: &Walk<'_>, node: Node<'_>, path: &[&str]) -> bool {
  let Some(modifiers) = w.child(node, Child::Kind(&["modifiers"])) else {
    return false;
  };
  let mut cursor = modifiers.walk();
  modifiers
    .named_children(&mut cursor)
    .filter(|it| w.is(*it, "annotation"))
    .filter_map(|it| w.child(it, Child::Kind(&["user_type"])))
    .any(|it| {
      let names = segments(w, it);
      names == path || names.strip_prefix(["kotlin", "jvm"].as_slice()) == Some(path)
    })
}

/// The parameters of `function_value_parameters`: a `parameter_modifiers` (`vararg`) before its parameter, `= value`
/// after it
fn signature<'t>(w: &mut Walk<'t>, node: Node<'t>) -> Option<Signature> {
  let list = w.child(node, Child::Kind(&["function_value_parameters"]))?;
  let type_parameters = w.type_parameter_names(w.child(node, Child::Kind(&["type_parameters"])));
  let mut parameters: Vec<Parameter> = Vec::new();
  let mut vararg = false;
  let mut cursor = list.walk();
  let children: Vec<Node<'t>> = list.children(&mut cursor).collect();
  for child in children {
    if w.is_token(child, "=") {
      if let Some(last) = parameters.last_mut() {
        last.optional = true;
      }
    } else if w.is(child, "parameter_modifiers") {
      let mut cursor = child.walk();
      vararg = child.named_children(&mut cursor).any(|it| w.is(it, "parameter_modifier") && w.text(it) == Some("vararg"));
    } else if w.is(child, "parameter")
      && let Some(parameter) = parameter_of(w, child, std::mem::take(&mut vararg), &type_parameters)
    {
      parameters.push(parameter);
    }
  }
  Some(Signature { parameters })
}

/// A `parameter` or a `class_parameter`: its name and its type, a vararg as an array, as on the JVM; without a default
/// value, which the list of a function has beside it
fn parameter_of<'t>(w: &mut Walk<'t>, node: Node<'t>, vararg: bool, type_parameters: &[&'t str]) -> Option<Parameter> {
  let name = w.child(node, Child::Kind(&["simple_identifier"])).and_then(|it| w.text(it))?;
  let type_ref = w
    .child(node, Child::Kind(TYPES))
    .and_then(|it| w.type_ref(it, w.is(it, "nullable_type"), u8::from(vararg), type_parameters));
  Some(Parameter {
    name: w.intern(name),
    type_ref,
    optional: false,
    vararg,
  })
}

/// The primary constructor of a class: its class parameters (a vararg by `vararg` among its modifiers, optional by a
/// default value after it) and the reach of the modifiers before `constructor`; none without one
fn constructors<'t>(w: &mut Walk<'t>, node: Node<'t>, _: Reach) -> Vec<Constructor> {
  let Some(primary) = w.child(node, Child::Kind(&["primary_constructor"])) else {
    return Vec::new();
  };
  let type_parameters = w.type_parameter_names(w.child(node, Child::Kind(&["type_parameters"])));
  let mut cursor = primary.walk();
  let declared: Vec<Node<'t>> = primary.named_children(&mut cursor).filter(|it| w.is(*it, "class_parameter")).collect();
  let mut parameters = Vec::new();
  for class_parameter in declared {
    let vararg = modifier_texts(w, class_parameter).contains(&"vararg");
    if let Some(parameter) = parameter_of(w, class_parameter, vararg, &type_parameters) {
      parameters.push(Parameter {
        optional: w.has_token(class_parameter, "="),
        ..parameter
      });
    }
  }
  vec![Constructor {
    signature: Signature { parameters },
    reach: reach(&modifier_texts(w, primary), true),
  }]
}

/// A secondary constructor: a scope of its parameters, and a constructor of the class around it
fn secondary_constructor<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(signature) = signature(w, node) {
    let reach = reach(&modifier_texts(w, node), true);
    w.add_constructor(Constructor { signature, reach }, false);
  }
  w.open_scope();
  true
}

/// The supertypes of a class, an object, a companion or an object expression, the enum of an enum entry; the type of a
/// supertype's constructor invocation becomes a call
fn supertypes<'t>(w: &mut Walk<'t>, node: Node<'t>) -> Option<Vec<Vec<u32>>> {
  match node.kind() {
    "class_declaration" | "object_declaration" | "companion_object" | "object_literal" => {}
    "enum_entry" => return Some(w.enum_supertypes()),
    _ => return None,
  }
  let mut cursor = node.walk();
  let specifiers: Vec<Node<'t>> = node.named_children(&mut cursor).filter(|it| w.is(*it, "delegation_specifier")).collect();
  if w.child(node, Child::Kind(&["class_body", "enum_class_body"])).is_none()
    && let Some(body) = specifiers.last().and_then(|it| delegated_body(w, *it))
  {
    w.mark_class_body(body);
  }
  // `Base(…)` calls the constructor of `Base`, here and not as a node of its own: the shape is of the code around an
  // object expression, which pushes its supertypes after this, and an annotation `@Suppress("x")` is an invocation too
  for specifier in &specifiers {
    if let Some(invocation) = w.child(*specifier, Child::Kind(&["constructor_invocation"]))
      && let Some(constructed) = w.child(invocation, Child::Kind(&["user_type"]))
    {
      w.give_call(constructed, invocation);
    }
  }
  // a function type keeps its place as an empty path: the class has a supertype whose members are unknown
  Some(
    specifiers
      .iter()
      .map(|it| supertype_of(w, *it).and_then(|it| w.supertype_paths(&[it]).pop()).unwrap_or_default())
      .collect(),
  )
}

/// The type a delegation specifier names: `Base` of `: Base`, `: Base()` and `: Base by d`; a function type is none
fn supertype_of<'t>(w: &Walk<'t>, specifier: Node<'t>) -> Option<Node<'t>> {
  let child = w.child(specifier, Child::Kind(&["user_type", "constructor_invocation", "explicit_delegation"]))?;
  if w.is(child, "user_type") {
    Some(child)
  } else {
    w.child(child, Child::Kind(&["user_type"]))
  }
}

/// The body of a class without one, misread as the trailing lambda of the call its last delegate starts with: `{ … }`
/// of `: I by d { … }` before another declaration, where the delegate is an infix chain of `d { … }` and the words of
/// the next declaration
fn delegated_body<'t>(w: &Walk<'t>, specifier: Node<'t>) -> Option<Node<'t>> {
  let delegation = w.child(specifier, Child::Kind(&["explicit_delegation"]))?;
  let mut delegate = w.child(delegation, Child::Last)?;
  while w.is(delegate, "infix_expression") {
    delegate = delegate.named_child(0)?;
  }
  if !w.is(delegate, "call_expression") {
    return None;
  }
  trailing_lambda(w, delegate)
}

/// The arguments of a call or of a constructor invocation: those of `value_arguments` in order, a named one
/// (`name = value`) with its name, a spread (`*items`, `name = *items`), and the trailing lambda of a call last;
/// `foo {} {}` is a call of the call `foo {}`
fn call_shape<'t>(w: &mut Walk<'t>, node: Node<'t>) -> Option<CallArguments> {
  // a constructor invocation has its arguments in itself, a call in its suffix
  let suffix = if w.is(node, "constructor_invocation") {
    node
  } else {
    w.child(node, Child::Kind(&["call_suffix"]))?
  };
  let mut arguments = Vec::new();
  let mut spread = false;
  let values: Vec<Node<'t>> = match w.child(suffix, Child::Kind(&["value_arguments"])) {
    Some(list) => {
      let mut cursor = list.walk();
      list.named_children(&mut cursor).filter(|it| w.is(*it, "value_argument")).collect()
    }
    None => Vec::new(),
  };
  for value_argument in values {
    let mut cursor = value_argument.walk();
    let children: Vec<Node<'t>> = value_argument.named_children(&mut cursor).filter(|it| !w.is_comment(*it)).collect();
    // `[annotation] [name =] [*] value`
    let name = if w.has_token(value_argument, "=") {
      children.iter().find(|it| w.is(**it, "simple_identifier")).and_then(|it| w.text(*it)).map(|it| w.intern(it))
    } else {
      None
    };
    let value = children.last().copied();
    spread |= w.has_token(value_argument, "*") || value.is_some_and(|it| w.is(it, "spread_expression"));
    arguments.push((name, value.map_or(Argument::Expression, |it| argument(w, it))));
  }
  if w.child(suffix, Child::Kind(&["annotated_lambda"])).is_some() {
    arguments.push((None, Argument::Lambda));
  }
  Some((arguments, spread))
}

/// What a value argument is: a string literal or template, an integer without a suffix (`-1` too), `true` or `false`,
/// a lambda or an anonymous function; `null`, a real, a character, `1L` and `1u` are expressions
fn argument<'t>(w: &Walk<'t>, value: Node<'t>) -> Argument {
  let integer = |it: Node<'t>| w.is(it, "integer_literal") || w.is(it, "hex_literal") || w.is(it, "bin_literal");
  if w.is(value, "string_literal") {
    Argument::Text
  } else if integer(value)
    || (w.is(value, "prefix_expression") && (w.has_token(value, "-") || w.has_token(value, "+")) && value.named_child_count() == 1 && value.named_child(0).is_some_and(integer))
  {
    Argument::Number
  } else if w.is(value, "boolean_literal") {
    Argument::Boolean
  } else if w.is(value, "lambda_literal") || w.is(value, "anonymous_function") {
    Argument::Lambda
  } else {
    Argument::Expression
  }
}
