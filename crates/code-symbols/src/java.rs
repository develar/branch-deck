//! Java on `tree-sitter-java`. Fields and locals share `variable_declarator`, so the declarator is the declaration
//! and its statement only lends it modifiers and a doc comment. A constructor is no declaration but one of the
//! `constructors` of its class, which carries its name, and `new` calls the class with the shape of its arguments. An
//! anonymous class declares nothing, but its `@Override` methods override the members of the type it constructs.

// jscpd:ignore-start: every language module implements one `Spec` with the same vocabulary
use crate::extract::{
  BLOCK_COMMENT, Binder, CallArguments, Child, DeclRule, Header, Holder, IDENTIFIER, JvmFields, JvmHooks, LINE_COMMENT, Modality, Rule, SCOPE, Spec, TYPE_IDENTIFIER, Traits,
  ValueType, Walk, binder, decl, function_decl, line_of, type_decl,
};
use crate::language::Language;
use crate::summary::{Argument, Constructor, DeclKind, MemberKind, Parameter, Reach, Role, Signature};
use tree_sitter::Node;
// jscpd:ignore-end

const TYPE: Rule = type_decl(Child::Field("name"));
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
  ("constructor_declaration", Rule::Special(constructor)),
  ("compact_constructor_declaration", Rule::Special(constructor)),
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
  ("instanceof_expression", Rule::Special(instanceof)),
  ("cast_expression", Rule::Special(cast)),
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
  ("explicit_constructor_invocation", Rule::Special(constructor_invocation)),
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
    "string_literal",
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "true",
    "false",
    "unary_expression",
  ],
  tokens: &["static", "final", "public", "protected", "private", "-", "+"],
  fields: &[
    "name",
    "type",
    "superclass",
    "interfaces",
    "type_parameters",
    "dimensions",
    "arguments",
    "operator",
    "operand",
    "constructor",
    "left",
    "value",
  ],
  traits,
  binds: |_| true,
  hoist: None,
  jvm: Some(JvmHooks {
    signature,
    overrides,
    supertypes,
    primitives: &["integral_type", "floating_point_type", "boolean_type"],
    constructors,
    // javac gives the default constructor the access of its class (JLS 8.8.9)
    implicit_constructor: |reach| reach,
    value_type,
    receiver_node: |_, node| node,
  }),
  call_shape: Some(call_shape),
  coroutine: Some(|_, _| None),
  header: Header {
    ends: &["block", "constructor_body"],
    end_tokens: &["="],
    drops: &["marker_annotation", "annotation"],
    drop_tokens: &[
      "public",
      "protected",
      "private",
      "final",
      "abstract",
      "synchronized",
      "native",
      "strictfp",
      "transient",
      "volatile",
      "default",
      ";",
    ],
    parameters: &["formal_parameters"],
  },
};

fn package<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  w.import_statement(node);
  if let Some(path) = node.named_children(&mut node.walk()).find_map(|it| w.path_of(it)) {
    w.set_package(path.join("."));
  }
  false
}

fn import<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  w.import_statement(node);
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

/// The type of a field, a local or a parameter (`JvmHooks::value_type`), as written without type arguments
/// (`java.util.List` of `java.util.List<T>`), not a type parameter, a primitive or an array; of a `var` the type its
/// initializer constructs, a `new` without a body (an anonymous class is a type of its own)
fn value_type<'t>(w: &Walk<'t>, node: Node<'t>) -> Option<ValueType<'t>> {
  let declared = if w.is(node, "variable_declarator") {
    w.field(node.parent()?, "type")
  } else {
    w.field(node, "type")
  }?;
  let constructed = w.text(declared) == Some("var");
  let path = if constructed {
    let value = w
      .field(node, "value")
      .filter(|it| w.is(*it, "object_creation_expression") && w.child(*it, Child::Kind(&["class_body"])).is_none())?;
    w.path_of(w.field(value, "type")?)?
  } else {
    w.path_of(declared)?
  };
  (!(path.len() == 1 && w.is_bound_text(path[0]))).then_some((path, constructed))
}

/// `m instanceof Sub s`: the pattern binds `s`, and a cast may give `m` another type
fn instanceof<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(name) = w.field(node, "name") {
    w.mark_pattern(name);
  }
  if let Some(left) = w.field(node, "left") {
    w.smart_cast(left);
  }
  true
}

/// `(Sub) m`
fn cast<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(value) = w.field(node, "value") {
    w.smart_cast(value);
  }
  true
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
    w.add_declaration(node, name, &traits, JvmFields::default());
  } else {
    w.type_binding(name, node);
    w.bind_node(name);
  }
  true
}

/// `new Type(…)` calls the type with the shape of its arguments, which are of the code around an anonymous class; an
/// anonymous class declares nothing, its methods override those of the type
fn new<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let constructed = w.field(node, "type");
  if let Some(constructed) = constructed {
    w.give_call(constructed, node);
  }
  if let Some(body) = w.child(node, Child::Kind(&["class_body"])) {
    w.mark_scope(body);
    let supertypes = w.supertype_paths(constructed.as_slice());
    w.push_supertypes(supertypes);
  }
  true
}

