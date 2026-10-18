//! TypeScript and JavaScript on the TSX grammar of `tree-sitter-typescript`, a superset of both with JSX.
//!
//! - **Hoisting**: `let`, `const`, `class` and `function` are visible in their whole block and `var` in its whole
//!   function, so a closure may use a local declared below it; `hoist` binds them when the scope opens.
//! - **Exports**: `export` makes a top-level declaration public, and so does `export { name }` below it. A re-export
//!   (`export … from`) is an `Import` with `exported`, and its names are `Binding` declarations.
//! - **JSX**: a capitalized element name is a `Type` reference; a lowercase one is an HTML tag.

// jscpd:ignore-start: every language module implements one `Spec` with the same vocabulary
use crate::extract::{
  Child, DeclRule, Header, Holder, IDENTIFIER, JvmFields, Opens, Rule, SCOPE, Spec, TYPE_IDENTIFIER, Traits, Walk, binder, decl, function_decl, line_of, type_decl,
};
use crate::language::Language;
use crate::summary::{DeclKind, Import, ImportTarget, MemberKind, Reach, Role};
use tree_sitter::Node;
// jscpd:ignore-end

const FUNCTION: Rule = function_decl(Child::Field("name"));
const TYPE: Rule = type_decl(Child::Field("name"));
const FUNCTION_EXPRESSION: Rule = Rule::Bind(binder(true, &[Child::Field("name")]));

/// Kinds whose body is a function scope, for `var`
const FUNCTIONS: &[&str] = &[
  "function_declaration",
  "generator_function_declaration",
  "function_expression",
  "generator_function",
  "arrow_function",
  "method_definition",
];
/// Statements whose names are visible in their whole block
const BLOCK_DECLARATIONS: &[&str] = &[
  "function_declaration",
  "generator_function_declaration",
  "class_declaration",
  "abstract_class_declaration",
  "enum_declaration",
];

