//! Rust on `tree-sitter-rust`.
//!
//! - **`use` binds**: every name a `use` brings in (`pub use` and `pub(crate) use` included) is a `Binding`
//!   declaration of the file, besides its `Import`.
//! - **`impl` blocks declare nothing themselves**: their members carry the impl's type as `receiver`, and belong to the
//!   declaration of that type when this file has it.
//! - **Macros**: the identifiers in a token tree are `Value` references (`a::b::c` qualified), because the arguments of
//!   `format!`, `vec!` and the like are expressions; locals stay dropped. An attribute's path is a `Macro` reference
//!   unless it is built in, and the names in `derive(…)` are `Type` references; the other attribute arguments are
//!   configuration, not names.
//! - **Patterns**: a capitalized identifier in a pattern is a variant or a constant (`None`, `MAX`), not a binding.

// jscpd:ignore-start: every language module implements one `Spec` with the same vocabulary
use crate::extract::{
  BLOCK_COMMENT, Binder, Child, DeclRule, Holder, IDENTIFIER, LINE_COMMENT, Opens, Rule, SCOPE, Spec, TYPE_IDENTIFIER, Traits, Walk, binder, decl, function_decl, line_of,
  type_decl,
};
use crate::language::Language;
use crate::summary::{DeclKind, MemberKind, Reach, Role};
use tree_sitter::Node;
// jscpd:ignore-end

const TYPE: Rule = type_decl(Child::Field("name"));
const TYPE_ALIAS: Rule = Rule::Decl(DeclRule {
  opens: Opens::Members,
  ..decl(DeclKind::TypeAlias, Child::Field("name"))
});
const FUNCTION: Rule = function_decl(Child::Field("name"));
const PATTERN: Rule = Rule::Bind(binder(false, &[Child::Field("pattern")]));
const LATE_PATTERN: Rule = Rule::Bind(Binder {
  late: true,
  ..binder(false, &[Child::Field("pattern")])
});

/// Attributes of the language and its tools; any other is a procedural macro (or a derive helper) to reference
const BUILTIN_ATTRIBUTES: &[&str] = &[
  "allow",
  "automatically_derived",
  "cfg",
  "cfg_attr",
  "clippy",
  "cold",
  "collapse_debuginfo",
  "coverage",
  "crate_name",
  "crate_type",
  "debugger_visualizer",
  "deny",
  "deprecated",
  "derive",
  "diagnostic",
  "doc",
  "expect",
  "export_name",
  "feature",
  "forbid",
  "global_allocator",
  "ignore",
  "inline",
  "instruction_set",
  "link",
  "link_name",
  "link_section",
  "macro_export",
  "macro_use",
  "must_use",
  "naked",
  "no_implicit_prelude",
  "no_main",
  "no_mangle",
  "no_std",
  "non_exhaustive",
  "optimize",
  "panic_handler",
  "path",
  "proc_macro",
  "proc_macro_attribute",
  "proc_macro_derive",
  "recursion_limit",
  "repr",
  "rustfmt",
  "should_panic",
  "target_feature",
  "test",
  "track_caller",
  "type_length_limit",
  "unsafe",
  "used",
  "warn",
  "windows_subsystem",
];

