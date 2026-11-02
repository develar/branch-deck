//! The summary of a Java file from its PSI (`FileSummary`), by the rules that the head of `extract.rs` states for
//! every language: locals dropped, overrides by the stack of supertypes, `Shape::Call` of a call, the constructors of a
//! class, `enclosed`. One scan over the preorder ids of the tree (`Walk::run`) with an explicit stack of frames, so a
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
//! - **As tree-sitter's walk had them**: a local of a `case` group is bound until the next group; the loop variable of
//!   a `for` and the parameter of a `catch` have no type.

use crate::summary::{Argument, Constructor, DeclKind, Declaration, FileSummary, LineSpan, MemberKind, Parameter, Reach, Role, Shape, Signature, TypeRef};
use crate::summary_builder::{Bindings, SummaryBuilder, TEST_ANNOTATIONS, Typed, blank_declaration, end_line, is_junit3_test};
use java_syntax::{NodeId, NodeKind as N, TokenKind as T, Tree};
use psi_builder::document::Lines;
use std::collections::HashMap;

/// Fills `summary` (`summary_builder::empty_summary` of the file, its size within the limits) with the facts of the
/// Java file `source`; whether it was read, else it stays `truncated`
pub(crate) fn summarize(source: &[u8], summary: &mut FileSummary) -> bool {
  let tree = super::parse(source);
  let mut walk = Walk::new(&tree);
  walk.run();
  // javac gives the default constructor the access of its class (JLS 8.8.9)
  walk.out.finish(summary, Some(|reach| reach));
  true
}

/// What a container is to the declarations directly in it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
  Class,
  /// Its fields are constants and its members public
  Interface,
  Enum,
  Annotation,
  /// A method, a field, an enum constant: nothing in it is a member
  Leaf,
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

struct Frame {
  /// The end of the node's subtree: the frame is left at this id
  end: u32,
  scopes: u8,
  container: bool,
  /// The entries this node pushed onto `Walk::supertypes`
  supertypes: u8,
  error: bool,
  /// A switch's body, whose `case` groups are scopes: whether one is open
  switch_group: Option<bool>,
  /// A field, the container of what follows its name: not of its modifiers and type, which its declaration shares
  deferred: Option<u32>,
}

struct Container {
  decl: u32,
  holder: Holder,
  /// The depth of `Walk::supertypes` when it was entered, its own entry included
  supertypes: usize,
}

/// A member reference whose receiver may have a type (`Reference::receiver_type`)
enum TypedReceiver {
  Bound { serial: u32, type_path: Typed },
  Property,
}

/// The facts of a declaration that the walk reads before it declares it (`extract::JvmFields`)
#[derive(Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a declaration")]
struct Fields {
  signature: Option<Signature>,
  overrides: bool,
  overridable: bool,
  test: bool,
  entry_point: bool,
  unstable: bool,
  supertypes: Vec<Vec<u32>>,
  value_type: Option<Typed>,
  constructors: Option<Vec<Constructor>>,
}

/// Where a declaration lies: its first and last line and the line of its name
struct Span {
  start: u32,
  end: u32,
  name: u32,
}

struct Walk<'t> {
  tree: &'t Tree,
  source: &'t [u8],
  lines: Lines,
  out: SummaryBuilder<'t>,
  bindings: Bindings,
  frames: Vec<Frame>,
  marks: HashMap<u32, Mark>,
  /// (frame index, name, type): bindings that become visible when that frame is left
  pending: Vec<(usize, u32, Option<Typed>)>,
  containers: Vec<Container>,
  /// The supertypes whose members the declarations at this level override; a class pushes its own, a method an empty
  /// list, an anonymous class the type it constructs, an enum constant its enum
  supertypes: Vec<Vec<Vec<u32>>>,
  error_depth: u32,
  /// The error elements of the code (not of a doc comment), in document order
  errors: Vec<u32>,
  /// The `FIELD` or `LOCAL_VARIABLE` with a type met last: the head of the later ones of its declaration
  last_head: Option<NodeId>,
}

impl<'t> Walk<'t> {
  fn new(tree: &'t Tree) -> Self {
    let source = tree.source();
    let mut walk = Self {
      tree,
      source,
      lines: Lines::new(source),
      out: SummaryBuilder::new(),
      bindings: Bindings::default(),
      frames: Vec::new(),
      marks: HashMap::new(),
      pending: Vec::new(),
      containers: Vec::new(),
      supertypes: Vec::new(),
      error_depth: 0,
      errors: Vec::new(),
      last_head: None,
    };
    walk.errors = tree.errors().map(|(id, _)| id).filter(|id| !walk.in_doc_comment(*id)).map(|id| id.0).collect();
    walk
  }

  fn run(&mut self) {
    let root = self.tree.root();
    let end = self.tree.subtree_end(root).0;
    let mut id = root.0;
    while id < end {
      while self.frames.last().is_some_and(|it| it.end <= id) {
        self.leave();
      }
      let node = NodeId(id);
      if self.tree.is_leaf(node) {
        self.token(node);
        id += 1;
        continue;
      }
      let end = self.tree.subtree_end(node).0;
      self.frames.push(Frame {
        end,
        scopes: 0,
        container: false,
        supertypes: 0,
        error: false,
        switch_group: None,
        deferred: None,
      });
      let mark = self.marks.remove(&id);
      id = if self.enter(node, mark) { id + 1 } else { end };
    }
    while !self.frames.is_empty() {
      self.leave();
    }
  }

  fn leave(&mut self) {
    let Some(frame) = self.frames.pop() else { return };
    let index = self.frames.len();
    for _ in 0..frame.scopes {
      self.bindings.pop();
    }
    while self.pending.last().is_some_and(|(at, ..)| *at >= index) {
      if let Some((_, name, type_path)) = self.pending.pop() {
        self.bindings.bind(name, type_path);
      }
    }
    if frame.container {
      self.containers.pop();
    }
    for _ in 0..frame.supertypes {
      self.supertypes.pop();
    }
    if frame.error {
      self.error_depth -= 1;
    }
  }