/// `this(…)` and `super(…)` of a constructor: a call of a constructor of the class around it, or of its superclass,
/// the type of `extends` (a class without one calls `Object`'s: no reference); the type is a call of its own here, as
/// the walk sees no node of its name
fn constructor_invocation<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let Some(keyword) = w.field(node, "constructor") else { return true };
  // the class around the constructor: an enum's body has its constructors in a list of its own
  let class = std::iter::successors(node.parent(), Node::parent).find(|it| ["class_declaration", "enum_declaration", "record_declaration"].iter().any(|kind| w.is(*it, kind)));
  let Some(class) = class else { return true };
  let path = if w.is(keyword, "this") {
    w.field(class, "name").and_then(|it| w.text(it)).map(|it| vec![it])
  } else if w.is(class, "class_declaration") {
    w.field(class, "superclass").and_then(|it| it.named_child(0)).and_then(|it| w.path_of(it))
  } else {
    None
  };
  if let Some(path) = path {
    w.emit_constructor_call(&path, line_of(keyword), node);
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
  let reach = reach_of(w, modifiers, holder.is_some(), in_interface || kind == DeclKind::EnumEntry);
  Some(Traits {
    kind,
    reach,
    member,
    receiver: None,
    holder: own,
  })
}

/// The reach of these modifiers: `public` and `protected` are public (and so is a member of an interface or an enum
/// constant without them), `private` of the container (of the file at the top level), none of the package
fn reach_of<'t>(w: &Walk<'t>, modifiers: Option<Node<'t>>, member: bool, public: bool) -> Reach {
  let has = |token: &str| modifiers.is_some_and(|it| w.has_token(it, token));
  if public || has("public") || has("protected") {
    Reach::Public
  } else if has("private") {
    if member { Reach::Container } else { Reach::File }
  } else {
    Reach::Module
  }
}

/// The canonical constructor of a record, of its components and the reach of the record; a class declares its
/// constructors in its body
fn constructors<'t>(w: &mut Walk<'t>, node: Node<'t>, reach: Reach) -> Vec<Constructor> {
  if !w.is(node, "record_declaration") {
    return Vec::new();
  }
  signature(w, node).map(|signature| Constructor { signature, reach }).into_iter().collect()
}

/// A constructor, a scope of its parameters, whose name is no reference: a constructor of the class around it. The
/// canonical one of a record (JLS 8.10.4), compact or declared again with the names and types of its components, every
/// type known, replaces the one of the record's header
fn constructor<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let header = w.ancestor(2).filter(|it| w.is(*it, "record_declaration")).and_then(|it| signature(w, it));
  let compact = w.is(node, "compact_constructor_declaration");
  let declared = if compact { header.clone() } else { signature(w, node) };
  if let Some(signature) = declared {
    let canonical = compact || header.is_some_and(|header| header == signature && signature.parameters.iter().all(|it| it.type_ref.is_some()));
    let reach = reach_of(w, w.child(node, Child::Kind(&["modifiers"])), true, false);
    w.add_constructor(Constructor { signature, reach }, canonical);
  }
  if let Some(name) = w.field(node, "name") {
    w.claim(name);
  }
  w.open_scope();
  true
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
    jvm_static: false,
    jvm_field: false,
    jvm_name: false,
    lateinit: false,
    setter: None,
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
  reason = "the signature of `JvmHooks::signature`, whose Kotlin one has none without a parameter list"
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

/// The arguments of a method invocation, in order; Java names none and spreads none (a vararg takes the values)
fn call_shape<'t>(w: &mut Walk<'t>, node: Node<'t>) -> Option<CallArguments> {
  let list = w.field(node, "arguments")?;
  let mut cursor = list.walk();
  let arguments = list.named_children(&mut cursor).filter(|it| !w.is_comment(*it)).map(|it| (None, argument(w, it))).collect();
  Some((arguments, false))
}

/// What an argument is: a string literal or text block, an integer without the `L` suffix (`-1` too), `true` or
/// `false`, a lambda; `null`, a real, a character and `1L` are expressions
fn argument<'t>(w: &Walk<'t>, value: Node<'t>) -> Argument {
  let integer = |it: Node<'t>| {
    ["decimal_integer_literal", "hex_integer_literal", "octal_integer_literal", "binary_integer_literal"]
      .iter()
      .any(|kind| w.is(it, kind))
      && w.text(it).is_some_and(|text| !text.ends_with(['l', 'L']))
  };
  let signed = || w.field(value, "operator").is_some_and(|it| w.is_token(it, "-") || w.is_token(it, "+")) && w.field(value, "operand").is_some_and(integer);
  if w.is(value, "string_literal") {
    Argument::Text
  } else if integer(value) || (w.is(value, "unary_expression") && signed()) {
    Argument::Number
  } else if w.is(value, "true") || w.is(value, "false") {
    Argument::Boolean
  } else if w.is(value, "lambda_expression") {
    Argument::Lambda
  } else {
    Argument::Expression
  }
}