const RULES: &[(&str, Rule)] = &[
  LINE_COMMENT,
  BLOCK_COMMENT,
  IDENTIFIER,
  TYPE_IDENTIFIER,
  ("field_identifier", Rule::Name(Role::Value)),
  ("shorthand_field_identifier", Rule::Name(Role::Value)),
  ("self", Rule::Keyword("self")),
  ("crate", Rule::Keyword("crate")),
  ("super", Rule::Keyword("super")),
  ("metavariable", Rule::Skip),
  ("label", Rule::Skip),
  ("lifetime", Rule::Skip),
  ("visibility_modifier", Rule::Skip),
  ("self_parameter", Rule::Skip),
  ("token_tree_pattern", Rule::Skip),
  ("inner_attribute_item", Rule::Skip),
  ("string_literal", Rule::Skip),
  ("raw_string_literal", Rule::Skip),
  ("use_declaration", Rule::Special(use_declaration)),
  ("extern_crate_declaration", Rule::Special(extern_crate)),
  ("attribute_item", Rule::Special(attribute_item)),
  ("token_tree", Rule::Special(token_tree)),
  ("impl_item", Rule::Special(impl_item)),
  ("field_initializer", Rule::Special(field)),
  ("field_pattern", Rule::Special(field)),
  ("mod_item", Rule::Decl(decl(DeclKind::Module, Child::Field("name")))),
  ("struct_item", TYPE),
  ("enum_item", TYPE),
  ("union_item", TYPE),
  ("trait_item", TYPE),
  ("type_item", TYPE_ALIAS),
  ("associated_type", TYPE_ALIAS),
  ("function_item", FUNCTION),
  ("function_signature_item", FUNCTION),
  ("const_item", Rule::Decl(decl(DeclKind::Constant, Child::Field("name")))),
  ("static_item", Rule::Decl(decl(DeclKind::Value, Child::Field("name")))),
  ("macro_definition", Rule::Decl(decl(DeclKind::Macro, Child::Field("name")))),
  ("enum_variant", Rule::Decl(decl(DeclKind::EnumEntry, Child::Field("name")))),
  ("field_declaration", Rule::Decl(decl(DeclKind::Value, Child::Field("name")))),
  (
    "scoped_identifier",
    Rule::Member {
      object: Child::Field("path"),
      name: Child::Field("name"),
      role: None,
    },
  ),
  (
    "scoped_type_identifier",
    Rule::Member {
      object: Child::Field("path"),
      name: Child::Field("name"),
      role: None,
    },
  ),
  (
    "field_expression",
    Rule::Member {
      object: Child::Field("value"),
      name: Child::Field("field"),
      role: None,
    },
  ),
  ("generic_type", Rule::Forward(Child::Field("type"))),
  ("generic_type_with_turbofish", Rule::Forward(Child::Field("type"))),
  ("generic_function", Rule::Forward(Child::Field("function"))),
  (
    "call_expression",
    Rule::Call {
      callee: Child::Field("function"),
      role: Role::Call,
    },
  ),
  (
    "macro_invocation",
    Rule::Call {
      callee: Child::Field("macro"),
      role: Role::Macro,
    },
  ),
  ("let_declaration", LATE_PATTERN),
  ("let_condition", LATE_PATTERN),
  ("parameter", PATTERN),
  ("closure_expression", SCOPE),
  ("closure_parameters", Rule::Bind(binder(false, &[Child::Itself]))),
  ("block", SCOPE),
  ("if_expression", SCOPE),
  ("while_expression", SCOPE),
  ("match_arm", SCOPE),
  // the pattern binds before the guard
  ("match_pattern", Rule::Bind(binder(false, &[Child::Nth(0)]))),
  ("for_expression", Rule::Bind(binder(true, &[Child::Field("pattern")]))),
  ("tuple_struct_pattern", Rule::Unbind(&[Child::Field("type")])),
  ("struct_pattern", Rule::Unbind(&[Child::Field("type")])),
  ("type_parameter", Rule::Bind(binder(false, &[Child::Field("name")]))),
  ("const_parameter", Rule::Bind(binder(false, &[Child::Field("name")]))),
];

pub(crate) static SPEC: Spec = Spec {
  language: Language::Rust,
  grammar: || tree_sitter_rust::LANGUAGE.into(),
  node_types: tree_sitter_rust::NODE_TYPES,
  rules: RULES,
  binding_names: &["identifier", "shorthand_field_identifier"],
  reserved: &["Self"],
  kinds: &[
    "use_list",
    "scoped_use_list",
    "use_wildcard",
    "use_as_clause",
    "attribute",
    "parameters",
    "struct_expression",
  ],
  tokens: &["::"],
  fields: &["argument", "path", "list", "alias", "name", "arguments", "type", "trait", "field", "parameters"],
  traits,
  binds: |name| !name.starts_with(|it: char| it.is_ascii_uppercase()),
  hoist: None,
};

fn extern_crate<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(name) = w.field(node, "name") {
    let alias = w.field(node, "alias");
    let reach = reach(w, node);
    use_name(w, node, &[], name, alias, reach);
  }
  false
}

/// Identifiers in a token tree are qualified by the `a::b::` tokens before them
fn token_tree<'t>(w: &mut Walk<'t>, _: Node<'t>) -> bool {
  w.start_tokens();
  true
}

