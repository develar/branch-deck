//! The summary of a Java file from its PSI (`FileSummary`), by the rules that the head of `extract.rs` states for
//! every language (locals dropped) and those of the JVM languages (`jvm`: overrides by the stack of supertypes,
//! `Shape::Call` of a call, the constructors of a class, `enclosed`). One scan over the preorder ids of the tree (`Walk::run`) with an explicit stack of frames, so a
//! file nested as deep as the parser allows takes no stack per level.
//!
//! - **Declarations**: a `CLASS` (a class, interface, enum, record or `@interface` by its keyword), a `METHOD` with a
//!   return type, an `ANNOTATION_METHOD`, a `FIELD` (a later one of `int a, b;` takes the modifiers and the type of its
//!   head, the `FIELD` that has them), an `ENUM_CONSTANT` and a `RECORD_COMPONENT` (a field). A `METHOD` without a
//!   return type is a constructor, one of the `constructors` of its class, without a parameter list the compact
//!   canonical one of a record. Inside a body (a `CODE_BLOCK`, a lambda, an anonymous class, an enum constant's body) a
//!   declaration is a local: its name is bound, a `LOCAL_VARIABLE` and a `RESOURCE_VARIABLE` once left.
//! - **References**: the name of a `REFERENCE_EXPRESSION` (a value, the callee of a `METHOD_CALL_EXPRESSION`) and of a
//!   `JAVA_CODE_REFERENCE` (a type; the callee of a `new`; an annotation), qualified by the path of its qualifier, and
//!   every qualifier of a path a reference of its own. The segments before the name of a type are types, those of an
//!   annotation's name and of `Outer.this` values. A method reference `Type::name` names `name` qualified by `Type`.
//! - **Not read**: doc comments (their `{@link}` references too), the receiver parameter `Outer this`, the labels of
//!   `break` and `continue`, a module declaration, the element names of an annotation's arguments.
//! - **As the tree-sitter walk before it had them**: a local of a `case` group is bound until the next group; the loop variable of
//!   a `for` and the parameter of a `catch` have no type.

use crate::jvm::{Holder, JvmFields, TypedReceiver, override_shape, smart_cast, type_name};
use crate::psi_walk::{Span, WalkCore, Walker, has_error_in};
use crate::summary::{Argument, Constructor, DeclKind, FileSummary, LineSpan, MemberKind, Parameter, Reach, Role, Shape, Signature, TypeRef};
use crate::summary_builder::{TEST_ANNOTATIONS, Typed, column_at};
use java_syntax::psi;
use java_syntax::psi::modifier::Modifier;
use java_syntax::{NodeId, NodeKind as N, TokenKind as T, Tree};

/// Fills `summary` (`summary_builder::empty_summary` of the file, its size within the limits) with the facts of the
/// Java file `source`; whether it was read, else it stays `truncated`
pub(crate) fn summarize(source: &[u8], summary: &mut FileSummary) -> bool {
  let tree = super::parse(source);
  let mut walk = Walk::new(&tree);
  crate::psi_walk::run(&mut walk);
  // javac gives the default constructor the access of its class (JLS 8.8.9)
  walk.core.out.finish(summary, Some(|reach| reach));
  true
}

/// What a parent decided about a reference node below it before the walk reaches it
#[derive(Debug, Clone, Copy)]
enum Mark {
  /// The callee of a call or a `new` (`Role::Call`, the shape of its arguments) or an annotation's name (`Role::Type`);
  /// the segments before the name are of `chain`
  Role { role: Role, shape: Option<u32>, chain: Role },
  /// A path of values: an annotation's qualifier, the qualifier of `Outer.this`
  Values,
  /// A body whose declarations are locals (an enum constant's)
  Scope,
}

/// What the walk keeps of a node (`psi_walk::Frame::extra`)
#[derive(Default)]
struct JavaFrame {
  error: bool,
  /// A switch's body, whose `case` groups are scopes: whether one is open
  switch_group: Option<bool>,
  /// A field, the container of what follows its name: not of its modifiers and type, which its declaration shares
  deferred: Option<u32>,
}

struct Walk<'t> {
  tree: &'t Tree,
  /// The text, the builder, the scopes, the frames; its containers and supertypes: a class pushes its own supertypes, a
  /// method an empty list, an anonymous class the type it constructs, an enum constant its enum
  core: WalkCore<'t, java_syntax::Java, JavaFrame, Mark>,
  error_depth: u32,
  /// The error elements of the code (not of a doc comment), in document order
  errors: Vec<u32>,
  /// The `FIELD` or `LOCAL_VARIABLE` with a type met last: the head of the later ones of its declaration
  last_head: Option<NodeId>,
  /// The class `in_final_class` answered last, and whether it is `final`
  final_class: Option<(NodeId, bool)>,
}

impl<'t> Walker<'t> for Walk<'t> {
  type Lang = java_syntax::Java;
  type Extra = JavaFrame;
  type Mark = Mark;

  fn core(&mut self) -> &mut WalkCore<'t, java_syntax::Java, JavaFrame, Mark> {
    &mut self.core
  }

  fn enter(&mut self, id: NodeId, mark: Option<Mark>) -> bool {
    self.enter_node(id, mark)
  }

  fn token(&mut self, id: NodeId) {
    self.leaf(id);
  }

  fn left(&mut self, extra: JavaFrame) {
    if extra.error {
      self.error_depth -= 1;
    }
  }
}

impl<'t> Walk<'t> {
  fn new(tree: &'t Tree) -> Self {
    let mut walk = Self {
      tree,
      core: WalkCore::new(tree),
      error_depth: 0,
      errors: Vec::new(),
      last_head: None,
      final_class: None,
    };
    walk.errors = tree.errors().map(|(id, _)| id).filter(|id| !tree.in_doc_comment(*id)).map(|id| id.0).collect();
    walk
  }

  /// A leaf: a name in an error element is a value, the `{` of an anonymous class opens its body, the name of a field
  /// makes it the container
  fn leaf(&mut self, id: NodeId) {
    match self.tree.token_kind(id) {
      Some(T::IDENTIFIER) if self.core.frames.last().is_some_and(|it| it.extra.deferred.is_some()) => {
        if let Some(decl) = self.core.frame().extra.deferred.take() {
          self.core.enter_container(decl, Holder::Leaf);
        }
      }
      Some(T::IDENTIFIER) if self.parent_kind(id) == Some(N::ERROR_ELEMENT) => {
        if let Some(text) = self.text(id) {
          let line = self.core.line_at(id);
          self.reference(text, line, Role::Value, None, None);
        }
      }
      Some(T::LBRACE) if self.parent_kind(id) == Some(N::ANONYMOUS_CLASS) => {
        self.core.bindings.push(true);
        // the frame of the anonymous class, which the `{` has none of its own
        self.core.frame().scopes += 1;
      }
      _ => {}
    }
  }