  /// A leaf: a name in an error element is a value, the `{` of an anonymous class opens its body, the name of a field
  /// makes it the container
  fn token(&mut self, id: NodeId) {
    match self.tree.token_kind(id) {
      Some(T::IDENTIFIER) if self.frames.last().is_some_and(|it| it.deferred.is_some()) => {
        if let Some(decl) = self.frame().deferred.take() {
          self.enter_container(decl, Holder::Leaf);
        }
      }
      Some(T::IDENTIFIER) if self.parent_kind(id) == Some(N::ERROR_ELEMENT) => {
        if let Some(text) = self.text(id) {
          let line = self.line_at(id);
          self.reference(text, line, Role::Value, None, None);
        }
      }
      Some(T::LBRACE) if self.parent_kind(id) == Some(N::ANONYMOUS_CLASS) => {
        self.bindings.push(true);
        // the frame of the anonymous class, which the `{` has none of its own
        self.frame().scopes += 1;
      }
      _ => {}
    }
  }

  fn enter(&mut self, id: NodeId, mark: Option<Mark>) -> bool {
    // a composite of a token type: a doc comment
    let Some(kind) = self.kind(id) else { return false };
    if matches!(mark, Some(Mark::Scope)) {
      self.push_scope(true);
    }
    match kind {
      N::ERROR_ELEMENT => {
        if self.error_depth == 0 {
          let bytes = u32::try_from(self.tree.range(id).len()).unwrap_or(u32::MAX).max(1);
          self.out.error_bytes = self.out.error_bytes.saturating_add(bytes);
        }
        self.error_depth += 1;
        self.frame().error = true;
        true
      }
      N::PACKAGE_STATEMENT => {
        self.import_statement(id);
        if let Some(path) = self.child(id, N::JAVA_CODE_REFERENCE).and_then(|it| self.path_of(it)) {
          self.out.package = Some(path.join("."));
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
      N::METHOD if self.child(id, N::TYPE).is_none() => {
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
        let name = self.name_leaf(id);
        self.bind_name(name, None, false);
        true
      }
      N::RESOURCE_VARIABLE => {
        let name = self.name_leaf(id);
        self.bind_name(name, None, true);
        true
      }
      N::CODE_BLOCK => {
        if matches!(self.parent_kind(id), Some(N::SWITCH_STATEMENT | N::SWITCH_EXPRESSION)) {
          self.frame().switch_group = Some(false);
        } else {
          self.push_scope(true);
        }
        true
      }
      N::FOR_STATEMENT | N::FOREACH_STATEMENT | N::FOREACH_PATTERN_STATEMENT | N::CATCH_SECTION | N::SWITCH_LABELED_RULE | N::LAMBDA_EXPRESSION => {
        self.push_scope(true);
        true
      }
      N::TRY_STATEMENT => {
        if self.child(id, N::RESOURCE_LIST).is_some() {
          self.push_scope(true);
        }
        true
      }
      N::SWITCH_LABEL_STATEMENT => {
        self.case_group(id);
        true
      }
      N::INSTANCE_OF_EXPRESSION => {
        let operand = self.composite_children(id).next();
        if let Some(operand) = operand {
          self.smart_cast(operand);
        }
        true
      }
      N::TYPE_CAST_EXPRESSION => {
        if let Some(operand) = self.composite_children(id).filter(|it| self.kind(*it) != Some(N::TYPE)).last() {
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
        if let Some(name) = self.child(id, N::JAVA_CODE_REFERENCE) {
          let mark = Mark::Role {
            role: Role::Type,
            shape: None,
            chain: Role::Value,
          };
          self.marks.insert(name.0, mark);
        }
        true
      }
      N::THIS_EXPRESSION | N::SUPER_EXPRESSION => {
        if let Some(qualifier) = self.child(id, N::JAVA_CODE_REFERENCE) {
          self.marks.insert(qualifier.0, Mark::Values);
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
      Some(T::INTERFACE_KEYWORD) if self.has_token(id, T::AT) => Holder::Annotation,
      Some(T::INTERFACE_KEYWORD) => Holder::Interface,
      Some(T::ENUM_KEYWORD) => Holder::Enum,
      Some(T::CLASS_KEYWORD | T::RECORD_KEYWORD) => Holder::Class,
      _ => Holder::Leaf,
    };
    let name = self.name_leaf(id);
    let supertypes = self.supertypes_of(id);
    let mut fields = Fields {
      unstable: self.unstable(id),
      supertypes: supertypes.clone(),
      ..Fields::default()
    };
    self.push_supertypes(supertypes);
    if self.bindings.is_local() {
      self.bind_name(name, None, false);
      self.push_scope(false);
      return;
    }
    if let Some(name) = name {
      let modifiers = self.child(id, N::MODIFIER_LIST);
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
      self.enter_container(decl, own);
    }
    self.push_scope(false);
  }

  /// A method or an annotation's element: a function; a method's parameters and body are local
  fn method(&mut self, id: NodeId, element: bool) {
    let name = self.name_leaf(id);
    let mut fields = self.function_fields(id, name, element);
    fields.unstable = self.unstable(id);
    if !element {
      self.push_supertypes(Vec::new());
    }
    if self.bindings.is_local() {
      self.bind_name(name, None, false);
      if !element {
        self.push_scope(true);
      }
      return;
    }
    if let Some(name) = name {
      let modifiers = self.child(id, N::MODIFIER_LIST);
      let traits = self.traits(modifiers, DeclKind::Function, Holder::Leaf, false);
      let span = self.member_span(id, name);
      let header = self.header(None, id, !element);
      let decl = self.declare(name, traits, &span, fields, Some(header));
      self.enter_container(decl, Holder::Leaf);
    }
    if !element {
      self.push_scope(true);
    }
  }

  /// The signature, the modifiers and the `Override` references of a method or an annotation's element
  fn function_fields(&mut self, id: NodeId, name: Option<NodeId>, element: bool) -> Fields {
    let modifiers = self.child(id, N::MODIFIER_LIST);
    let (overrides, test, overridable) = if element {
      (false, false, false)
    } else {
      let annotations = self.annotation_paths(modifiers);
      let overrides = annotations.iter().any(|path| matches!(path.as_slice(), ["Override"] | ["java", "lang", "Override"]));
      let test = annotations.iter().any(|path| path.last().is_some_and(|last| TEST_ANNOTATIONS.contains(last)));
      let open = match self.holder() {
        Some(Holder::Interface) => true,
        Some(Holder::Class | Holder::Enum) => !self.in_final_class(id),
        _ => false,
      };
      let has = |token| self.modifier(modifiers, token);
      let overridable = open && !self.bindings.is_local() && !has(T::FINAL_KEYWORD) && !has(T::PRIVATE_KEYWORD) && !has(T::STATIC_KEYWORD);
      (overrides, test, overridable)
    };
    // a local function is no declaration: only the shape of an override needs its parameters
    let signature = (overrides || !self.bindings.is_local()).then(|| self.signature(id));
    if let (true, Some(name)) = (overrides, name) {
      self.override_references(name, signature.as_ref());
    }
    let text = name.and_then(|it| self.text(it));
    let test = test || self.junit3_test(text, signature.as_ref());
    let entry_point = !element && !self.bindings.is_local() && text == Some("main") && signature.as_ref().is_some_and(|it| self.entry_point(id, modifiers, it));
    Fields {
      signature,
      overrides,
      overridable,
      test,
      entry_point,
      ..Fields::default()
    }
  }

  /// A constructor, a scope of its parameters, whose name is no reference: a constructor of the class around it. The
  /// canonical one of a record (JLS 8.10.4), compact or declared again with the names and types of its components,
  /// every type known, replaces the one of the record's header
  fn constructor(&mut self, id: NodeId) {
    let class = self.tree.parent(id).filter(|it| self.kind(*it) == Some(N::CLASS));
    let record = class.filter(|it| self.class_keyword(*it) == Some(T::RECORD_KEYWORD));
    let header = record.map(|it| self.signature(it));
    let compact = self.child(id, N::PARAMETER_LIST).is_none();
    let declared = if compact { header.clone() } else { Some(self.signature(id)) };
    if let Some(signature) = declared {
      let canonical = compact || header.is_some_and(|header| header == signature && signature.parameters.iter().all(|it| it.type_ref.is_some()));
      let reach = self.reach_of(self.child(id, N::MODIFIER_LIST), true, false);
      if !self.bindings.is_local()
        && let Some(decl) = self.containers.last().map(|it| it.decl)
      {
        self.out.add_constructor(decl, Constructor { signature, reach }, canonical);
      }
    }
    self.push_scope(true);
  }

  /// A field, a local variable: a local binds its name once left, with its type
  fn variable(&mut self, id: NodeId) {
    let head = self.head_of(id);
    let name = self.name_leaf(id);
    let initializer = self.initializer(id);
    let value_type = if self.has_token(id, T::LBRACKET) {
      // `Foo a[]`: an array, of no type here
      None
    } else {
      self.value_type(self.child(head, N::TYPE), initializer)
    };
    if self.bindings.is_local() {
      self.bind_name(name, value_type, true);
      return;
    }
    let Some(name) = name else { return };
    let modifiers = self.child(head, N::MODIFIER_LIST);
    let traits = self.traits(modifiers, DeclKind::Value, Holder::Leaf, false);
    let fields = Fields {
      unstable: self.annotation_paths(modifiers).iter().any(|path| self.out.unstable_annotation(path)),
      value_type,
      ..Fields::default()
    };
    // a declarator starts at its name, after the modifiers and the type of its declaration; a later one at a doc comment
    // above it too (`int a, /** b */ b;`)
    let start = self.tree.range(name).start;
    let mut start_line = self.lines.line(start);
    if head != id {
      for child in self.tree.children(id).take_while(|it| *it != name).filter(|it| self.is_doc_comment(*it)) {
        start_line = start_line.min(self.line_at(child));
      }
    }
    let span = Span {
      start: start_line,
      end: self.end_line(start, self.own_range(id).end),
      name: self.lines.line(start),
    };
    let header = self.header((head != id).then_some(head), id, false);
    let decl = self.declare(name, traits, &span, fields, Some(header));
    self.frame().deferred = Some(decl);
  }

  /// An enum constant, whose body is local and overrides the members of its enum
  fn enum_constant(&mut self, id: NodeId) {
    let name = self.name_leaf(id);
    let fields = Fields {
      unstable: self.unstable(id),
      ..Fields::default()
    };
    let supertypes = self.enum_supertypes();
    self.push_supertypes(supertypes);
    if self.bindings.is_local() {
      self.bind_name(name, None, false);
      return;
    }
    if let Some(name) = name {
      let traits = self.traits(self.child(id, N::MODIFIER_LIST), DeclKind::EnumEntry, Holder::Leaf, false);
      let span = self.member_span(id, name);
      let decl = self.declare(name, traits, &span, fields, None);
      self.enter_container(decl, Holder::Leaf);
    }
    if let Some(body) = self.child(id, N::ENUM_CONSTANT_INITIALIZER) {
      self.marks.insert(body.0, Mark::Scope);
    }
  }

  /// A component of a record: a public field (and its accessor), a vararg one too (`int... xs`, which tree-sitter's
  /// walk read as a field of the package without a type); in a local record a parameter
  fn record_component(&mut self, id: NodeId) {
    let name = self.name_leaf(id);
    let ty = self.child(id, N::TYPE);
    let vararg = ty.is_some_and(|it| self.has_token(it, T::ELLIPSIS));
    if self.bindings.is_local() {
      let value_type = if vararg { None } else { self.value_type(ty, None) };
      self.bind_name(name, value_type, vararg);
      return;
    }
    let Some(name) = name else { return };
    let fields = Fields {
      unstable: self.unstable(id),
      ..Fields::default()
    };
    let range = self.own_range(id);
    let span = Span {
      start: self.lines.line(range.start),
      end: self.end_line(range.start, range.end),
      name: self.line_at(name),
    };
    let header = self.header(None, id, false);
    self.declare(name, (DeclKind::Value, Reach::Public, MemberKind::Instance), &span, fields, Some(header));
  }

  /// A parameter of a method or a lambda binds its name with its declared type (a vararg and `Foo a[]` none); the
  /// variable of a `for` and the parameter of a `catch` without
  fn parameter(&mut self, id: NodeId) {
    let typed = self.parent_kind(id) == Some(N::PARAMETER_LIST) && !self.has_token(id, T::LBRACKET);
    let ty = self.child(id, N::TYPE);
    let vararg = ty.is_some_and(|it| self.has_token(it, T::ELLIPSIS));
    let value_type = if typed && !vararg { self.value_type(ty, None) } else { None };
    let name = self.name_leaf(id);
    self.bind_name(name, value_type, false);
  }

  /// Binds the name `name` in the current scope, `late` when the current node is left
  fn bind_name(&mut self, name: Option<NodeId>, type_path: Option<Typed>, late: bool) {
    let Some(text) = name.and_then(|it| self.text(it)) else { return };
    let name = self.out.intern(text);
    if late {
      let index = self.frames.len() - 1;
      self.pending.push((index, name, type_path));
    } else {
      self.bindings.bind(name, type_path);
    }
  }

  /// A `case` group of a switch's body (labels, then statements) is a scope until the next group
  fn case_group(&mut self, id: NodeId) {
    let Some(parent) = self.frames.len().checked_sub(2) else { return };
    let Some(open) = self.frames[parent].switch_group else { return };
    let previous = std::iter::successors(self.tree.previous_sibling(id), |it| self.tree.previous_sibling(*it)).find(|it| !self.is_trivia(*it));
    if previous.is_some_and(|it| self.kind(it) == Some(N::SWITCH_LABEL_STATEMENT)) {
      return;
    }
    if open {
      self.bindings.pop();
      self.frames[parent].scopes -= 1;
    }
    self.bindings.push(true);
    self.frames[parent].scopes += 1;
    self.frames[parent].switch_group = Some(true);
  }

  // jscpd:ignore-start: the port of the JVM rules of extract.rs and java.rs, which go with tree-sitter's Java and Kotlin
  /// The kind, reach and member kind of a declaration of `kind` with these modifiers, given what it is to its own
  /// members and whether it is a `class` (only a class without `static` is inner)
  fn traits(&self, modifiers: Option<NodeId>, kind: DeclKind, own: Holder, class: bool) -> (DeclKind, Reach, MemberKind) {
    let has = |token| self.modifier(modifiers, token);
    let holder = self.holder();
    let in_interface = matches!(holder, Some(Holder::Interface | Holder::Annotation));
    // only a class without `static` is inner; enums, records, interfaces and annotations are static
    let nested_type = own != Holder::Leaf && (!class || has(T::STATIC_KEYWORD));
    let mut kind = kind;
    if kind == DeclKind::Value && (in_interface || (has(T::STATIC_KEYWORD) && has(T::FINAL_KEYWORD))) {
      kind = DeclKind::Constant;
    }
    let member = match holder {
      None => MemberKind::TopLevel,
      Some(_) if kind == DeclKind::EnumEntry || kind == DeclKind::Constant || nested_type || has(T::STATIC_KEYWORD) => MemberKind::Static,
      Some(_) if in_interface && own != Holder::Leaf => MemberKind::Static,
      Some(_) => MemberKind::Instance,
    };
    let reach = self.reach_of(modifiers, holder.is_some(), in_interface || kind == DeclKind::EnumEntry);
    (kind, reach, member)
  }

  /// The reach of these modifiers: `public` and `protected` are public (and so is a member of an interface or an enum
  /// constant without them), `private` of the container (of the file at the top level), none of the package
  fn reach_of(&self, modifiers: Option<NodeId>, member: bool, public: bool) -> Reach {
    let has = |token| self.modifier(modifiers, token);
    if public || has(T::PUBLIC_KEYWORD) || has(T::PROTECTED_KEYWORD) {
      Reach::Public
    } else if has(T::PRIVATE_KEYWORD) {
      if member { Reach::Container } else { Reach::File }
    } else {
      Reach::Module
    }
  }

  fn declare(&mut self, name: NodeId, (kind, reach, member): (DeclKind, Reach, MemberKind), span: &Span, fields: Fields, header: Option<String>) -> u32 {
    let text = self.text(name).unwrap_or_default();
    let name = self.out.intern(text);
    let declaration = Declaration {
      signature: fields.signature,
      overrides: fields.overrides,
      overridable: fields.overridable,
      test: fields.test,
      entry_point: fields.entry_point,
      unstable: fields.unstable,
      supertypes: fields.supertypes,
      value_constructed: fields.value_type.as_ref().is_some_and(|it| it.constructed),
      value_type: fields.value_type.map(|it| it.path),
      constructors: fields.constructors,
      header,
      start_line: span.start,
      end_line: span.end,
      name_line: span.name,
      in_error: self.error_depth > 0,
      ..blank_declaration(name, kind, reach, member, self.container())
    };
    self.out.add_declaration(declaration)
  }
  // jscpd:ignore-end

  /// The lines of a class, a method or an enum constant: from its first token, or a doc comment above it, to its last
  fn member_span(&self, id: NodeId, name: NodeId) -> Span {
    let range = self.own_range(id);
    let mut start = self.lines.line(range.start);
    for child in self.tree.children(id) {
      if !self.is_trivia(child) {
        break;
      }
      if self.is_doc_comment(child) {
        start = start.min(self.line_at(child));
      }
    }
    Span {
      start,
      end: self.end_line(range.start, range.end),
      name: self.line_at(name),
    }
  }

  fn enter_container(&mut self, decl: u32, holder: Holder) {
    let supertypes = self.supertypes.len();
    self.containers.push(Container { decl, holder, supertypes });
    self.frame().container = true;
  }

  fn container(&self) -> Option<u32> {
    self.containers.last().map(|it| it.decl)
  }

  fn holder(&self) -> Option<Holder> {
    self.containers.last().map(|it| it.holder)
  }

  /// Whether an anonymous class or a local declaration lies between the current node and the declaration around it:
  /// each pushes onto `supertypes` (a lambda does not in Java)
  fn enclosed(&self) -> bool {
    let level = self.containers.last().map_or(0, |it| it.supertypes);
    self.supertypes.len() > level
  }

  fn push_scope(&mut self, body: bool) {
    self.bindings.push(body);
    self.frame().scopes += 1;
  }

  fn push_supertypes(&mut self, supertypes: Vec<Vec<u32>>) {
    self.supertypes.push(supertypes);
    self.frame().supertypes += 1;
  }

  fn frame(&mut self) -> &mut Frame {
    let index = self.frames.len() - 1;
    &mut self.frames[index]
  }

  // --- the JVM facts of a declaration ---

  /// `extends` and `implements` of a class, an enum or a record, `extends` of an interface, each as written without
  /// type arguments
  fn supertypes_of(&mut self, id: NodeId) -> Vec<Vec<u32>> {
    let mut paths = Vec::new();
    for list in self.tree.children(id).filter(|it| matches!(self.kind(*it), Some(N::EXTENDS_LIST | N::IMPLEMENTS_LIST))) {
      for reference in self.tree.children(list).filter(|it| self.kind(*it) == Some(N::JAVA_CODE_REFERENCE)) {
        if let Some(path) = self.path_of(reference) {
          paths.push(path);
        }
      }
    }
    paths.iter().map(|it| self.out.ids(it)).collect()
  }

  // jscpd:ignore-start: the port of the JVM rules of extract.rs and java.rs, which go with tree-sitter's Java and Kotlin
  /// The supertypes of the body of an enum constant: the path of the enum around it, its containers and its name; none
  /// outside an enum
  fn enum_supertypes(&self) -> Vec<Vec<u32>> {
    if self.holder() != Some(Holder::Enum) {
      return Vec::new();
    }
    let mut path = Vec::new();
    let mut current = self.container();
    while let Some(declaration) = current.and_then(|it| self.out.declarations.get(it as usize)) {
      path.push(declaration.name);
      current = declaration.container;
    }
    path.reverse();
    vec![path]
  }

  /// One `Override` reference per supertype of the class around the method, qualified by its path
  fn override_references(&mut self, name: NodeId, signature: Option<&Signature>) {
    let Some(supertypes) = self.supertypes.last().filter(|it| !it.is_empty()).cloned() else {
      return;
    };
    let Some(text) = self.text(name) else { return };
    let parameters = signature.map_or_else(Vec::new, |it| it.parameters.iter().map(|parameter| parameter.type_ref.clone()).collect());
    let shape = self.out.intern_shape(Shape::Override {
      function: true,
      receiver: None,
      parameters,
    });
    let line = self.line_at(name);
    for supertype in supertypes {
      self.reference(text, line, Role::Override, Some(supertype), Some(shape));
    }
  }
  // jscpd:ignore-end

  /// Whether a function of this name and signature is a `JUnit` 3 test of the class around it
  fn junit3_test(&self, name: Option<&str>, signature: Option<&Signature>) -> bool {
    let Some(name) = name else { return false };
    let class = self.container().and_then(|id| self.out.declarations.get(id as usize)).map(|it| self.out.name(it.name));
    let supertypes = self.supertypes.last().into_iter().flatten().filter_map(|path| path.last()).map(|&name| self.out.name(name));
    is_junit3_test(name, signature.is_some_and(|it| it.parameters.is_empty()), self.bindings.is_local(), class, supertypes)
  }

  /// `public static void main(String[] args)` of a type, `String... args` and `String args[]` too; `public` is implied
  /// only for a non-private method of an interface
  fn entry_point(&self, id: NodeId, modifiers: Option<NodeId>, signature: &Signature) -> bool {
    let holder = self.holder();
    let has = |token| self.modifier(modifiers, token);
    holder.is_some()
      && (has(T::PUBLIC_KEYWORD) || (holder == Some(Holder::Interface) && !has(T::PRIVATE_KEYWORD)))
      && has(T::STATIC_KEYWORD)
      && self.child(id, N::TYPE).is_some_and(|it| self.has_token(it, T::VOID_KEYWORD))
      && matches!(signature.parameters.as_slice(), [parameter] if self.out.type_is(parameter.type_ref.as_ref(), &["String"], 1))
  }

  /// Whether the class whose body holds the method is `final` or a record
  fn in_final_class(&self, id: NodeId) -> bool {
    let Some(class) = self.tree.parent(id).filter(|it| self.kind(*it) == Some(N::CLASS)) else {
      return false;
    };
    match self.class_keyword(class) {
      Some(T::RECORD_KEYWORD) => true,
      Some(T::CLASS_KEYWORD) => self.modifier(self.child(class, N::MODIFIER_LIST), T::FINAL_KEYWORD),
      _ => false,
    }
  }

  /// Whether an annotation of the declaration marks it as no stable API
  fn unstable(&self, id: NodeId) -> bool {
    self
      .annotation_paths(self.child(id, N::MODIFIER_LIST))
      .iter()
      .any(|path| self.out.unstable_annotation(path))
  }

  /// The names of the annotations among these modifiers, each as its path (`[org, junit, Test]`)
  fn annotation_paths(&self, modifiers: Option<NodeId>) -> Vec<Vec<&'t str>> {
    let Some(modifiers) = modifiers else { return Vec::new() };
    self
      .tree
      .children(modifiers)
      .filter(|it| self.kind(*it) == Some(N::ANNOTATION))
      .filter_map(|it| self.child(it, N::JAVA_CODE_REFERENCE))
      .filter_map(|it| self.path_of(it))
      .collect()
  }

  /// The parameters of a method or a constructor, the components of a record (`String... rest` a vararg, an array on
  /// the JVM); an annotation's element has none
  fn signature(&mut self, id: NodeId) -> Signature {
    let (list, item) = match self.kind(id) {
      Some(N::CLASS) => (self.child(id, N::RECORD_HEADER), N::RECORD_COMPONENT),
      Some(N::ANNOTATION_METHOD) => (None, N::PARAMETER),
      _ => (self.child(id, N::PARAMETER_LIST), N::PARAMETER),
    };
    let type_parameters = self.type_parameter_names(id);
    let items: Vec<NodeId> = list.map_or_else(Vec::new, |list| self.tree.children(list).filter(|it| self.kind(*it) == Some(item)).collect());
    let mut parameters = Vec::new();
    for parameter in items {
      let Some(name) = self.name_text(parameter) else { continue };
      let ty = self.child(parameter, N::TYPE);
      let vararg = ty.is_some_and(|it| self.has_token(it, T::ELLIPSIS));
      // `int[][] x`, `int x[]` and `int... x` are arrays on the JVM
      let after_name = self.tree.children(parameter).filter(|it| self.tree.token_kind(*it) == Some(T::LBRACKET)).count();
      let arrays = ty.map_or(0, |it| self.dimensions(it)) + after_name + usize::from(vararg);
      let arrays = u8::try_from(arrays).unwrap_or(u8::MAX);
      let type_ref = ty.and_then(|it| self.type_ref(it, arrays, &type_parameters));
      parameters.push(Parameter {
        name: self.out.intern(name),
        type_ref,
        optional: false,
        vararg,
      });
    }
    Signature { parameters }
  }

  /// The `[]` of a type, those of its element included: a `TYPE` nests one level per dimension
  fn dimensions(&self, ty: NodeId) -> usize {
    let mut count = 0;
    let mut current = Some(ty);
    while let Some(ty) = current {
      count += self.tree.children(ty).filter(|it| self.tree.token_kind(*it) == Some(T::LBRACKET)).count();
      current = self.child(ty, N::TYPE);
    }
    count
  }

  /// The names of the type parameters of a declaration (`<T, R>`)
  fn type_parameter_names(&self, id: NodeId) -> Vec<&'t str> {
    let Some(list) = self.child(id, N::TYPE_PARAMETER_LIST) else { return Vec::new() };
    self
      .tree
      .children(list)
      .filter(|it| self.kind(*it) == Some(N::TYPE_PARAMETER))
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
          names.push(self.out.intern(text));
        }
        continue;
      }
      let parent = match self.kind(node) {
        Some(N::JAVA_CODE_REFERENCE) => {
          let path = self.path_of(node)?;
          let (first, last) = (path.first()?, path.last()?);
          if type_parameters.contains(first) || self.out.name_id(first).is_some_and(|it| self.bindings.is_bound(it)) {
            return None;
          }
          names.push(self.out.intern(last));
          // the type arguments of the last name; those of a qualifier are of no name here
          match self.child(node, N::REFERENCE_PARAMETER_LIST) {
            Some(arguments) => arguments,
            None => continue,
          }
        }
        Some(N::ANNOTATION) | None => continue,
        Some(_) => node,
      };
      let start = stack.len();
      stack.extend(self.tree.children(parent).filter(|it| !self.is_trivia(*it)));
      stack[start..].reverse();
    }
    (!names.is_empty()).then_some(TypeRef { names, nullable: false, arrays })
  }