const RULES: &[(&str, Rule)] = &[
  ("comment", Rule::Comment),
  ("html_comment", Rule::Skip),
  ("hash_bang_line", Rule::Skip),
  ("regex", Rule::Skip),
  ("statement_identifier", Rule::Skip),
  IDENTIFIER,
  TYPE_IDENTIFIER,
  ("shorthand_property_identifier", Rule::Name(Role::Value)),
  ("shorthand_property_identifier_pattern", Rule::Name(Role::Value)),
  ("this", Rule::Keyword("this")),
  ("super", Rule::Keyword("super")),
  ("import_statement", Rule::Special(import)),
  ("export_statement", Rule::Special(export)),
  ("function_declaration", FUNCTION),
  ("generator_function_declaration", FUNCTION),
  ("function_signature", FUNCTION),
  ("method_definition", FUNCTION),
  ("method_signature", FUNCTION),
  ("abstract_method_signature", FUNCTION),
  ("class_declaration", TYPE),
  ("abstract_class_declaration", TYPE),
  ("interface_declaration", TYPE),
  ("enum_declaration", Rule::Decl(decl(DeclKind::Type, Child::Field("name")))),
  (
    "type_alias_declaration",
    Rule::Decl(DeclRule {
      opens: Opens::Members,
      ..decl(DeclKind::TypeAlias, Child::Field("name"))
    }),
  ),
  ("internal_module", Rule::Decl(decl(DeclKind::Module, Child::Field("name")))),
  ("module", Rule::Decl(decl(DeclKind::Module, Child::Field("name")))),
  ("public_field_definition", Rule::Decl(decl(DeclKind::Value, Child::Field("name")))),
  ("property_signature", Rule::Decl(decl(DeclKind::Value, Child::Field("name")))),
  (
    "variable_declarator",
    Rule::Decl(DeclRule {
      pattern: Some(Child::Field("name")),
      ..decl(DeclKind::Value, Child::Field("name"))
    }),
  ),
  ("lexical_declaration", Rule::Wrapper),
  ("variable_declaration", Rule::Wrapper),
  ("class", SCOPE),
  ("function_expression", FUNCTION_EXPRESSION),
  ("generator_function", FUNCTION_EXPRESSION),
  ("arrow_function", Rule::Bind(binder(true, &[Child::Field("parameter")]))),
  ("required_parameter", Rule::Special(parameter)),
  ("optional_parameter", Rule::Special(parameter)),
  ("catch_clause", Rule::Bind(binder(true, &[Child::Field("parameter")]))),
  ("for_in_statement", Rule::Special(for_in)),
  ("for_statement", SCOPE),
  ("statement_block", Rule::Special(statement_block)),
  ("switch_body", SCOPE),
  ("class_static_block", SCOPE),
  (
    "member_expression",
    Rule::Member {
      object: Child::Field("object"),
      name: Child::Field("property"),
      role: None,
    },
  ),
  (
    "nested_identifier",
    Rule::Member {
      object: Child::Field("object"),
      name: Child::Field("property"),
      role: None,
    },
  ),
  (
    "nested_type_identifier",
    Rule::Member {
      object: Child::Field("module"),
      name: Child::Field("name"),
      role: None,
    },
  ),
  (
    "call_expression",
    Rule::Call {
      callee: Child::Field("function"),
      role: Role::Call,
    },
  ),
  (
    "new_expression",
    Rule::Call {
      callee: Child::Field("constructor"),
      role: Role::Call,
    },
  ),
  ("instantiation_expression", Rule::Forward(Child::Field("function"))),
  ("generic_type", Rule::Forward(Child::Field("name"))),
  ("assignment_pattern", Rule::Unbind(&[Child::Field("right")])),
  ("object_assignment_pattern", Rule::Unbind(&[Child::Field("right")])),
  ("jsx_opening_element", Rule::Special(jsx_element)),
  ("jsx_closing_element", Rule::Special(jsx_element)),
  ("jsx_self_closing_element", Rule::Special(jsx_element)),
  ("enum_body", Rule::Special(enum_body)),
  ("type_parameter", Rule::Bind(binder(false, &[Child::Field("name")]))),
  ("mapped_type_clause", Rule::Bind(binder(false, &[Child::Field("name")]))),
  ("infer_type", Rule::Bind(binder(false, &[Child::Kind(&["type_identifier"])]))),
];

pub(crate) static SPEC: Spec = Spec {
  language: Language::TypeScript,
  grammar: || tree_sitter_typescript::LANGUAGE_TSX.into(),
  node_types: tree_sitter_typescript::TSX_NODE_TYPES,
  rules: RULES,
  binding_names: &["identifier", "shorthand_property_identifier_pattern"],
  reserved: &[],
  kinds: &[
    "class_body",
    "interface_body",
    "object_type",
    "accessibility_modifier",
    "private_property_identifier",
    "property_identifier",
    "import_clause",
    "named_imports",
    "import_specifier",
    "namespace_import",
    "import_require_clause",
    "export_clause",
    "export_specifier",
    "namespace_export",
    "enum_assignment",
    "string",
    "string_fragment",
  ],
  tokens: &["static", "get", "set", "readonly", "const"],
  fields: &["name", "alias", "source", "value", "body", "kind", "left", "pattern"],
  traits,
  binds: |_| true,
  hoist: Some(hoist),
  jvm: None,
  call_shape: None,
  coroutine: None,
  header: Header {
    ends: &["statement_block"],
    end_tokens: &["="],
    drops: &["decorator", "accessibility_modifier", "override_modifier"],
    drop_tokens: &[";"],
    parameters: &["formal_parameters"],
  },
};

/// A namespace body holds members, any other block is a scope
fn statement_block<'t>(w: &mut Walk<'t>, _: Node<'t>) -> bool {
  if !w.ancestor(1).is_some_and(|it| w.is(it, "internal_module") || w.is(it, "module")) {
    w.open_scope();
  }
  true
}

/// `for (const x of xs)` binds `x`; `for (x of xs)` assigns to an outer one
fn for_in<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  w.open_scope();
  if let (Some(_), Some(left)) = (w.field(node, "kind"), w.field(node, "left")) {
    w.mark_pattern(left);
  }
  true
}