  fn enter_node(&mut self, id: NodeId, mark: Option<Mark>) -> bool {
    // a composite of a token type: a doc comment
    let Some(kind) = self.kind(id) else { return false };
    if matches!(mark, Some(Mark::Scope)) {
      self.core.push_scope(true);
    }
    match kind {
      N::ERROR_ELEMENT => {
        if self.error_depth == 0 {
          let bytes = u32::try_from(self.tree.range(id).len()).unwrap_or(u32::MAX).max(1);
          self.core.out.error_bytes = self.core.out.error_bytes.saturating_add(bytes);
        }
        self.error_depth += 1;
        self.core.frame().extra.error = true;
        true
      }
      N::PACKAGE_STATEMENT => {
        self.import_statement(id);
        if let Some(path) = psi::package_statement::get_package_reference(self.tree, id).and_then(|it| self.path_of(it)) {
          self.core.out.package = Some(path.join("."));
        }
        false
      }
      N::IMPORT_STATEMENT | N::IMPORT_STATIC_STATEMENT => {
        self.import(id);
        false
      }
      N::IMPORT_MODULE_STATEMENT => {
        self.import_statement(id);
        false
      }
      N::MODULE | N::RECEIVER_PARAMETER | N::BREAK_STATEMENT | N::CONTINUE_STATEMENT => false,
      N::CLASS => {
        self.class(id);
        true
      }
      N::METHOD if psi::method::is_constructor(self.tree, id) => {
        self.constructor(id);
        true
      }
      N::METHOD | N::ANNOTATION_METHOD => {
        self.method(id, kind == N::ANNOTATION_METHOD);
        true
      }
      N::FIELD | N::LOCAL_VARIABLE => {
        self.variable(id);
        true
      }
      N::ENUM_CONSTANT => {
        self.enum_constant(id);
        true
      }
      N::RECORD_COMPONENT => {
        self.record_component(id);
        true
      }
      N::PARAMETER => {
        self.parameter(id);
        true
      }
      N::TYPE_PARAMETER | N::PATTERN_VARIABLE | N::DECONSTRUCTION_PATTERN_VARIABLE => {
        let name = psi::get_name_identifier(self.tree, id);
        self.bind_name(name, None, false);
        true
      }
      N::RESOURCE_VARIABLE => {
        let name = psi::get_name_identifier(self.tree, id);
        self.bind_name(name, None, true);
        true
      }
      N::CODE_BLOCK => {
        if matches!(self.parent_kind(id), Some(N::SWITCH_STATEMENT | N::SWITCH_EXPRESSION)) {
          self.core.frame().extra.switch_group = Some(false);
        } else {
          self.core.push_scope(true);
        }
        true
      }
      N::FOR_STATEMENT | N::FOREACH_STATEMENT | N::FOREACH_PATTERN_STATEMENT | N::CATCH_SECTION | N::SWITCH_LABELED_RULE | N::LAMBDA_EXPRESSION => {
        self.core.push_scope(true);
        true
      }
      N::TRY_STATEMENT => {
        if self.child(id, N::RESOURCE_LIST).is_some() {
          self.core.push_scope(true);
        }
        true
      }
      N::SWITCH_LABEL_STATEMENT => {
        self.case_group(id);
        true
      }
      N::INSTANCE_OF_EXPRESSION => {
        if let Some(operand) = psi::instance_of_expression::get_operand(self.tree, id) {
          self.smart_cast(operand);
        }
        true
      }
      N::TYPE_CAST_EXPRESSION => {
        if let Some(operand) = psi::type_cast_expression::get_operand(self.tree, id) {
          self.smart_cast(operand);
        }
        true
      }
      N::METHOD_CALL_EXPRESSION => {
        self.call(id);
        true
      }
      N::REFERENCE_EXPRESSION => {
        let (role, shape) = match mark {
          Some(Mark::Role { role, shape, .. }) => (role, shape),
          _ => (Role::Value, None),
        };
        self.member(id, role, shape);
        true
      }
      N::JAVA_CODE_REFERENCE => {
        self.code_reference(id, mark);
        true
      }
      N::METHOD_REF_EXPRESSION => {
        self.method_reference(id);
        true
      }
      N::NEW_EXPRESSION => {
        self.creation(id);
        true
      }
      N::ANNOTATION => {
        if let Some(name) = psi::annotation::get_name_reference_element(self.tree, id) {
          let mark = Mark::Role {
            role: Role::Type,
            shape: None,
            chain: Role::Value,
          };
          self.core.marks.insert(name.0, mark);
        }
        true
      }
      N::THIS_EXPRESSION | N::SUPER_EXPRESSION => {
        if let Some(qualifier) = self.outer_qualifier(id) {
          self.core.marks.insert(qualifier.0, Mark::Values);
        }
        true
      }
      _ => true,
    }
  }

  // --- declarations ---

  /// A class, interface, enum, record or annotation; its type parameters are visible in it
  fn class(&mut self, id: NodeId) {
    let keyword = self.class_keyword(id);
    let own = match keyword {
      Some(T::INTERFACE_KEYWORD) if psi::class::is_annotation_type(self.tree, id) => Holder::Annotation,
      Some(T::INTERFACE_KEYWORD) => Holder::Interface,
      Some(T::ENUM_KEYWORD) => Holder::Enum,
      Some(T::CLASS_KEYWORD | T::RECORD_KEYWORD) => Holder::Class,
      _ => Holder::Leaf,
    };
    let name = psi::get_name_identifier(self.tree, id);
    let supertypes = self.supertypes_of(id);
    let mut fields = JvmFields {
      unstable: self.unstable(id),
      supertypes: supertypes.clone(),
      ..JvmFields::default()
    };
    self.core.push_supertypes(supertypes);
    if self.core.bindings.is_local() {
      self.bind_name(name, None, false);
      self.core.push_scope(false);
      return;
    }
    if let Some(name) = name {
      let modifiers = psi::get_modifier_list(self.tree, id);
      let (kind, reach, member) = self.traits(modifiers, DeclKind::Type, own, keyword == Some(T::CLASS_KEYWORD));
      // a parse error in a class may swallow a constructor: its constructors are unknown then
      if own == Holder::Class && !self.has_error(id) {
        fields.constructors = Some(if keyword == Some(T::RECORD_KEYWORD) {
          vec![Constructor {
            signature: self.signature(id),
            reach,
          }]
        } else {
          Vec::new()
        });
      }
      let span = self.member_span(id, name);
      let decl = self.declare(name, (kind, reach, member), &span, fields, None);
      self.core.enter_container(decl, own);
    }
    self.core.push_scope(false);
  }