  /// The type a value is declared with, as written without type arguments (`java.util.List` of `java.util.List<T>`),
  /// not a type parameter, a primitive or an array; of a `var` the type its initializer constructs, a `new` without a
  /// body (an anonymous class is a type of its own)
  fn value_type(&mut self, ty: Option<NodeId>, initializer: Option<NodeId>) -> Option<Typed> {
    let ty = ty?;
    let constructed = self.has_token(ty, T::VAR_KEYWORD);
    let path = if constructed {
      let new = initializer.filter(|it| self.kind(*it) == Some(N::NEW_EXPRESSION) && self.child(*it, N::ANONYMOUS_CLASS).is_none() && !self.is_array_creation(*it))?;
      self.path_of(self.child(new, N::JAVA_CODE_REFERENCE)?)?
    } else {
      self.path_of(ty)?
    };
    if path.len() == 1 && self.out.name_id(path[0]).is_some_and(|it| self.bindings.is_bound(it)) {
      return None;
    }
    Some(Typed {
      path: self.out.ids(&path),
      constructed,
    })
  }

  /// The `FIELD` or `LOCAL_VARIABLE` whose modifiers and type a later one of the same declaration (`b` of `int a, b;`)
  /// shares: the one before it that has a type
  fn head_of(&mut self, id: NodeId) -> NodeId {
    if self.child(id, N::TYPE).is_some() {
      self.last_head = Some(id);
      return id;
    }
    // the head met last, unless a declaration in an initializer came between them
    if let Some(head) = self.last_head.filter(|it| self.tree.parent(*it) == self.tree.parent(id)) {
      return head;
    }
    let kind = self.kind(id);
    let mut current = id;
    while self.child(current, N::TYPE).is_none() {
      let previous = std::iter::successors(self.tree.previous_sibling(current), |it| self.tree.previous_sibling(*it))
        .find(|it| !self.is_trivia(*it) && self.tree.token_kind(*it) != Some(T::COMMA));
      match previous.filter(|it| self.kind(*it) == kind) {
        Some(previous) => current = previous,
        None => return id,
      }
    }
    current
  }