/// The name of a JSX tag: a component when capitalized, else an HTML tag
fn jsx_element<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  if let Some(name) = w.field(node, "name") {
    if !w.is(name, "identifier") {
      w.give_role(name, Role::Type);
    } else if w.text(name).is_some_and(|it| it.starts_with(|c: char| c.is_ascii_uppercase())) {
      w.emit(name, Role::Type, None);
    } else {
      w.claim(name);
    }
  }
  true
}

fn module_specifier<'t>(w: &Walk<'t>, node: Node<'t>) -> Option<String> {
  let source = w.field(node, "source")?;
  let fragment = w.child(source, Child::Kind(&["string_fragment"]));
  Some(fragment.and_then(|it| w.text(it)).unwrap_or_default().to_string())
}

fn import<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let specifier = module_specifier(w, node);
  let mut cursor = node.walk();
  let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
  for child in children {
    if w.is(child, "import_clause") {
      let mut cursor = child.walk();
      let parts: Vec<Node<'t>> = child.named_children(&mut cursor).collect();
      for part in parts {
        if w.is(part, "identifier") {
          bound_import(w, node, ImportTarget::Default, part, specifier.clone(), false);
        } else if w.is(part, "namespace_import") {
          if let Some(alias) = part.named_child(0) {
            bound_import(w, node, ImportTarget::Namespace, alias, specifier.clone(), false);
          }
        } else if w.is(part, "named_imports") {
          specifiers(w, node, part, specifier.as_deref(), false);
        }
      }
    } else if w.is(child, "import_require_clause")
      && let Some(alias) = child.named_child(0)
    {
      let specifier = module_specifier(w, child);
      bound_import(w, node, ImportTarget::Namespace, alias, specifier, false);
    }
  }
  false
}

/// A default or namespace import: the module under a local name
fn bound_import<'t>(w: &mut Walk<'t>, node: Node<'t>, target: ImportTarget, alias: Node<'t>, specifier: Option<String>, exported: bool) {
  let Some(text) = w.text(alias) else { return };
  let alias_id = w.intern(text);
  w.add_import(Import {
    path: Vec::new(),
    target,
    alias: Some(alias_id),
    exported,
    specifier,
    line: line_of(alias),
  });
  w.add_binding(node, alias, text, if exported { Reach::Public } else { Reach::File });
}

/// `{ a, b as c }` of an import or a re-export
fn specifiers<'t>(w: &mut Walk<'t>, node: Node<'t>, list: Node<'t>, specifier: Option<&str>, exported: bool) {
  let mut cursor = list.walk();
  let items: Vec<Node<'t>> = list
    .named_children(&mut cursor)
    .filter(|it| w.is(*it, "import_specifier") || w.is(*it, "export_specifier"))
    .collect();
  for item in items {
    let Some(name) = w.field(item, "name") else { continue };
    let Some(text) = name_text(w, name) else { continue };
    let alias = w.field(item, "alias");
    let alias_text = alias.and_then(|it| name_text(w, it));
    let target = w.intern(text);
    let alias_id = alias_text.map(|it| w.intern(it));
    w.add_import(Import {
      path: Vec::new(),
      target: ImportTarget::Name(target),
      alias: alias_id,
      exported,
      specifier: specifier.map(ToString::to_string),
      line: line_of(name),
    });
    w.emit_text(text, line_of(name), Role::Import, None);
    let bound = alias.zip(alias_text).unwrap_or((name, text));
    w.add_binding(node, bound.0, bound.1, if exported { Reach::Public } else { Reach::File });
  }
}

/// An identifier, or the string of `export { "a-b" as c }`
fn name_text<'t>(w: &Walk<'t>, node: Node<'t>) -> Option<&'t str> {
  if w.is(node, "string") {
    w.child(node, Child::Kind(&["string_fragment"])).and_then(|it| w.text(it))
  } else {
    w.text(node)
  }
}

