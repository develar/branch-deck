//! Java on `tree-sitter-java`. Fields and locals share `variable_declarator`, so the declarator is the declaration
//! and its statement only lends it modifiers and a doc comment. Constructors are no declarations: they carry the name
//! of their class, and a call of one is a reference to the class.

// jscpd:ignore-start: every language module implements one `Spec` with the same vocabulary
use crate::extract::{
  BLOCK_COMMENT, Binder, Child, DeclRule, Holder, IDENTIFIER, LINE_COMMENT, Rule, SCOPE, Spec, TYPE_IDENTIFIER, Traits, Walk, binder, decl, function_decl, line_of, type_decl,
};
use crate::language::Language;
use crate::summary::{DeclKind, MemberKind, Reach, Role};
use tree_sitter::Node;
// jscpd:ignore-end

const TYPE: Rule = type_decl(Child::Field("name"));
const CONSTRUCTOR: Rule = Rule::Bind(Binder {
  labels: &[Child::Field("name")],
  ..binder(true, &[])
});
const ANNOTATION: Rule = Rule::Call {
  callee: Child::Field("name"),
  role: Role::Type,
};

const RULES: &[(&str, Rule)] = &[
  LINE_COMMENT,
  BLOCK_COMMENT,
  IDENTIFIER,
  TYPE_IDENTIFIER,
  ("this", Rule::Keyword("this")),
  ("super", Rule::Keyword("super")),
  ("package_declaration", Rule::Special(package)),
  ("import_declaration", Rule::Special(import)),
  ("module_declaration", Rule::Skip),
  ("class_declaration", TYPE),
  ("interface_declaration", TYPE),
  ("enum_declaration", TYPE),
  ("record_declaration", TYPE),
  ("annotation_type_declaration", TYPE),
  ("annotation_type_element_declaration", Rule::Decl(decl(DeclKind::Function, Child::Field("name")))),
  ("method_declaration", function_decl(Child::Field("name"))),
  ("constructor_declaration", CONSTRUCTOR),
  ("compact_constructor_declaration", CONSTRUCTOR),
  ("field_declaration", Rule::Wrapper),
  ("constant_declaration", Rule::Wrapper),
  (
    "variable_declarator",
    Rule::Decl(DeclRule {
      late: true,
      ..decl(DeclKind::Value, Child::Field("name"))
    }),
  ),
  (
    "enum_constant",
    Rule::Decl(DeclRule {
      local_body: Some(Child::Field("body")),
      ..decl(DeclKind::EnumEntry, Child::Field("name"))
    }),
  ),
  ("formal_parameter", Rule::Special(parameter)),
  ("receiver_parameter", Rule::Skip),
  ("type_parameter", Rule::Bind(binder(false, &[Child::Kind(&["type_identifier"])]))),
  ("catch_formal_parameter", Rule::Bind(binder(false, &[Child::Field("name")]))),
  ("catch_clause", SCOPE),
  ("block", SCOPE),
  ("for_statement", SCOPE),
  ("enhanced_for_statement", Rule::Bind(binder(true, &[Child::Field("name")]))),
  ("try_with_resources_statement", SCOPE),
  (
    "resource",
    Rule::Bind(Binder {
      late: true,
      ..binder(false, &[Child::Field("name")])
    }),
  ),
  ("lambda_expression", Rule::Bind(binder(true, &[Child::Field("parameters")]))),
  ("switch_rule", SCOPE),
  ("switch_block_statement_group", SCOPE),
  ("instanceof_expression", Rule::Bind(binder(false, &[Child::Field("name")]))),
  ("type_pattern", Rule::Bind(binder(false, &[Child::Kind(&["identifier"])]))),
  ("record_pattern_component", Rule::Bind(binder(false, &[Child::Kind(&["identifier"])]))),
  // `case Point(int x, int y)`: the first child is the type
  (
    "record_pattern",
    Rule::Call {
      callee: Child::Nth(0),
      role: Role::Type,
    },
  ),
  (
    "method_invocation",
    Rule::Member {
      object: Child::Field("object"),
      name: Child::Field("name"),
      role: Some(Role::Call),
    },
  ),
  (
    "field_access",
    Rule::Member {
      object: Child::Field("object"),
      name: Child::Field("field"),
      role: None,
    },
  ),
  (
    "scoped_identifier",
    Rule::Member {
      object: Child::Field("scope"),
      name: Child::Field("name"),
      role: None,
    },
  ),
  (
    "scoped_type_identifier",
    Rule::Member {
      object: Child::Nth(0),
      name: Child::Last,
      role: None,
    },
  ),
  ("generic_type", Rule::Forward(Child::Nth(0))),
  ("object_creation_expression", Rule::Special(new)),
  ("method_reference", Rule::Special(method_reference)),
  ("annotation", ANNOTATION),
  ("marker_annotation", ANNOTATION),
  ("element_value_pair", Rule::Label(Child::Field("key"))),
  ("labeled_statement", Rule::Label(Child::Kind(&["identifier"]))),
  ("break_statement", Rule::Skip),
  ("continue_statement", Rule::Skip),
];