  /// A method or an annotation's element: a function; a method's parameters and body are local
  fn method(&mut self, id: NodeId, element: bool) {
    let name = psi::get_name_identifier(self.tree, id);
    let mut fields = self.function_fields(id, name, element);
    fields.unstable = self.unstable(id);
    if !element {
      self.core.push_supertypes(Vec::new());
    }
    if self.core.bindings.is_local() {
      self.bind_name(name, None, false);
      if !element {
        self.core.push_scope(true);
      }
      return;
    }
    if let Some(name) = name {
      let modifiers = psi::get_modifier_list(self.tree, id);
      let traits = self.traits(modifiers, DeclKind::Function, Holder::Leaf, false);
      let span = self.member_span(id, name);
      let header = self.header(None, id, !element);
      let decl = self.declare(name, traits, &span, fields, Some(header));
      self.core.enter_container(decl, Holder::Leaf);
    }
    if !element {
      self.core.push_scope(true);
    }
  }

  /// The signature, the modifiers and the `Override` references of a method or an annotation's element
  fn function_fields(&mut self, id: NodeId, name: Option<NodeId>, element: bool) -> JvmFields {
    let modifiers = psi::get_modifier_list(self.tree, id);
    let (overrides, test, overridable) = if element {
      (false, false, false)
    } else {
      let annotations = self.annotation_paths(modifiers);
      let overrides = annotations.iter().any(|path| matches!(path.as_slice(), ["Override"] | ["java", "lang", "Override"]));
      let test = annotations.iter().any(|path| path.last().is_some_and(|last| TEST_ANNOTATIONS.contains(last)));
      let open = match self.core.holder() {
        Some(Holder::Interface) => true,
        Some(Holder::Class | Holder::Enum) => !self.in_final_class(id),
        _ => false,
      };
      let has = |modifier| self.modifier(modifiers, modifier);
      let overridable = open && !self.core.bindings.is_local() && !has(Modifier::FINAL) && !has(Modifier::PRIVATE) && !has(Modifier::STATIC);
      (overrides, test, overridable)
    };
    // a local function is no declaration: only the shape of an override needs its parameters
    let signature = (overrides || !self.core.bindings.is_local()).then(|| self.signature(id));
    if let (true, Some(name)) = (overrides, name) {
      self.override_references(name, signature.as_ref());
    }
    let text = name.and_then(|it| self.text(it));
    let test = test || self.core.classes.junit3_test(&self.core.out, self.core.bindings.is_local(), text, signature.as_ref());
    let entry_point = !element && !self.core.bindings.is_local() && text == Some("main") && signature.as_ref().is_some_and(|it| self.entry_point(id, modifiers, it));
    JvmFields {
      signature,
      overrides,
      overridable,
      test,
      entry_point,
      ..JvmFields::default()
    }
  }

  /// A constructor, a scope of its parameters, whose name is no reference: a constructor of the class around it. The
  /// canonical one of a record (JLS 8.10.4), compact or declared again with the names and types of its components,
  /// every type known, replaces the one of the record's header
  fn constructor(&mut self, id: NodeId) {
    let tree = self.tree;
    let class = psi::method::get_containing_class(tree, id).filter(|it| self.kind(*it) == Some(N::CLASS));
    let record = class.filter(|it| psi::class::is_record(tree, *it));
    let header = record.map(|it| self.signature(it));
    let compact = psi::method::is_compact_constructor(tree, id);
    let declared = if compact { header.clone() } else { Some(self.signature(id)) };
    if let Some(signature) = declared {
      let canonical = compact || header.is_some_and(|header| header == signature && signature.parameters.iter().all(|it| it.type_ref.is_some()));
      let reach = self.reach_of(psi::get_modifier_list(tree, id), true, false);
      if !self.core.bindings.is_local()
        && let Some(decl) = self.core.classes.innermost()
      {
        self.core.out.add_constructor(decl, Constructor { signature, reach }, canonical);
      }
    }
    self.core.push_scope(true);
  }

  /// A field, a local variable: a local binds its name once left, with its type
  fn variable(&mut self, id: NodeId) {
    let tree = self.tree;
    let (ty, modifiers) = self.declaration_head(id);
    let name = psi::get_name_identifier(tree, id);
    let initializer = psi::get_initializer(tree, id);
    let value_type = if name.is_some_and(|it| psi::java_shared_impl_util::c_style_dimensions(tree, it) > 0) {
      // `Foo a[]`: an array, of no type here
      None
    } else {
      self.value_type(ty, initializer)
    };
    if self.core.bindings.is_local() {
      self.bind_name(name, value_type, true);
      return;
    }
    let Some(name) = name else { return };
    let traits = self.traits(modifiers, DeclKind::Value, Holder::Leaf, false);
    let fields = JvmFields {
      unstable: self.annotation_paths(modifiers).iter().any(|path| self.core.out.unstable_annotation(path)),
      value_type,
      ..JvmFields::default()
    };
    // a declarator starts at its name, after the modifiers and the type of its declaration; a later one at a doc comment
    // above it too (`int a, /** b */ b;`)
    // the head of the declaration, whose modifiers and type a later variable shares (`b` of `int a, b;`)
    let head = ty.and_then(|it| tree.parent(it)).filter(|it| *it != id);
    let start = tree.range(name).start;
    let mut start_line = self.core.lines.line(start);
    if head.is_some() {
      for child in tree.children(id).take_while(|it| *it != name).filter(|it| self.tree.is_doc_comment(*it)) {
        start_line = start_line.min(self.core.line_at(child));
      }
    }
    let span = Span {
      start: start_line,
      end: self.core.end_line(start, self.tree.declaration_range(id).end),
      name: self.core.lines.line(start),
      name_column: column_at(self.core.source, start),
    };
    let header = self.header(head, id, false);
    let decl = self.declare(name, traits, &span, fields, Some(header));
    self.core.frame().extra.deferred = Some(decl);
  }

  /// An enum constant, whose body is local and overrides the members of its enum
  fn enum_constant(&mut self, id: NodeId) {
    let name = psi::get_name_identifier(self.tree, id);
    let fields = JvmFields {
      unstable: self.unstable(id),
      ..JvmFields::default()
    };
    let supertypes = self.core.classes.enum_supertypes(&self.core.out);
    self.core.push_supertypes(supertypes);
    if self.core.bindings.is_local() {
      self.bind_name(name, None, false);
      return;
    }
    if let Some(name) = name {
      let traits = self.traits(psi::get_modifier_list(self.tree, id), DeclKind::EnumEntry, Holder::Leaf, false);
      let span = self.member_span(id, name);
      let decl = self.declare(name, traits, &span, fields, None);
      self.core.enter_container(decl, Holder::Leaf);
    }
    if let Some(body) = psi::enum_constant::get_initializing_class(self.tree, id) {
      self.core.marks.insert(body.0, Mark::Scope);
    }
  }