fn export<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let clause = w.child(node, Child::Kind(&["export_clause"]));
  if w.field(node, "source").is_some() {
    let specifier = module_specifier(w, node);
    if let Some(clause) = clause {
      specifiers(w, node, clause, specifier.as_deref(), true);
    } else if let Some(namespace) = w.child(node, Child::Kind(&["namespace_export"])) {
      if let Some(alias) = namespace.named_child(0) {
        bound_import(w, node, ImportTarget::Namespace, alias, specifier, true);
      }
    } else {
      w.add_import(Import {
        path: Vec::new(),
        target: ImportTarget::All,
        alias: None,
        exported: true,
        specifier,
        line: line_of(node),
      });
    }
    return false;
  }
  if let Some(clause) = clause {
    // `export { a, b as c }` publishes local names
    let mut cursor = clause.walk();
    let items: Vec<Node<'t>> = clause.named_children(&mut cursor).collect();
    for item in items {
      let (Some(name), alias) = (w.field(item, "name"), w.field(item, "alias")) else {
        continue;
      };
      w.emit(name, Role::Value, None);
      match (alias, alias.and_then(|it| name_text(w, it))) {
        (Some(alias), Some(text)) => w.add_binding(node, alias, text, Reach::Public),
        _ => {
          if let Some(text) = w.text(name) {
            w.export_name(text);
          }
        }
      }
    }
    return false;
  }
  w.pass_leading(node);
  true
}

/// A parameter; `constructor(private readonly x: X)` also declares the field `x`
fn parameter<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let Some(pattern) = w.field(node, "pattern") else { return true };
  let property = w.child(node, Child::Kind(&["accessibility_modifier"])).is_some() || w.has_token(node, "readonly");
  if property && w.is(pattern, "identifier") && w.holder() == Some(Holder::Class) {
    let private = w.child(node, Child::Kind(&["accessibility_modifier"])).and_then(|it| w.text(it)) == Some("private");
    let reach = if private { Reach::Container } else { Reach::Public };
    let traits = Traits {
      kind: DeclKind::Value,
      reach,
      member: MemberKind::Instance,
      receiver: None,
      holder: Holder::Leaf,
    };
    w.add_declaration(node, pattern, &traits, JvmFields::default());
    w.bind_node(pattern);
  } else {
    w.mark_pattern(pattern);
  }
  true
}

fn enum_body<'t>(w: &mut Walk<'t>, node: Node<'t>) -> bool {
  let local = w.is_local();
  let traits = Traits {
    kind: DeclKind::EnumEntry,
    reach: Reach::Public,
    member: MemberKind::Static,
    receiver: None,
    holder: Holder::Leaf,
  };
  let mut cursor = node.walk();
  let members: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
  for member in members {
    let name = if w.is(member, "enum_assignment") { w.field(member, "name") } else { Some(member) };
    if let Some(name) = name.filter(|it| w.is(*it, "property_identifier")) {
      if local {
        w.claim(name);
      } else {
        w.add_declaration(member, name, &traits, JvmFields::default());
      }
    }
  }
  true
}

/// Binds what is visible in the whole scope that opens at `node`
fn hoist<'t>(w: &mut Walk<'t>, node: Node<'t>) {
  if FUNCTIONS.contains(&node.kind()) {
    if let Some(body) = w.field(node, "body") {
      hoist_vars(w, body);
    }
    return;
  }
  let mut cursor = node.walk();
  let statements: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
  for statement in statements {
    if w.is(statement, "lexical_declaration") {
      bind_declarators(w, statement);
    } else if BLOCK_DECLARATIONS.contains(&statement.kind())
      && let Some(name) = w.field(statement, "name").and_then(|it| w.text(it))
    {
      w.bind_text(name);
    }
  }
}

/// `var` anywhere in a function body, but not in the functions inside it
fn hoist_vars<'t>(w: &mut Walk<'t>, body: Node<'t>) {
  let mut stack = vec![body];
  while let Some(node) = stack.pop() {
    if w.is(node, "variable_declaration") {
      bind_declarators(w, node);
      continue;
    }
    if node.id() != body.id() && (FUNCTIONS.contains(&node.kind()) || w.is(node, "class")) {
      continue;
    }
    let mut cursor = node.walk();
    stack.extend(node.named_children(&mut cursor));
  }
}

