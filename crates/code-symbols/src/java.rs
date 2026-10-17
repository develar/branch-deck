//! Java on `tree-sitter-java`. Fields and locals share `variable_declarator`, so the declarator is the declaration
//! and its statement only lends it modifiers and a doc comment. Constructors are no declarations: they carry the name
//! of their class, and a call of one is a reference to the class. An anonymous class declares nothing, but its
//! `@Override` methods override the members of the type it constructs.

// jscpd:ignore-start: every language module implements one `Spec` with the same vocabulary
use crate::extract::{
  BLOCK_COMMENT, Binder, Child, DeclRule, Holder, IDENTIFIER, Inheritance, LINE_COMMENT, Modality, Rule, SCOPE, Spec, TYPE_IDENTIFIER, Traits, Walk, binder, decl, function_decl,
  line_of, type_decl,
};
use crate::language::Language;
use crate::summary::{DeclKind, MemberKind, Parameter, Reach, Role, Signature};
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
  kinds: &[
    "modifiers",
    "formal_parameters",
    "class_body",
    "asterisk",
    "spread_parameter",
    "type_list",
    "extends_interfaces",
    "integral_type",
    "floating_point_type",
    "boolean_type",
    "array_type",
  ],
  tokens: &["static", "final", "public", "protected", "private"],
  fields: &["name", "type", "superclass", "interfaces", "type_parameters", "dimensions"],
  traits,
  binds: |_| true,
  hoist: None,
  inheritance: Some(Inheritance {
    signature,
    overrides,
    supertypes,
    primitives: &["integral_type", "floating_point_type", "boolean_type"],
  }),
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

/// `new Type(…)` calls the type; an anonymous class declares nothing, its methods override those of the type
fn new<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let constructed = w.field(node, "type");
  if let Some(constructed) = constructed {
    w.give_role(constructed, Role::Call);
  }
  if let Some(body) = w.child(node, Child::Kind(&["class_body"])) {
    w.mark_scope(body);
    let supertypes = w.supertype_paths(&Vec::from_iter(constructed));
    w.push_supertypes(supertypes);
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

/// `@Override` (or `@java.lang.Override`), `final`, `private` and `static` of a method; a field is never an override
fn overrides<'t>(w: &mut Walk<'t>, node: Node<'t>) -> Modality {
  if !w.is(node, "method_declaration") {
    return Modality::default();
  }
  let modifiers = w.child(node, Child::Kind(&["modifiers"]));
  let has = |token: &str| modifiers.is_some_and(|it| w.has_token(it, token));
  let overrides = modifiers.is_some_and(|it| {
    let mut cursor = it.walk();
    it.named_children(&mut cursor)
      .filter(|it| w.is(*it, "marker_annotation") || w.is(*it, "annotation"))
      .filter_map(|it| w.field(it, "name").and_then(|name| w.path_of(name)))
      .any(|path| matches!(path.as_slice(), ["Override"] | ["java", "lang", "Override"]))
  });
  let open = match w.holder() {
    Some(Holder::Interface) => true,
    Some(Holder::Class | Holder::Enum) => !in_final_class(w),
    _ => false,
  };
  Modality {
    overrides,
    overridable: open && !w.is_local() && !has("final") && !has("private") && !has("static"),
    receiver: None,
  }
}

/// Whether the class whose body holds the current member is `final` or a record
fn in_final_class(w: &Walk<'_>) -> bool {
  let Some(class) = w.ancestor(2) else { return false };
  w.is(class, "record_declaration") || (w.is(class, "class_declaration") && w.child(class, Child::Kind(&["modifiers"])).is_some_and(|it| w.has_token(it, "final")))
}

/// The `formal_parameter`s and the `spread_parameter` (`String... rest`, no fields) of a method; an annotation element
/// has none
#[expect(
  clippy::unnecessary_wraps,
  reason = "the signature of `Inheritance::signature`, whose Kotlin one has none without a parameter list"
)]
fn signature<'t>(w: &mut Walk<'t>, node: Node<'t>) -> Option<Signature> {
  let Some(list) = w.field(node, "parameters") else {
    return Some(Signature { parameters: Vec::new() });
  };
  let type_parameters = w.type_parameter_names(w.field(node, "type_parameters"));
  let mut cursor = list.walk();
  let children: Vec<Node<'t>> = list.named_children(&mut cursor).collect();
  let mut parameters = Vec::new();
  for child in children {
    // the `[]` after the name of `String names[]`
    let (type_node, name, vararg, declarator) = if w.is(child, "formal_parameter") {
      (w.field(child, "type"), w.field(child, "name"), false, Some(child))
    } else if w.is(child, "spread_parameter") {
      let mut cursor = child.walk();
      let type_node = child.named_children(&mut cursor).find(|it| !w.is(*it, "modifiers") && !w.is(*it, "variable_declarator"));
      let name = w.child(child, Child::Kind(&["variable_declarator"])).and_then(|it| w.field(it, "name"));
      (type_node, name, true, None)
    } else {
      continue;
    };
    let Some(name) = name.and_then(|it| w.text(it)) else { continue };
    // `int[][] x`, `int x[]` and `int... x` are arrays on the JVM
    let of_type = type_node.filter(|it| w.is(*it, "array_type"));
    let arrays = dimensions(w, of_type).saturating_add(dimensions(w, declarator)).saturating_add(u8::from(vararg));
    let type_ref = type_node.and_then(|it| w.type_ref(it, false, arrays, &type_parameters));
    parameters.push(Parameter {
      name: w.intern(name),
      type_ref,
      optional: false,
      vararg,
    });
  }
  Some(Signature { parameters })
}

/// The `[]` of the `dimensions` field of an `array_type` or a `formal_parameter`
fn dimensions<'t>(w: &Walk<'t>, node: Option<Node<'t>>) -> u8 {
  let text = node.and_then(|it| w.field(it, "dimensions")).and_then(|it| w.text(it)).unwrap_or_default();
  u8::try_from(text.matches('[').count()).unwrap_or(u8::MAX)
}

/// `extends` and `implements` of a class, an enum or a record, `extends` of an interface, the enum of an enum constant
fn supertypes<'t>(w: &mut Walk<'t>, node: Node<'t>) -> Option<Vec<Vec<u32>>> {
  match node.kind() {
    "class_declaration" | "interface_declaration" | "enum_declaration" | "record_declaration" | "annotation_type_declaration" => {}
    "enum_constant" => return Some(w.enum_supertypes()),
    _ => return None,
  }
  let mut types = Vec::new();
  // the superclass, then the type lists of `implements` (a field) and of an interface's `extends` (no field)
  let lists = [w.field(node, "interfaces"), w.child(node, Child::Kind(&["extends_interfaces"]))].map(|it| it.and_then(|it| w.child(it, Child::Kind(&["type_list"]))));
  for parent in std::iter::once(w.field(node, "superclass")).chain(lists).flatten() {
    let mut cursor = parent.walk();
    types.extend(parent.named_children(&mut cursor));
  }
  Some(w.supertype_paths(&types))
}