  /// A component of a record: a public field (and its accessor), a vararg one too (`int... xs`, which tree-sitter's
  /// walk read as a field of the package without a type); in a local record a parameter
  fn record_component(&mut self, id: NodeId) {
    let name = psi::get_name_identifier(self.tree, id);
    let ty = psi::get_type_element(self.tree, id);
    let vararg = self.is_var_args(id);
    if self.core.bindings.is_local() {
      let value_type = if vararg { None } else { self.value_type(ty, None) };
      self.bind_name(name, value_type, vararg);
      return;
    }
    let Some(name) = name else { return };
    let fields = JvmFields {
      unstable: self.unstable(id),
      ..JvmFields::default()
    };
    let span = self.core.span(self.tree.declaration_range(id), self.tree.range(name).start, None);
    let header = self.header(None, id, false);
    self.declare(name, (DeclKind::Value, Reach::Public, MemberKind::Instance), &span, fields, Some(header));
  }

  /// A parameter of a method or a lambda binds its name with its declared type (a vararg and `Foo a[]` none); the
  /// variable of a `for` and the parameter of a `catch` without
  fn parameter(&mut self, id: NodeId) {
    let tree = self.tree;
    let name = psi::get_name_identifier(tree, id);
    let typed = self.parent_kind(id) == Some(N::PARAMETER_LIST) && name.is_none_or(|it| psi::java_shared_impl_util::c_style_dimensions(tree, it) == 0);
    let ty = psi::get_type_element(tree, id);
    let vararg = self.is_var_args(id);
    let value_type = if typed && !vararg { self.value_type(ty, None) } else { None };
    self.bind_name(name, value_type, false);
  }

  /// Binds the name `name` in the current scope, `late` when the current node is left
  fn bind_name(&mut self, name: Option<NodeId>, type_path: Option<Typed>, late: bool) {
    let Some(text) = name.and_then(|it| self.text(it)) else { return };
    let name = self.core.out.intern(text);
    if late {
      let index = self.core.frames.len() - 1;
      self.core.bindings.bind_later(index, name, type_path);
    } else {
      self.core.bindings.bind(name, type_path);
    }
  }

  /// A `case` group of a switch's body (labels, then statements) is a scope until the next group
  fn case_group(&mut self, id: NodeId) {
    let Some(parent) = self.core.frames.len().checked_sub(2) else { return };
    let Some(open) = self.core.frames[parent].extra.switch_group else { return };
    let previous = std::iter::successors(self.tree.previous_sibling(id), |it| self.tree.previous_sibling(*it)).find(|it| !self.tree.is_trivia(*it));
    if previous.is_some_and(|it| self.kind(it) == Some(N::SWITCH_LABEL_STATEMENT)) {
      return;
    }
    if open {
      self.core.bindings.pop();
      self.core.frames[parent].scopes -= 1;
    }
    self.core.bindings.push(true);
    self.core.frames[parent].scopes += 1;
    self.core.frames[parent].extra.switch_group = Some(true);
  }

  /// The kind, reach and member kind of a declaration of `kind` with these modifiers, given what it is to its own
  /// members and whether it is a `class` (only a class without `static` is inner)
  fn traits(&self, modifiers: Option<NodeId>, kind: DeclKind, own: Holder, class: bool) -> (DeclKind, Reach, MemberKind) {
    let has = |modifier| self.modifier(modifiers, modifier);
    let holder = self.core.holder();
    let in_interface = matches!(holder, Some(Holder::Interface | Holder::Annotation));
    // only a class without `static` is inner; enums, records, interfaces and annotations are static
    let nested_type = own != Holder::Leaf && (!class || has(Modifier::STATIC));
    let mut kind = kind;
    if kind == DeclKind::Value && (in_interface || (has(Modifier::STATIC) && has(Modifier::FINAL))) {
      kind = DeclKind::Constant;
    }
    let member = match holder {
      None => MemberKind::TopLevel,
      Some(_) if kind == DeclKind::EnumEntry || kind == DeclKind::Constant || nested_type || has(Modifier::STATIC) => MemberKind::Static,
      Some(_) if in_interface && own != Holder::Leaf => MemberKind::Static,
      Some(_) => MemberKind::Instance,
    };
    let reach = self.reach_of(modifiers, holder.is_some(), kind == DeclKind::EnumEntry);
    (kind, reach, member)
  }

  /// The reach of these modifiers, the implicit ones included (`PsiModifierListImpl.hasModifierProperty`): `public` and
  /// `protected` are public (a member of an interface that is not `private` is `public`, and so is an enum constant),
  /// `private` of the container (of the file at the top level; a private method of an interface, the constructor of
  /// an enum), none of the package
  fn reach_of(&self, modifiers: Option<NodeId>, member: bool, public: bool) -> Reach {
    let has = |modifier| modifiers.is_some_and(|it| psi::modifier_list::has_modifier_property(self.tree, it, modifier));
    if public || has(Modifier::PUBLIC) || has(Modifier::PROTECTED) {
      Reach::Public
    } else if has(Modifier::PRIVATE) {
      if member { Reach::Container } else { Reach::File }
    } else {
      Reach::Module
    }
  }

  fn declare(&mut self, name: NodeId, traits: (DeclKind, Reach, MemberKind), span: &Span, fields: JvmFields, header: Option<String>) -> u32 {
    let text = self.text(name).unwrap_or_default();
    self.core.declare(text, traits, span, fields, header, self.error_depth > 0)
  }

  /// The lines of a class, a method or an enum constant: from its first token, or a doc comment above it, to its last
  fn member_span(&self, id: NodeId, name: NodeId) -> Span {
    let doc = self
      .tree
      .children(id)
      .take_while(|child| self.tree.is_trivia(*child))
      .filter(|child| self.tree.is_doc_comment(*child))
      .map(|child| self.tree.range(child).start)
      .min();
    self.core.span(self.tree.declaration_range(id), self.tree.range(name).start, doc)
  }

  /// Whether an anonymous class or a local declaration lies between the current node and the declaration around it
  /// (`Classes::enclosed`; a lambda pushes no supertypes in Java)
  fn enclosed(&self) -> bool {
    self.core.classes.enclosed()
  }

  // --- the JVM facts of a declaration ---

  /// `extends` and `implements` of a class, an enum or a record, `extends` of an interface, each as written without
  /// type arguments
  fn supertypes_of(&mut self, id: NodeId) -> Vec<Vec<u32>> {
    let tree = self.tree;
    let lists = [psi::class::get_extends_list(tree, id), psi::class::get_implements_list(tree, id)];
    let paths: Vec<Vec<&'t str>> = lists
      .into_iter()
      .flatten()
      .flat_map(|list| psi::reference_list::get_reference_elements(tree, list))
      .filter_map(|reference| self.path_of(reference))
      .collect();
    paths.iter().map(|it| self.core.out.ids(it)).collect()
  }