fn bind_declarators<'t>(w: &mut Walk<'t>, statement: Node<'t>) {
  let mut cursor = statement.walk();
  let declarators: Vec<Node<'t>> = statement.named_children(&mut cursor).filter(|it| w.is(*it, "variable_declarator")).collect();
  for declarator in declarators {
    if let Some(name) = w.field(declarator, "name") {
      for bound in w.pattern_names(name) {
        if let Some(text) = w.text(bound) {
          w.bind_text(text);
        }
      }
    }
  }
}

fn traits<'t>(w: &mut Walk<'t>, node: Node<'t>, rule: &DeclRule) -> Option<Traits> {
  let parent = w.ancestor(1)?;
  let holder = w.holder();
  let in_class = w.is(parent, "class_body");
  let in_interface = w.is(parent, "interface_body") || (w.is(parent, "object_type") && holder == Some(Holder::Alias));
  let mut kind = rule.kind;
  match node.kind() {
    // an object literal's method, a constructor, a type literal's member: no declarations
    "method_definition" if !in_class => return None,
    "method_definition" if w.field(node, "name").and_then(|it| w.text(it)) == Some("constructor") => return None,
    "method_definition" if w.has_token(node, "get") || w.has_token(node, "set") => kind = DeclKind::Value,
    "method_signature" | "property_signature" if !in_interface && !in_class => return None,
    "variable_declarator" => {
      if !matches!(holder, None | Some(Holder::Module)) {
        return None;
      }
      kind = declarator_kind(w, node, parent);
    }
    _ => {}
  }
  let own = match node.kind() {
    "class_declaration" | "abstract_class_declaration" => Holder::Class,
    "interface_declaration" => Holder::Interface,
    "enum_declaration" => Holder::Enum,
    "type_alias_declaration" => Holder::Alias,
    "internal_module" | "module" => Holder::Module,
    _ => Holder::Leaf,
  };
  let statement = if w.is(node, "variable_declarator") { w.ancestor(2) } else { Some(parent) };
  let exported = statement.is_some_and(|it| w.is(it, "export_statement"));
  let (member, reach) = match holder {
    None => (MemberKind::TopLevel, if exported { Reach::Public } else { Reach::File }),
    Some(Holder::Module) => (MemberKind::TopLevel, if exported { Reach::Public } else { Reach::Container }),
    Some(Holder::Class) => {
      let member = if w.has_token(node, "static") { MemberKind::Static } else { MemberKind::Instance };
      let private = w.child(node, Child::Kind(&["accessibility_modifier"])).and_then(|it| w.text(it)) == Some("private")
        || w.field(node, "name").is_some_and(|it| w.is(it, "private_property_identifier"));
      (member, if private { Reach::Container } else { Reach::Public })
    }
    Some(_) => (MemberKind::Instance, Reach::Public),
  };
  Some(Traits {
    kind,
    reach,
    member,
    receiver: None,
    holder: own,
  })
}

/// `const f = () => …` is a function, `const MAX_SIZE = 10` a constant
fn declarator_kind<'t>(w: &Walk<'t>, node: Node<'t>, statement: Node<'t>) -> DeclKind {
  let value = w.field(node, "value");
  if value.is_some_and(|it| ["arrow_function", "function_expression", "generator_function"].iter().any(|kind| w.is(it, kind))) {
    return DeclKind::Function;
  }
  let constant = w.is(statement, "lexical_declaration")
    && w.has_token(statement, "const")
    && w
      .field(node, "name")
      .and_then(|it| w.text(it))
      .is_some_and(|name| name.chars().any(|it| it.is_ascii_uppercase()) && name.chars().all(|it| it.is_ascii_uppercase() || it.is_ascii_digit() || it == '_'));
  if constant { DeclKind::Constant } else { DeclKind::Value }
}