  /// The initializer of a variable: the expression after its `=`
  fn initializer(&self, id: NodeId) -> Option<NodeId> {
    self
      .tree
      .children(id)
      .skip_while(|it| self.tree.token_kind(*it) != Some(T::EQ))
      .find(|it| !self.tree.is_leaf(*it) && !self.is_trivia(*it))
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
    let prune = |node: NodeId| self.is_trivia(node) || self.kind(node) == Some(N::ANNOTATION) || (drop_parameters && self.kind(node) == Some(N::PARAMETER_LIST));
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
    let Some(callee) = self.child(id, N::REFERENCE_EXPRESSION) else { return };
    let shape = self.call_shape(self.child(id, N::EXPRESSION_LIST));
    if self.name_leaf(callee).is_some() {
      let mark = Mark::Role {
        role: Role::Call,
        shape,
        chain: Role::Value,
      };
      self.marks.insert(callee.0, mark);
      return;
    }
    let Some(keyword) = self
      .tree
      .children(callee)
      .find(|it| matches!(self.tree.token_kind(*it), Some(T::THIS_KEYWORD | T::SUPER_KEYWORD)))
    else {
      return;
    };
    let class = std::iter::successors(self.tree.parent(id), |it| self.tree.parent(*it))
      .find(|it| self.kind(*it) == Some(N::CLASS) && matches!(self.class_keyword(*it), Some(T::CLASS_KEYWORD | T::ENUM_KEYWORD | T::RECORD_KEYWORD)));
    let Some(class) = class else { return };
    let path = if self.tree.token_kind(keyword) == Some(T::THIS_KEYWORD) {
      self.name_text(class).map(|it| vec![it])
    } else if self.class_keyword(class) == Some(T::CLASS_KEYWORD) {
      self
        .child(class, N::EXTENDS_LIST)
        .and_then(|it| self.child(it, N::JAVA_CODE_REFERENCE))
        .and_then(|it| self.path_of(it))
    } else {
      None
    };
    let Some((name, qualifier)) = path.as_deref().and_then(<[&str]>::split_last) else {
      return;
    };
    let qualifier = (!qualifier.is_empty()).then(|| self.out.ids(qualifier));
    let line = self.line_at(keyword);
    self.reference(name, line, Role::Call, qualifier, shape);
  }