  /// One `Override` reference per supertype of the class around the method, qualified by its path
  fn override_references(&mut self, name: NodeId, signature: Option<&Signature>) {
    let Some(supertypes) = self.core.classes.overridden() else { return };
    let Some(text) = self.text(name) else { return };
    let shape = override_shape(&mut self.core.out, true, None, signature);
    let line = self.core.line_at(name);
    for supertype in supertypes {
      self.reference(text, line, Role::Override, Some(supertype), Some(shape));
    }
  }

  /// `public static void main(String[] args)` of a type, `String... args` and `String args[]` too; `public` is implied
  /// only for a non-private method of an interface
  fn entry_point(&self, id: NodeId, modifiers: Option<NodeId>, signature: &Signature) -> bool {
    let holder = self.core.holder();
    let has = |modifier| self.modifier(modifiers, modifier);
    holder.is_some()
      && (has(Modifier::PUBLIC) || (holder == Some(Holder::Interface) && !has(Modifier::PRIVATE)))
      && has(Modifier::STATIC)
      && psi::method::get_return_type_element(self.tree, id).is_some_and(|it| self.has_token(it, T::VOID_KEYWORD))
      && matches!(signature.parameters.as_slice(), [parameter] if self.core.out.type_is(parameter.type_ref.as_ref(), &["String"], 1))
  }

  /// Whether the class whose body holds the method is `final`, explicitly or implicitly (a record, an enum none of whose
  /// constants has a body)
  fn in_final_class(&mut self, id: NodeId) -> bool {
    let Some(class) = psi::method::get_containing_class(self.tree, id).filter(|it| self.kind(*it) == Some(N::CLASS)) else {
      return false;
    };
    // an enum's implicit `final` reads all of its members: the class asked last is kept, whose methods ask in a row
    if let Some(&(known, is_final)) = self.final_class.as_ref()
      && known == class
    {
      return is_final;
    }
    let is_final = psi::has_modifier_property(self.tree, class, Modifier::FINAL);
    self.final_class = Some((class, is_final));
    is_final
  }

  /// Whether an annotation of the declaration marks it as no stable API
  fn unstable(&self, id: NodeId) -> bool {
    self
      .annotation_paths(psi::get_modifier_list(self.tree, id))
      .iter()
      .any(|path| self.core.out.unstable_annotation(path))
  }