/// An `impl` declares nothing; its members name its type as their receiver
fn impl_item<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let receiver = w.field(node, "type").and_then(|it| type_name(w, it)).map(|it| w.intern(it));
  let holder = if w.field(node, "trait").is_some() { Holder::TraitImpl } else { Holder::Impl };
  if !w.is_local() {
    w.enter_container(None, holder, receiver);
  }
  w.open_members_scope();
  true
}

/// `S { field: value }` and `let S { field: pattern } = …`: the field is a member of the struct, never a local
fn field<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let name = if w.is(node, "field_pattern") { w.field(node, "name") } else { w.field(node, "field") };
  let Some(name) = name.filter(|it| w.is(*it, "field_identifier")) else { return true };
  let structure = if w.is(node, "field_pattern") {
    w.ancestor(1).filter(|it| w.is(*it, "struct_pattern")).and_then(|it| w.field(it, "type"))
  } else {
    w.ancestor(2).filter(|it| w.is(*it, "struct_expression")).and_then(|it| w.field(it, "name"))
  };
  let qualifier = structure.map_or_else(Vec::new, |it| w.qualifier_of(it));
  w.emit(name, Role::Value, Some(qualifier));
  true
}

/// `use a::{b, c::d as e, f::*}`, walked with an explicit stack of (tree, path before it)
fn use_declaration<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let Some(argument) = w.field(node, "argument") else { return false };
  let reach = reach(w, node);
  let mut stack: Vec<(Node<'t>, Vec<&'t str>)> = vec![(argument, Vec::new())];
  while let Some((item, prefix)) = stack.pop() {
    if w.is(item, "use_list") {
      let mut cursor = item.walk();
      let start = stack.len();
      stack.extend(item.named_children(&mut cursor).map(|it| (it, prefix.clone())));
      stack[start..].reverse();
    } else if w.is(item, "scoped_use_list") {
      let mut prefix = prefix;
      if let Some(path) = w.field(item, "path").and_then(|it| w.path_of(it)) {
        prefix.extend(path);
      }
      if let Some(list) = w.field(item, "list") {
        stack.push((list, prefix));
      }
    } else if w.is(item, "use_wildcard") {
      let mut path = prefix;
      if let Some(segments) = item.named_child(0).and_then(|it| w.path_of(it)) {
        path.extend(segments);
      }
      w.import_all(&path, is_exported(reach), line_of(item));
    } else if w.is(item, "use_as_clause") {
      if let Some(path) = w.field(item, "path") {
        let alias = w.field(item, "alias");
        use_name(w, node, &prefix, path, alias, reach);
      }
    } else {
      use_name(w, node, &prefix, item, None, reach);
    }
  }
  false
}

/// `pub use` passes names on, and so do `pub(crate) use` and `pub(super) use`
const fn is_exported(reach: Reach) -> bool {
  matches!(reach, Reach::Public | Reach::Module | Reach::Parent)
}

/// One name a `use` or `extern crate` brings in: its import, its reference and its binding
fn use_name<'t>(w: &mut Walk<'t>, node: Node<'t>, prefix: &[&'t str], path: Node<'t>, alias: Option<Node<'t>>, reach: Reach) {
  let Some(mut segments) = w.path_of(path) else { return };
  segments.splice(0..0, prefix.iter().copied());
  // `use a::b::{self}` brings in `b`
  if segments.len() > 1 && segments.last() == Some(&"self") {
    segments.pop();
  }
  let Some((name, before)) = segments.split_last() else { return };
  let name_node = w.field(path, "name").unwrap_or(path);
  let line = line_of(name_node);
  let alias_text = alias.and_then(|it| w.text(it));
  w.import_name(before, name, alias_text, is_exported(reach), line, line);
  let bound = alias_text.unwrap_or(name);
  if bound != "_" && !["self", "crate", "super"].contains(&bound) {
    w.add_binding(node, alias.unwrap_or(name_node), bound, reach);
  }
}