  /// `new Type(…)` calls the type with the shape of its arguments, which are of the code around an anonymous class; an
  /// anonymous class declares nothing, its methods override those of the type. An array's type is a type
  fn creation(&mut self, id: NodeId) {
    let anonymous = self.child(id, N::ANONYMOUS_CLASS);
    let owner = anonymous.unwrap_or(id);
    let constructed = self.child(owner, N::JAVA_CODE_REFERENCE);
    if let Some(constructed) = constructed.filter(|_| !self.is_array_creation(id)) {
      let shape = self.call_shape(self.child(owner, N::EXPRESSION_LIST));
      let mark = Mark::Role {
        role: Role::Call,
        shape,
        chain: Role::Type,
      };
      self.marks.insert(constructed.0, mark);
    }
    if anonymous.is_some() {
      let path = constructed.and_then(|it| self.path_of(it));
      let supertypes = path.iter().map(|it| self.out.ids(it)).collect();
      self.push_supertypes(supertypes);
    }
  }

  /// `Type::method`, `object::method`; `Type::new` names no method
  fn method_reference(&mut self, id: NodeId) {
    let Some(name) = self.name_leaf(id) else { return };
    let Some(object) = self.tree.children(id).find(|it| !self.is_trivia(*it)) else { return };
    let qualifier = self.qualifier_of(object);
    if let Some(text) = self.text(name) {
      let line = self.line_at(name);
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
      self.marks.insert(qualifier.0, Mark::Values);
    }
    self.member(id, role, shape);
  }