  /// The names of the annotations among these modifiers, each as its path (`[org, junit, Test]`)
  fn annotation_paths(&self, modifiers: Option<NodeId>) -> Vec<Vec<&'t str>> {
    let Some(modifiers) = modifiers else { return Vec::new() };
    psi::modifier_list::get_annotations(self.tree, modifiers)
      .filter_map(|it| psi::annotation::get_name_reference_element(self.tree, it))
      .filter_map(|it| self.path_of(it))
      .collect()
  }

  /// The parameters of a method or a constructor, the components of a record (`String... rest` a vararg, an array on
  /// the JVM); an annotation's element has none
  fn signature(&mut self, id: NodeId) -> Signature {
    let tree = self.tree;
    let items: Vec<NodeId> = match self.kind(id) {
      Some(N::CLASS) => psi::class::get_record_header(tree, id).map_or_else(Vec::new, |it| psi::record_header::get_record_components(tree, it).collect()),
      Some(N::ANNOTATION_METHOD) => Vec::new(),
      _ => psi::method::get_parameter_list(tree, id).map_or_else(Vec::new, |it| psi::parameter_list::get_parameters(tree, it).collect()),
    };
    let type_parameters = self.type_parameter_names(id);
    let mut parameters = Vec::new();
    for parameter in items {
      let Some(name) = self.name_text(parameter) else { continue };
      let ty = psi::get_type_element(tree, parameter);
      let vararg = self.is_var_args(parameter);
      // `int[][] x`, `int x[]` and `int... x` are arrays on the JVM: the dimensions of `getType()`
      let after_name = psi::get_name_identifier(tree, parameter).map_or(0, |it| psi::java_shared_impl_util::c_style_dimensions(tree, it));
      let arrays = ty.map_or(0, |it| psi::type_element::calculate_type(tree, it).array_dimensions) + after_name;
      let arrays = u8::try_from(arrays).unwrap_or(u8::MAX);
      let type_ref = ty.and_then(|it| self.type_ref(it, arrays, &type_parameters));
      parameters.push(Parameter {
        name: self.core.out.intern(name),
        type_ref,
        optional: false,
        vararg,
      });
    }
    Signature { parameters }
  }

  /// The names of the type parameters of a declaration (`<T, R>`)
  fn type_parameter_names(&self, id: NodeId) -> Vec<&'t str> {
    let tree = self.tree;
    let list = if self.kind(id) == Some(N::CLASS) {
      psi::class::get_type_parameter_list(tree, id)
    } else {
      psi::method::get_type_parameter_list(tree, id)
    };
    list
      .into_iter()
      .flat_map(|it| psi::type_parameter_list::get_type_parameters(tree, it))
      .filter_map(|it| self.name_text(it))
      .collect()
  }

  /// The type of a parameter as the last name of each type in it, in source order (`TypeRef`); `None` when a type
  /// parameter is in it, of the function (`type_parameters`) or bound around it, or when it has no names
  fn type_ref(&mut self, ty: NodeId, arrays: u8, type_parameters: &[&'t str]) -> Option<TypeRef> {
    let mut names = Vec::new();
    let mut stack = vec![ty];
    while let Some(node) = stack.pop() {
      if let Some(token) = self.tree.token_kind(node) {
        if is_primitive(token)
          && let Some(text) = self.text(node)
        {
          names.push(self.core.out.intern(text));
        }
        continue;
      }
      let parent = match self.kind(node) {
        Some(N::JAVA_CODE_REFERENCE) => {
          let path = self.path_of(node)?;
          names.push(type_name(&mut self.core.out, &self.core.bindings, &path, type_parameters)?);
          // the type arguments of the last name; those of a qualifier are of no name here
          match psi::code_reference::get_parameter_list(self.tree, node) {
            Some(arguments) => arguments,
            None => continue,
          }
        }
        Some(N::ANNOTATION) | None => continue,
        Some(_) => node,
      };
      let start = stack.len();
      stack.extend(self.tree.children(parent).filter(|it| !self.tree.is_trivia(*it)));
      stack[start..].reverse();
    }
    (!names.is_empty()).then_some(TypeRef { names, nullable: false, arrays })
  }

  /// The type a value is declared with, as written without type arguments (`java.util.List` of `java.util.List<T>`),
  /// not a type parameter, a primitive or an array; of a `var` the type its initializer constructs, a `new` without a
  /// body (an anonymous class is a type of its own)
  fn value_type(&mut self, ty: Option<NodeId>, initializer: Option<NodeId>) -> Option<Typed> {
    let ty = ty?;
    let constructed = psi::type_element::is_inferred_type(self.tree, ty);
    let path = if constructed {
      let new =
        initializer.filter(|it| self.kind(*it) == Some(N::NEW_EXPRESSION) && psi::new_expression::get_anonymous_class(self.tree, *it).is_none() && !self.is_array_creation(*it))?;
      self.path_of(psi::new_expression::get_class_reference(self.tree, new)?)?
    } else {
      self.path_of(ty)?
    };
    self.core.typed(&path, constructed)
  }

  /// The type element and the modifier list of a `FIELD` or a `LOCAL_VARIABLE`, those of its declaration's first one for
  /// a later one (`b` of `int a, b;`): a field's from `field::find_first_field_in_declaration`, the field met last with
  /// its own modifiers when it is of the same parent (no declaration in an initializer came between them), so a long
  /// declaration costs a step a field
  fn declaration_head(&mut self, id: NodeId) -> (Option<NodeId>, Option<NodeId>) {
    let tree = self.tree;
    if self.kind(id) != Some(N::FIELD) {
      return (psi::get_type_element(tree, id), psi::get_modifier_list(tree, id));
    }
    let head = if psi::field::get_modifier_list_with_head(tree, id).is_some() {
      self.last_head = Some(id);
      id
    } else {
      match self.last_head.filter(|it| tree.parent(*it) == tree.parent(id)) {
        Some(head) => head,
        None => psi::field::find_first_field_in_declaration(tree, id),
      }
    };
    (psi::field::get_type_element_with_head(tree, head), psi::field::get_modifier_list_with_head(tree, head))
  }

  /// The header of a declaration (`Declaration::header`): the tokens of `head`'s modifiers and type (the head of a
  /// later variable of a declaration), then those of `id` before its body or initializer, without comments,
  /// annotations, the modifiers no use depends on and, when `drop_parameters`, the parameter list
  fn header(&self, head: Option<NodeId>, id: NodeId, drop_parameters: bool) -> String {
    let mut tokens: Vec<&'t str> = Vec::new();
    if let Some(head) = head {
      for child in self.tree.children(head).filter(|it| matches!(self.kind(*it), Some(N::MODIFIER_LIST | N::TYPE))) {
        self.header_tokens(child, false, &mut tokens);
      }
    }
    self.header_tokens(id, drop_parameters, &mut tokens);
    tokens.join(" ")
  }

  /// The header tokens of `id` and below, until a body or the `=` of an initializer
  fn header_tokens(&self, id: NodeId, drop_parameters: bool, tokens: &mut Vec<&'t str>) {
    let prune = |node: NodeId| self.tree.is_trivia(node) || self.kind(node) == Some(N::ANNOTATION) || (drop_parameters && self.kind(node) == Some(N::PARAMETER_LIST));
    let mut parameters_end = 0;
    for node in self.tree.descendants_pruned(id, prune) {
      match self.kind(node) {
        Some(N::PARAMETER_LIST) => parameters_end = self.tree.subtree_end(node).0,
        Some(N::CODE_BLOCK) => break,
        _ => {}
      }
      let Some(token) = self.tree.token_kind(node) else { continue };
      if token == T::EQ && node.0 >= parameters_end {
        break;
      }
      if is_dropped(token) {
        continue;
      }
      if let Some(text) = self.text(node).filter(|it| !it.is_empty()) {
        tokens.push(text);
      }
    }
  }

  // --- references ---

  /// A call: the callee's reference gets the shape of the arguments; `this(…)` and `super(…)` call a constructor of
  /// the class around them, or of its superclass, the type of `extends` (a class without one calls `Object`'s: no
  /// reference)
  fn call(&mut self, id: NodeId) {
    let tree = self.tree;
    let Some(callee) = psi::method_call_expression::get_method_expression(tree, id) else {
      return;
    };
    let shape = self.call_shape(psi::method_call_expression::get_argument_list(tree, id));
    if self.name_leaf(callee).is_some() {
      let mark = Mark::Role {
        role: Role::Call,
        shape,
        chain: Role::Value,
      };
      self.core.marks.insert(callee.0, mark);
      return;
    }
    let Some(keyword) = psi::reference_expression::get_reference_name_element(tree, callee).filter(|it| matches!(tree.token_kind(*it), Some(T::THIS_KEYWORD | T::SUPER_KEYWORD)))
    else {
      return;
    };
    let class = std::iter::successors(self.tree.parent(id), |it| self.tree.parent(*it))
      .find(|it| self.kind(*it) == Some(N::CLASS) && matches!(self.class_keyword(*it), Some(T::CLASS_KEYWORD | T::ENUM_KEYWORD | T::RECORD_KEYWORD)));
    let Some(class) = class else { return };
    let path = if self.tree.token_kind(keyword) == Some(T::THIS_KEYWORD) {
      self.name_text(class).map(|it| vec![it])
    } else if self.class_keyword(class) == Some(T::CLASS_KEYWORD) {
      psi::class::get_extends_list(tree, class)
        .and_then(|it| psi::reference_list::get_reference_elements(tree, it).next())
        .and_then(|it| self.path_of(it))
    } else {
      None
    };
    let Some((name, qualifier)) = path.as_deref().and_then(<[&str]>::split_last) else {
      return;
    };
    let qualifier = (!qualifier.is_empty()).then(|| self.core.out.ids(qualifier));
    let line = self.core.line_at(keyword);
    self.reference(name, line, Role::Call, qualifier, shape);
  }

  /// `new Type(…)` calls the type with the shape of its arguments, which are of the code around an anonymous class; an
  /// anonymous class declares nothing, its methods override those of the type. An array's type is a type
  fn creation(&mut self, id: NodeId) {
    let tree = self.tree;
    let anonymous = psi::new_expression::get_anonymous_class(tree, id);
    let constructed = match anonymous {
      Some(class) => psi::anonymous_class::get_base_class_reference(tree, class),
      None => psi::new_expression::get_class_reference(tree, id),
    };
    if let Some(constructed) = constructed.filter(|_| !self.is_array_creation(id)) {
      let shape = self.call_shape(psi::new_expression::get_argument_list(tree, id));
      let mark = Mark::Role {
        role: Role::Call,
        shape,
        chain: Role::Type,
      };
      self.core.marks.insert(constructed.0, mark);
    }
    if anonymous.is_some() {
      let path = constructed.and_then(|it| self.path_of(it));
      let supertypes = path.iter().map(|it| self.core.out.ids(it)).collect();
      self.core.push_supertypes(supertypes);
    }
  }

  /// `Type::method`, `object::method`; `Type::new` names no method
  fn method_reference(&mut self, id: NodeId) {
    let Some(name) = self.name_leaf(id) else { return };
    let qualifier = psi::method_reference_expression::get_qualifier(self.tree, id).map_or_else(Vec::new, |it| self.qualifier_of(it));
    if let Some(text) = self.text(name) {
      let line = self.core.line_at(name);
      self.reference(text, line, Role::Value, Some(qualifier), None);
    }
  }

  /// A name in a type (an annotation's, a `new`'s), qualified by the path of the reference before it, whose segments
  /// are types (values in an annotation's name and before `.this`)
  fn code_reference(&mut self, id: NodeId, mark: Option<Mark>) {
    let (role, shape, chain) = match mark {
      Some(Mark::Role { role, shape, chain }) => (role, shape, chain),
      Some(Mark::Values) => (Role::Value, None, Role::Value),
      _ => (Role::Type, None, Role::Type),
    };
    if chain == Role::Value
      && let Some(qualifier) = self.qualifier(id)
    {
      self.core.marks.insert(qualifier.0, Mark::Values);
    }
    self.member(id, role, shape);
  }

  /// The reference of the name of `id`, of `role` and `shape`, qualified by the path of its qualifier, with the type
  /// of its receiver
  fn member(&mut self, id: NodeId, role: Role, shape: Option<u32>) {
    let Some(name) = self.name_leaf(id) else { return };
    let Some(text) = self.text(name) else { return };
    let line = self.core.line_at(name);
    let Some(object) = self.qualifier(id) else {
      self.reference(text, line, role, None, shape);
      return;
    };
    let qualifier = self.qualifier_of(object);
    let receiver = if matches!(role, Role::Call | Role::Value) {
      self.typed_receiver(object, &qualifier)
    } else {
      None
    };
    if !is_reference_name(text) {
      return;
    }
    self.core.member_reference(text, line, role, qualifier, shape, Some(false), receiver);
  }

  /// What the receiver `object` of a member reference may have a type by: a bound name with a declared type, or, not
  /// enclosed, a property `[m]` or `[this, m]` of the qualifier
  fn typed_receiver(&self, object: NodeId, qualifier: &[u32]) -> Option<TypedReceiver> {
    let bare = matches!(self.kind(object), Some(N::REFERENCE_EXPRESSION | N::JAVA_CODE_REFERENCE)) && self.qualifier(object).is_none();
    self.core.typed_receiver(if bare { self.name_text(object) } else { None }, qualifier)
  }

  /// The interned `Shape::Call` of the arguments `list`, `enclosed` by the walk
  fn call_shape(&mut self, list: Option<NodeId>) -> Option<u32> {
    let list = list?;
    let arguments = self.tree.composite_children(list).map(|it| (None, self.argument(it))).collect();
    let enclosed = self.enclosed();
    Some(self.core.out.intern_shape(Shape::Call {
      arguments,
      spread: false,
      enclosed,
    }))
  }

  /// What an argument is: a string literal or text block, an integer without the `L` suffix (`-1` too), `true` or
  /// `false`, a lambda; `null`, a real, a character and `1L` are expressions
  fn argument(&self, value: NodeId) -> Argument {
    let literal = |it: NodeId| if self.kind(it) == Some(N::LITERAL_EXPRESSION) { self.child_token(it) } else { None };
    match self.kind(value) {
      Some(N::LITERAL_EXPRESSION) => match literal(value) {
        Some(T::STRING_LITERAL | T::TEXT_BLOCK_LITERAL) => Argument::Text,
        Some(T::INTEGER_LITERAL) => Argument::Number,
        Some(T::TRUE_KEYWORD | T::FALSE_KEYWORD) => Argument::Boolean,
        _ => Argument::Expression,
      },
      Some(N::PREFIX_EXPRESSION) => {
        let signed = self.tree.children(value).any(|it| matches!(self.tree.token_kind(it), Some(T::MINUS | T::PLUS)));
        let operand = self.tree.composite_children(value).next();
        if signed && operand.and_then(literal) == Some(T::INTEGER_LITERAL) {
          Argument::Number
        } else {
          Argument::Expression
        }
      }
      Some(N::LAMBDA_EXPRESSION) => Argument::Lambda,
      _ => Argument::Expression,
    }
  }

  /// The operand of an `instanceof` or a cast: a cast may give a name of it, a local or a field (`m`, `this.m`), another
  /// type in its scope or function, which the walk can't follow
  fn smart_cast(&mut self, operand: NodeId) {
    if let Some(path) = self.path_of(operand) {
      smart_cast(&mut self.core.out, &self.core.bindings, self.core.classes.container(), &path);
    }
  }

  /// A reference, unless its name is `var` or a bare name bound around it
  fn reference(&mut self, name: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>, shape: Option<u32>) {
    if is_reference_name(name) {
      self.core.reference(name, line, role, qualifier, shape, Some(false));
    }
  }

  /// The qualifier of a member of `object`: its path of names, or empty for an expression or a path from a local
  fn qualifier_of(&mut self, object: NodeId) -> Vec<u32> {
    let path = self.path_of(object);
    self.core.qualifier_ids(path)
  }

  /// `a.b.c` as `[a, b, c]`: a reference and the references of its qualifiers, `this`, `super`, and the type of a
  /// plain class type; `Outer.super` is `Outer`'s; else `None`, also past `MAX_PATH` segments
  fn path_of(&self, id: NodeId) -> Option<Vec<&'t str>> {
    let mut reversed = Vec::new();
    let mut node = id;
    loop {
      match self.kind(node) {
        Some(N::REFERENCE_EXPRESSION | N::JAVA_CODE_REFERENCE | N::IMPORT_STATIC_REFERENCE) => {
          if reversed.len() == MAX_PATH {
            return None;
          }
          reversed.push(self.name_text(node)?);
          match self.qualifier(node) {
            Some(qualifier) => node = qualifier,
            None => break,
          }
        }
        Some(N::THIS_EXPRESSION) if psi::this_expression::get_qualifier(self.tree, node).is_none() => {
          reversed.push("this");
          break;
        }
        Some(N::SUPER_EXPRESSION) => {
          let Some(qualifier) = psi::super_expression::get_qualifier(self.tree, node) else {
            reversed.push("super");
            break;
          };
          node = qualifier;
        }
        Some(N::TYPE) => {
          let mut children = self.tree.children(node).filter(|it| !self.tree.is_trivia(*it));
          match (children.next(), children.next()) {
            (Some(reference), None) if self.kind(reference) == Some(N::JAVA_CODE_REFERENCE) => node = reference,
            _ => return None,
          }
        }
        _ => return None,
      }
    }
    reversed.reverse();
    Some(reversed)
  }

  /// The qualifier of a reference: the expression before the name of a reference expression, the reference before the
  /// name of a code reference (not an annotation of the type, `@A List`, `java.util.@A List`) or of a static import
  fn qualifier(&self, id: NodeId) -> Option<NodeId> {
    match self.kind(id)? {
      N::REFERENCE_EXPRESSION => psi::reference_expression::get_qualifier_expression(self.tree, id),
      N::JAVA_CODE_REFERENCE => psi::code_reference::get_qualifier(self.tree, id),
      N::IMPORT_STATIC_REFERENCE => psi::import_static_reference::get_class_reference(self.tree, id),
      _ => None,
    }
  }

  /// The `Outer` of `Outer.this` and `Outer.super`
  fn outer_qualifier(&self, id: NodeId) -> Option<NodeId> {
    match self.kind(id)? {
      N::THIS_EXPRESSION => psi::this_expression::get_qualifier(self.tree, id),
      N::SUPER_EXPRESSION => psi::super_expression::get_qualifier(self.tree, id),
      _ => None,
    }
  }

  // --- imports ---

  /// The lines of an import or package statement
  fn import_statement(&mut self, id: NodeId) {
    let range = self.tree.declaration_range(id);
    let span = LineSpan {
      start: self.core.lines.line(range.start),
      end: self.core.end_line(range.start, range.end),
    };
    self.core.out.import_lines.push(span);
  }

  /// `import a.b.C;`, `import a.b.*;`, `import static a.B.c;`, `import static a.B.*;`: the import, and the `Import`
  /// reference to a name
  fn import(&mut self, id: NodeId) {
    self.import_statement(id);
    let reference = psi::import_statement::get_import_reference(self.tree, id);
    let Some(segments) = reference.and_then(|it| self.path_of(it)) else { return };
    let line = self.core.line_at(id);
    // by its `*`, not `isOnDemand`'s dot: `import a.b.;` imports `b`
    if self.has_token(id, T::ASTERISK) {
      self.core.out.add_import_all(&segments, false, line);
    } else if let Some((name, path)) = segments.split_last() {
      self.core.out.add_import_name(path, name, None, false, line);
      let qualifier = (!path.is_empty()).then(|| self.core.out.ids(path));
      self.reference(name, line, Role::Import, qualifier, None);
    }
  }

  // --- reading the tree ---

  fn kind(&self, id: NodeId) -> Option<N> {
    self.tree.node_kind(id)
  }

  fn parent_kind(&self, id: NodeId) -> Option<N> {
    self.tree.parent(id).and_then(|it| self.kind(it))
  }

  fn child(&self, id: NodeId, kind: N) -> Option<NodeId> {
    self.tree.first_child_of_kind(id, kind)
  }

  /// The token of the first leaf child
  fn child_token(&self, id: NodeId) -> Option<T> {
    self.tree.children(id).find_map(|it| self.tree.token_kind(it))
  }

  fn has_token(&self, id: NodeId, token: T) -> bool {
    self.tree.children(id).any(|it| self.tree.token_kind(it) == Some(token))
  }

  /// Whether these modifiers have this keyword, the explicit modifiers only
  fn modifier(&self, modifiers: Option<NodeId>, modifier: Modifier) -> bool {
    modifiers.is_some_and(|it| psi::modifier_list::has_explicit_modifier(self.tree, it, modifier))
  }

  /// The keyword of a `CLASS`: `class`, `interface` (also of `@interface`), `enum` or `record`
  fn class_keyword(&self, id: NodeId) -> Option<T> {
    psi::class::class_or_interface_keyword(self.tree, id).and_then(|it| self.tree.token_kind(it))
  }

  /// Whether a parameter or a record component is a vararg (`String... rest`)
  fn is_var_args(&self, id: NodeId) -> bool {
    match self.kind(id) {
      Some(N::PARAMETER) => psi::parameter::is_var_args(self.tree, id),
      Some(N::RECORD_COMPONENT) => psi::record_component::is_var_args(self.tree, id),
      _ => false,
    }
  }

  /// The name of a declaration or a reference: its name identifier, the `IDENTIFIER` a reference names (none for the
  /// `this` and `super` of a delegating call, the `new` of `Type::new`)
  fn name_leaf(&self, id: NodeId) -> Option<NodeId> {
    let tree = self.tree;
    let name = psi::get_reference_name_element(tree, id).or_else(|| psi::get_name_identifier(tree, id));
    name.filter(|it| tree.token_kind(*it) == Some(T::IDENTIFIER))
  }

  fn name_text(&self, id: NodeId) -> Option<&'t str> {
    self.name_leaf(id).and_then(|it| self.text(it))
  }

  fn text(&self, id: NodeId) -> Option<&'t str> {
    self.core.text_of(id)
  }

  /// Whether the subtree of `id` holds an error element of the code
  fn has_error(&self, id: NodeId) -> bool {
    has_error_in(&self.errors, id.0, self.tree.subtree_end(id).0)
  }

  /// `new int[n]`, `new Foo[] {…}`: an array, no call of a constructor
  fn is_array_creation(&self, id: NodeId) -> bool {
    self
      .tree
      .children(id)
      .any(|it| self.tree.token_kind(it) == Some(T::LBRACKET) || self.kind(it) == Some(N::ARRAY_INITIALIZER_EXPRESSION))
  }
}