pub(crate) static SPEC: Spec = Spec {
  language: Language::Java,
  grammar: || tree_sitter_java::LANGUAGE.into(),
  node_types: tree_sitter_java::NODE_TYPES,
  rules: RULES,
  binding_names: &["identifier"],
  reserved: &["var"],
  kinds: &["modifiers", "formal_parameters", "class_body", "asterisk"],
  tokens: &["static", "final", "public", "protected", "private"],
  fields: &["name", "type"],
  traits,
  binds: |_| true,
  hoist: None,
};

fn package<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(path) = node.named_children(&mut node.walk()).find_map(|it| w.path_of(it)) {
    w.set_package(path.join("."));
  }
  false
}

fn import<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let Some(segments) = node.named_children(&mut node.walk()).find(|it| !w.is(*it, "asterisk")).and_then(|it| w.path_of(it)) else {
    return false;
  };
  let line = line_of(node);
  if w.child(node, Child::Kind(&["asterisk"])).is_some() {
    w.import_all(&segments, false, line);
  } else if let Some((name, path)) = segments.split_last() {
    w.import_name(path, name, None, false, line, line);
  }
  false
}

/// A parameter is local; a record component is a field (and its accessor)
fn parameter<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let Some(name) = w.field(node, "name") else { return true };
  let record = w.ancestor(1).is_some_and(|it| w.is(it, "formal_parameters")) && w.ancestor(2).is_some_and(|it| w.is(it, "record_declaration"));
  if record && !w.is_local() {
    let traits = Traits {
      kind: DeclKind::Value,
      reach: Reach::Public,
      member: MemberKind::Instance,
      receiver: None,
      holder: Holder::Leaf,
    };
    w.add_declaration(node, name, &traits);
  } else {
    w.bind_node(name);
  }
  true
}

/// `new Type(…)` calls the type; an anonymous class declares nothing
fn new<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(constructed) = w.field(node, "type") {
    w.give_role(constructed, Role::Call);
  }
  if let Some(body) = w.child(node, Child::Kind(&["class_body"])) {
    w.mark_scope(body);
  }
  true
}

/// `Type::method`, `object::method`; `Type::new` names no method
fn method_reference<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let count = node.named_child_count();
  if let (true, Some(object), Some(name)) = (count >= 2, node.named_child(0), node.named_child(u32::try_from(count).unwrap_or(1) - 1))
    && w.is(name, "identifier")
  {
    let qualifier = w.qualifier_of(object);
    w.emit(name, Role::Value, Some(qualifier));
  }
  true
}

fn traits<'t>(w: &mut Walk<'t>, node: Node<'t>, rule: &DeclRule) -> Option<Traits> {
  // a declarator takes the modifiers of its statement
  let owner = if w.is(node, "variable_declarator") { w.ancestor(1)? } else { node };
  let modifiers = w.child(owner, Child::Kind(&["modifiers"]));
  let has = |token: &str| modifiers.is_some_and(|it| w.has_token(it, token));
  let holder = w.holder();
  let in_interface = matches!(holder, Some(Holder::Interface | Holder::Annotation));
  let own = match node.kind() {
    "class_declaration" | "record_declaration" => Holder::Class,
    "interface_declaration" => Holder::Interface,
    "enum_declaration" => Holder::Enum,
    "annotation_type_declaration" => Holder::Annotation,
    _ => Holder::Leaf,
  };
  // only a class without `static` is inner; enums, records, interfaces and annotations are static
  let nested_type = own != Holder::Leaf && (!w.is(node, "class_declaration") || has("static"));
  let mut kind = rule.kind;
  if kind == DeclKind::Value && (in_interface || (has("static") && has("final"))) {
    kind = DeclKind::Constant;
  }
  let member = match holder {
    None => MemberKind::TopLevel,
    Some(_) if kind == DeclKind::EnumEntry || kind == DeclKind::Constant || nested_type || has("static") => MemberKind::Static,
    Some(_) if in_interface && own != Holder::Leaf => MemberKind::Static,
    Some(_) => MemberKind::Instance,
  };
  let reach = if has("public") || has("protected") || in_interface || kind == DeclKind::EnumEntry {
    Reach::Public
  } else if has("private") {
    if holder.is_some() { Reach::Container } else { Reach::File }
  } else {
    Reach::Module
  };
  Some(Traits {
    kind,
    reach,
    member,
    receiver: None,
    holder: own,
  })
}