  // jscpd:ignore-start: the port of the JVM rules of extract.rs and java.rs, which go with tree-sitter's Java and Kotlin
  /// The reference of the name of `id`, of `role` and `shape`, qualified by the path of its qualifier, with the type
  /// of its receiver
  fn member(&mut self, id: NodeId, role: Role, shape: Option<u32>) {
    let Some(name) = self.name_leaf(id) else { return };
    let Some(text) = self.text(name) else { return };
    let line = self.line_at(name);
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
    let before = self.out.references.len();
    self.reference(text, line, role, Some(qualifier), shape);
    if let Some(receiver) = receiver.filter(|_| self.out.references.len() > before) {
      match receiver {
        TypedReceiver::Bound { serial, type_path } => self.out.bound_receiver(before, serial, type_path),
        TypedReceiver::Property => self.out.property_receiver(before),
      }
    }
  }

  /// What the receiver `object` of a member reference may have a type by: a bound name with a declared type, or, not
  /// enclosed, a property `[m]` or `[this, m]` of the qualifier
  fn typed_receiver(&self, object: NodeId, qualifier: &[u32]) -> Option<TypedReceiver> {
    if matches!(self.kind(object), Some(N::REFERENCE_EXPRESSION | N::JAVA_CODE_REFERENCE)) && self.qualifier(object).is_none() {
      let id = self.out.name_id(self.name_text(object)?)?;
      if self.bindings.is_bound(id) {
        let binding = self.bindings.binding(id)?;
        return Some(TypedReceiver::Bound {
          serial: binding.serial,
          type_path: binding.type_path.clone()?,
        });
      }
    }
    let this = self.out.name_id("this");
    let property = match qualifier {
      [_] => true,
      [first, _] => Some(*first) == this,
      _ => false,
    };
    (property && !self.enclosed()).then_some(TypedReceiver::Property)
  }