/// Whether a name is one a reference may have: not empty, not `var`
fn is_reference_name(name: &str) -> bool {
  !name.is_empty() && name != "var"
}

/// The segments of the longest path (`Walk::path_of`); a longer one is of an expression. Every segment of a chain
/// `a.b.c…` is a reference qualified by the segments before it, so without a bound a chain of `n` segments would take
/// `n²/2` names: 20 000 in 60 KB took 4 s and 1.7 GB. The longest of intellij-community has 11.
const MAX_PATH: usize = 64;

/// A primitive type's keyword, a name of a `TypeRef`
const fn is_primitive(token: T) -> bool {
  matches!(
    token,
    T::BOOLEAN_KEYWORD | T::BYTE_KEYWORD | T::CHAR_KEYWORD | T::SHORT_KEYWORD | T::INT_KEYWORD | T::LONG_KEYWORD | T::FLOAT_KEYWORD | T::DOUBLE_KEYWORD
  )
}

/// The modifiers a header leaves out: those that the reach, `overridable` and `overrides` say or that no use depends on
const fn is_dropped(token: T) -> bool {
  matches!(
    token,
    T::PUBLIC_KEYWORD
      | T::PROTECTED_KEYWORD
      | T::PRIVATE_KEYWORD
      | T::FINAL_KEYWORD
      | T::ABSTRACT_KEYWORD
      | T::SYNCHRONIZED_KEYWORD
      | T::NATIVE_KEYWORD
      | T::STRICTFP_KEYWORD
      | T::TRANSIENT_KEYWORD
      | T::VOLATILE_KEYWORD
      | T::DEFAULT_KEYWORD
      | T::SEMICOLON
  )
}

#[cfg(test)]
#[path = "summary_test.rs"]
mod tests;