/// `#[…]` extends the item below it; its path is a macro unless built in, `derive(…)` names traits
fn attribute_item<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  w.extend_leading(node);
  let Some(attribute) = w.child(node, Child::Kind(&["attribute"])) else { return false };
  let Some(path_node) = attribute.named_child(0) else { return false };
  let Some(path) = w.path_of(path_node) else { return false };
  let Some((name, before)) = path.split_last() else { return false };
  if *name == "derive" && before.is_empty() {
    if let Some(arguments) = w.field(attribute, "arguments") {
      derives(w, arguments);
    }
  } else if !before.is_empty() || !BUILTIN_ATTRIBUTES.contains(name) {
    let qualifier = (!before.is_empty()).then(|| w.ids(before));
    w.emit_text(name, line_of(path_node), Role::Macro, qualifier);
  }
  false
}

/// The traits of `derive(Debug, serde::Serialize)`
fn derives<'t>(w: &mut Walk<'t>, arguments: Node<'t>) {
  let mut path: Vec<&'t str> = Vec::new();
  let mut cursor = arguments.walk();
  let children: Vec<Node<'t>> = arguments.children(&mut cursor).collect();
  for child in children {
    if w.is(child, "identifier") {
      let Some(text) = w.text(child) else { continue };
      let qualifier = (!path.is_empty()).then(|| w.ids(&path));
      w.emit_text(text, line_of(child), Role::Type, qualifier);
      path.push(text);
    } else if !w.is_token(child, "::") {
      path.clear();
    }
  }
}

/// The last name of an impl's type: `Foo` of `a::Foo<T>`
fn type_name<'t>(w: &Walk<'t>, node: Node<'t>) -> Option<&'t str> {
  let mut node = node;
  loop {
    node = if w.is(node, "type_identifier") {
      return w.text(node);
    } else if w.is(node, "generic_type") {
      w.field(node, "type")?
    } else if w.is(node, "scoped_type_identifier") {
      w.field(node, "name")?
    } else {
      return None;
    };
  }
}

fn reach<'t>(w: &Walk<'t>, node: Node<'t>) -> Reach {
  let Some(visibility) = w.child(node, Child::Kind(&["visibility_modifier"])) else {
    return if w.has_holder(Holder::Module) { Reach::Container } else { Reach::File };
  };
  let text = w.text(visibility).unwrap_or("pub");
  if text == "pub" {
    Reach::Public
  } else if text.contains("super") {
    Reach::Parent
  } else if text.contains("self") {
    if w.has_holder(Holder::Module) { Reach::Container } else { Reach::File }
  } else {
    Reach::Module
  }
}

#[expect(clippy::unnecessary_wraps, reason = "the signature of `Spec::traits`, whose TypeScript one declines declarations")]
fn traits<'t>(w: &mut Walk<'t>, node: Node<'t>, rule: &DeclRule) -> Option<Traits> {
  let holder = w.holder();
  let own = match node.kind() {
    "struct_item" | "union_item" | "enum_variant" => Holder::Class,
    "enum_item" => Holder::Enum,
    "trait_item" => Holder::Trait,
    "mod_item" => Holder::Module,
    _ => Holder::Leaf,
  };
  let has_self = w.field(node, "parameters").is_some_and(|parameters| {
    let mut cursor = parameters.walk();
    parameters.named_children(&mut cursor).any(|it| w.is(it, "self_parameter"))
  });
  let member = match holder {
    None | Some(Holder::Module) => MemberKind::TopLevel,
    Some(Holder::Class) => MemberKind::Instance,
    Some(Holder::Impl | Holder::TraitImpl | Holder::Trait) if has_self => MemberKind::Instance,
    Some(_) => MemberKind::Static,
  };
  // a variant, a trait item and an item of a trait impl are as visible as their trait or type
  let in_variant = w.ancestor(2).is_some_and(|it| w.is(it, "enum_variant"));
  let reach = match holder {
    Some(Holder::Enum | Holder::Trait | Holder::TraitImpl) => Reach::Public,
    Some(Holder::Class) if in_variant => Reach::Public,
    _ => reach(w, node),
  };
  let receiver = if matches!(holder, Some(Holder::Impl | Holder::TraitImpl)) {
    w.container_receiver()
  } else {
    None
  };
  Some(Traits {
    kind: rule.kind,
    reach,
    member,
    receiver,
    holder: own,
  })
}