  /// The interned `Shape::Call` of the arguments `list`, `enclosed` by the walk
  fn call_shape(&mut self, list: Option<NodeId>) -> Option<u32> {
    let list = list?;
    let arguments = self.composite_children(list).map(|it| (None, self.argument(it))).collect();
    let enclosed = self.enclosed();
    Some(self.out.intern_shape(Shape::Call {
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
        let operand = self.composite_children(value).next();
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
    let Some(path) = self.path_of(operand) else { return };
    let name = match path.as_slice() {
      [name] | ["this", name] => *name,
      _ => return,
    };
    let Some(id) = self.out.name_id(name) else { return };
    if path.len() == 1
      && let Some(serial) = self.bindings.binding(id).map(|it| it.serial)
    {
      self.out.cast_binding(serial);
      return;
    }
    let container = self.container();
    self.out.cast_name(container, id);
  }
  // jscpd:ignore-end

  /// A reference, unless its name is `var` or a bare name bound around it
  fn reference(&mut self, name: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>, shape: Option<u32>) {
    if name.is_empty() || name == "var" {
      return;
    }
    let id = self.out.intern(name);
    if qualifier.is_none() && self.bindings.is_bound(id) {
      return;
    }
    let container = self.container();
    self.out.add_reference(id, line, role, qualifier, container, shape, Some(false));
  }

  /// The qualifier of a member of `object`: its path of names, or empty for an expression or a path from a local
  fn qualifier_of(&mut self, object: NodeId) -> Vec<u32> {
    let Some(path) = self.path_of(object) else { return Vec::new() };
    let ids = self.out.ids(&path);
    if ids.first().is_some_and(|it| self.bindings.is_bound(*it)) { Vec::new() } else { ids }
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
        Some(N::THIS_EXPRESSION) if self.child(node, N::JAVA_CODE_REFERENCE).is_none() => {
          reversed.push("this");
          break;
        }
        Some(N::SUPER_EXPRESSION) => {
          let Some(qualifier) = self.child(node, N::JAVA_CODE_REFERENCE) else {
            reversed.push("super");
            break;
          };
          node = qualifier;
        }
        Some(N::TYPE) => {
          let mut children = self.tree.children(node).filter(|it| !self.is_trivia(*it));
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

  /// The qualifier of a reference: the expression or the reference before its name, not an annotation of the type
  /// (`@A List`, `java.util.@A List`)
  fn qualifier(&self, id: NodeId) -> Option<NodeId> {
    self
      .tree
      .children(id)
      .take_while(|it| self.tree.token_kind(*it) != Some(T::IDENTIFIER))
      .find(|it| !self.tree.is_leaf(*it) && !self.is_trivia(*it) && !matches!(self.kind(*it), Some(N::REFERENCE_PARAMETER_LIST | N::ANNOTATION)))
  }

  // --- imports ---

  /// The lines of an import or package statement
  fn import_statement(&mut self, id: NodeId) {
    let range = self.own_range(id);
    let span = LineSpan {
      start: self.lines.line(range.start),
      end: self.end_line(range.start, range.end),
    };
    self.out.import_lines.push(span);
  }

  /// `import a.b.C;`, `import a.b.*;`, `import static a.B.c;`, `import static a.B.*;`: the import, and the `Import`
  /// reference to a name
  fn import(&mut self, id: NodeId) {
    self.import_statement(id);
    let reference = self.child(id, N::IMPORT_STATIC_REFERENCE).or_else(|| self.child(id, N::JAVA_CODE_REFERENCE));
    let Some(segments) = reference.and_then(|it| self.path_of(it)) else { return };
    let line = self.line_at(id);
    if self.has_token(id, T::ASTERISK) {
      self.out.add_import_all(&segments, false, line);
    } else if let Some((name, path)) = segments.split_last() {
      self.out.add_import_name(path, name, None, false, line);
      let qualifier = (!path.is_empty()).then(|| self.out.ids(path));
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

  /// The composite children, without doc comments
  fn composite_children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    self.tree.children(id).filter(|it| !self.tree.is_leaf(*it) && !self.is_trivia(*it))
  }

  /// The token of the first leaf child
  fn child_token(&self, id: NodeId) -> Option<T> {
    self.tree.children(id).find_map(|it| self.tree.token_kind(it))
  }

  fn has_token(&self, id: NodeId, token: T) -> bool {
    self.tree.children(id).any(|it| self.tree.token_kind(it) == Some(token))
  }

  /// Whether these modifiers have this keyword
  fn modifier(&self, modifiers: Option<NodeId>, token: T) -> bool {
    modifiers.is_some_and(|it| self.has_token(it, token))
  }

  /// The keyword of a `CLASS`: `class`, `interface` (also of `@interface`), `enum` or `record`
  fn class_keyword(&self, id: NodeId) -> Option<T> {
    self
      .tree
      .children(id)
      .filter_map(|it| self.tree.token_kind(it))
      .find(|it| matches!(it, T::CLASS_KEYWORD | T::INTERFACE_KEYWORD | T::ENUM_KEYWORD | T::RECORD_KEYWORD))
  }

  /// The name of a declaration or a reference: its `IDENTIFIER`
  fn name_leaf(&self, id: NodeId) -> Option<NodeId> {
    self.tree.children(id).find(|it| self.tree.token_kind(*it) == Some(T::IDENTIFIER))
  }

  fn name_text(&self, id: NodeId) -> Option<&'t str> {
    self.name_leaf(id).and_then(|it| self.text(it))
  }

  fn text(&self, id: NodeId) -> Option<&'t str> {
    std::str::from_utf8(self.source.get(self.tree.range(id))?).ok()
  }

  fn line_at(&self, id: NodeId) -> u32 {
    self.lines.line(self.tree.range(id).start)
  }

  fn end_line(&self, start: usize, end: usize) -> u32 {
    end_line(|offset| self.lines.line(offset), self.source, start, end)
  }

  /// Whitespace, a comment, a doc comment
  fn is_trivia(&self, id: NodeId) -> bool {
    self.tree.kind(id).token().is_some_and(T::is_whitespace_or_comment)
  }

  /// A comment that documents the declaration after it: `/**` (not `/**/`) or `///` (not `////`)
  fn is_doc_comment(&self, id: NodeId) -> bool {
    self
      .text(id)
      .is_some_and(|it| (it.starts_with("/**") && !it.starts_with("/**/")) || (it.starts_with("///") && !it.starts_with("////")))
  }

  /// Whether a node lies in a doc comment, a composite of a token type
  fn in_doc_comment(&self, id: NodeId) -> bool {
    // a doc comment holds tags, references and types; any other node is of the code
    let in_doc = |kind: N| kind.debug_name().starts_with("DOC_") || matches!(kind, N::JAVA_CODE_REFERENCE | N::REFERENCE_PARAMETER_LIST | N::TYPE | N::ERROR_ELEMENT);
    let mut current = self.tree.parent(id);
    while let Some(node) = current {
      match self.tree.kind(node).node() {
        None => return true,
        Some(kind) if in_doc(kind) => current = self.tree.parent(node),
        Some(_) => return false,
      }
    }
    false
  }

  /// A declaration's range from its first token to its last, without the comments at its edges and a separator
  fn own_range(&self, id: NodeId) -> std::ops::Range<usize> {
    self.tree.own_range(id, |it| self.is_trivia(it), |it| matches!(it, T::SEMICOLON | T::COMMA))
  }

  /// Whether the subtree of `id` holds an error element of the code
  fn has_error(&self, id: NodeId) -> bool {
    let end = self.tree.subtree_end(id).0;
    let index = self.errors.partition_point(|it| *it < id.0);
    self.errors.get(index).is_some_and(|it| *it < end)
  }

  /// `new int[n]`, `new Foo[] {…}`: an array, no call of a constructor
  fn is_array_creation(&self, id: NodeId) -> bool {
    self
      .tree
      .children(id)
      .any(|it| self.tree.token_kind(it) == Some(T::LBRACKET) || self.kind(it) == Some(N::ARRAY_INITIALIZER_EXPRESSION))
  }
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
