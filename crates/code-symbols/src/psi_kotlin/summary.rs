//! The summary of a Kotlin file from its PSI (`FileSummary`): one preorder walk over the tree of `kotlin-syntax`, by
//! `NodeKind` and `TokenKind`, with the rules the head of `extract.rs` states for every language and the parser-neutral
//! builders of `summary_builder`.
//!
//! - **Names in PSI**: a declared name is an `IDENTIFIER` token of its declaration (a class, a function, a parameter, a
//!   destructuring entry), never a `REFERENCE_EXPRESSION`; a reference is a `REFERENCE_EXPRESSION`, so a name that is no
//!   reference needs no claim, but for a label (`VALUE_ARGUMENT_NAME`, `LABEL_QUALIFIER`) and the segments of a path
//!   that its parent reads (`USER_TYPE`, a qualified expression, a callable reference). The tokens a parse error skipped
//!   are tokens of its `ERROR_ELEMENT`: no reference.
//! - **Locals are dropped**: a parameter, a type parameter, a local and the implicit `it` and `field` are bound in the
//!   scope of the function, lambda, class or block around them (`Bindings`); a declaration inside a body scope is a
//!   local. A local `val` and the names of a lambda's parameters and a `when` subject become visible when their node is
//!   left; a function parameter at once (its default values see it); a `for` variable in its body, not its range. A
//!   property with context or type parameters is a scope of its own; a `do`'s body shares its scope with its condition.
//! - **Overrides**, **calls**, **constructors**, **`enclosed`**: as `extract.rs` says. A class-like node pushes its
//!   supertypes before its members are walked, a function, a lambda and an anonymous function an empty list. A call is
//!   a `CALL_EXPRESSION`; `a.b(c)` is `DOT_QUALIFIED_EXPRESSION(a, CALL_EXPRESSION(b, (c)))`, so the qualified
//!   expression gives its selector's callee the role `Call` and the shape of that call. A class with an
//!   `ERROR_ELEMENT` in it has unknown constructors.
//! - **Lines**: of the file's `\n`s (`Lines` of the source), as `extract.rs` counts them; a declaration from its first
//!   token, the doc comments before it included (PSI binds a doc comment and the comments right before a declaration
//!   into it), to its last token, without the comments bound after it.
//! - **The file kind**: a summary is asked by language alone, without the file's name (`extract::summarize`), so
//!   `summarize` parses a file as a `.kt` and, when that gives an error element at the top level (a statement, which
//!   only a script has), as a `.kts` script too, and keeps the parse with fewer error bytes: a build script reads as a
//!   script, a file as the compiler reads it. A script's statements are its `SCRIPT`'s `BLOCK`, read as the top level.
//! - **Bounded**: the walk keeps its own stack of frames, the parse is bounded by the parser (`psi_kotlin`), the size
//!   by the caller (`extract::summarize_with_budget`).

// jscpd:ignore-start: until the tree-sitter Kotlin and Java go (`kotlin.rs`, `java.rs` and the `JvmHooks` of
// `extract.rs`), the walk there holds the same JVM rules; then this walk and `psi_java`'s are the two that share them
use crate::psi_kotlin::parse;
use crate::summary::{Argument, Constructor, DeclKind, Declaration, FileSummary, LineSpan, MemberKind, Parameter, Reach, Role, Shape, Signature, TypeRef};
use crate::summary_builder::{Bindings, SummaryBuilder, TEST_ANNOTATIONS, Typed, blank_declaration, end_line, is_junit3_test};
use kotlin_syntax::{FileKind, NodeId, NodeKind as N, TokenKind as K, Tree};
use psi_builder::document::Lines;

/// Fills `summary` (`summary_builder::empty_summary` of the file, its size within the limits) with the facts of the
/// Kotlin file `source`; whether it was read, else it stays `truncated`
pub(crate) fn summarize(source: &[u8], summary: &mut FileSummary) -> bool {
  let tree = parse_file(source);
  let mut walk = Walk::new(&tree, source);
  walk.run();
  walk.out.finish(summary, Some(|_| Reach::Public));
  true
}

/// The tree of a file whose kind the summary is not told: as a `.kt` file, else, when that has an error element at the
/// top level, the one of a `.kt` file and a `.kts` script with fewer error bytes
fn parse_file(source: &[u8]) -> Tree {
  let file = parse(source, FileKind::Kt);
  if !file.children(file.root()).any(|it| file.node_kind(it) == Some(N::ERROR_ELEMENT)) {
    return file;
  }
  let script = parse(source, FileKind::Kts);
  if error_bytes(&script) < error_bytes(&file) { script } else { file }
}

/// The bytes under the outermost error elements, one for an empty one (an expected token)
fn error_bytes(tree: &Tree) -> usize {
  let mut bytes = 0;
  let mut end = 0;
  for (id, _) in tree.errors() {
    if id.0 >= end {
      bytes += tree.range(id).len().max(1);
      end = tree.subtree_end(id).0;
    }
  }
  bytes
}

/// The functions whose lambda argument is a coroutine: a suspend call is allowed in it
const COROUTINE_BUILDERS: &[&str] = &[
  "runBlocking",
  "launch",
  "async",
  "coroutineScope",
  "supervisorScope",
  "withContext",
  "withTimeout",
  "withTimeoutOrNull",
  "runTest",
  "flow",
  "channelFlow",
  "callbackFlow",
  "produce",
  "timeoutRunBlocking",
];

/// The modifiers a header leaves out: those that the reach, `overridable` and `overrides` say, and those no use
/// depends on
const HEADER_DROPS: &[&str] = &[
  "private",
  "public",
  "protected",
  "internal",
  "abstract",
  "final",
  "open",
  "override",
  "lateinit",
  "inline",
  "tailrec",
  "external",
];

/// The coroutine context of the code a node holds (`Reference::coroutine`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
  /// A suspend call is allowed: a `suspend` function, the lambda of a coroutine builder
  Coroutine,
  /// It isn't: any other function, an initializer, an accessor, a class body
  Plain,
  /// A lambda that a suspend function type may take
  Unknown,
}

/// What a container is to the declarations in it, for their member kind and reach
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
  Class,
  Interface,
  /// An object or a companion: its members are static
  Object,
  Enum,
  Annotation,
  /// A function, an enum entry, a property, an accessor: nothing in it is a member
  Leaf,
}

/// The scope a declaration opens for its children
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opens {
  Nothing,
  /// Type parameters and constructor parameters; the members are still declarations
  Members,
  /// A function: everything inside is local
  Body,
}

/// What a parent decided about a child before the walk reaches it
#[derive(Debug, Clone, Copy)]
enum Mark {
  /// A name that is no reference
  Claimed,
  /// The role a call, a constructor invocation or a class literal gives a name or a type, with the shape of a call
  Role(Role, Option<u32>),
  /// A body whose declarations are locals (the body of an enum entry)
  Scope,
}

/// What `traits` says about a declaration
#[derive(Debug, Clone, Copy)]
struct Traits {
  kind: DeclKind,
  reach: Reach,
  member: MemberKind,
  receiver: Option<u32>,
  /// What the declaration is to its own members
  holder: Holder,
}

/// What the modifiers of a member say about overriding
#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a declaration")]
struct Modality {
  overrides: bool,
  overridable: bool,
  jvm_static: bool,
  jvm_field: bool,
  jvm_name: bool,
  lateinit: bool,
  test: bool,
  setter: Option<Reach>,
  receiver: Option<u32>,
}

/// The JVM facts of a declaration, before it is declared
#[derive(Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a declaration")]
struct JvmFields {
  signature: Option<Signature>,
  overrides: bool,
  overridable: bool,
  jvm_static: bool,
  jvm_field: bool,
  jvm_name: bool,
  lateinit: bool,
  test: bool,
  entry_point: bool,
  unstable: bool,
  setter: Option<Reach>,
  supertypes: Vec<Vec<u32>>,
  value_type: Option<Typed>,
  constructors: Option<Vec<Constructor>>,
}

/// A declaration node: its kind and the scope it opens
#[derive(Debug, Clone, Copy)]
struct DeclRule {
  kind: DeclKind,
  opens: Opens,
  /// A local's name becomes visible when the node is left (`val x = x`)
  late: bool,
  /// The name of a declaration without one in the source (`companion object`)
  default_name: Option<&'static str>,
}

const fn rule(kind: DeclKind, opens: Opens) -> DeclRule {
  DeclRule {
    kind,
    opens,
    late: false,
    default_name: None,
  }
}

/// One composite node on the path from the root to the node being walked
#[derive(Debug)]
struct Frame {
  scopes: u8,
  container: bool,
  /// The entries this node pushed onto `Walk::supertypes`
  supertypes: u8,
  /// It pushed its coroutine context onto `Walk::coroutines`
  coroutine: bool,
  error: bool,
  /// The first line of the doc comments above the next child
  leading: Option<u32>,
}

struct Container {
  decl: Option<u32>,
  holder: Holder,
  /// A function or an accessor, whose code sees no parameter of a primary constructor
  code: bool,
  /// The depth of `Walk::supertypes` when it was entered, its own entry included
  supertypes: usize,
}

/// A member reference whose receiver may have a type (`Reference::receiver_type`), decided when the walk is done
enum TypedReceiver {
  Bound { serial: u32, type_path: Typed },
  Property,
}

/// The state of one walk
struct Walk<'t> {
  tree: &'t Tree,
  source: &'t [u8],
  lines: Lines,
  /// The ids of the error elements, ascending
  errors: Vec<u32>,
  out: SummaryBuilder<'t>,
  frames: Vec<Frame>,
  /// (node, mark, index of the frame that set it)
  marks: Vec<(NodeId, Mark, usize)>,
  bindings: Bindings,
  /// (frame index, name, type): bindings that become visible when that frame is left
  pending: Vec<(usize, u32, Option<Typed>)>,
  containers: Vec<Container>,
  /// The supertypes whose members the declarations at this level override
  supertypes: Vec<Vec<Vec<u32>>>,
  coroutines: Vec<Context>,
  error_depth: u32,
  /// The leading line taken from the parent for the node being entered
  leading_now: Option<u32>,
  /// The paths of the annotations of the file (`@file:ApiStatus.Internal`)
  file_annotations: Vec<Vec<&'t str>>,
}

impl<'t> Walk<'t> {
  fn new(tree: &'t Tree, source: &'t [u8]) -> Self {
    Self {
      tree,
      source,
      lines: Lines::new(source),
      errors: tree.errors().map(|(id, _)| id.0).collect(),
      out: SummaryBuilder::new(),
      frames: Vec::new(),
      marks: Vec::new(),
      bindings: Bindings::default(),
      pending: Vec::new(),
      containers: Vec::new(),
      supertypes: Vec::new(),
      coroutines: Vec::new(),
      error_depth: 0,
      leading_now: None,
      file_annotations: Vec::new(),
    }
  }

  fn run(&mut self) {
    let tree = self.tree;
    let mut node = tree.root();
    'nodes: loop {
      let leaf = tree.is_leaf(node);
      let descend = if leaf {
        self.leaf(node);
        false
      } else {
        self.enter(node)
      };
      if descend && let Some(child) = tree.first_child(node) {
        node = child;
        continue;
      }
      if !leaf {
        self.leave();
      }
      loop {
        if let Some(next) = tree.next_sibling(node) {
          node = next;
          continue 'nodes;
        }
        let Some(parent) = tree.parent(node) else { break 'nodes };
        node = parent;
        self.leave();
      }
    }
  }

  /// A token: a doc comment extends the declaration after it
  fn leaf(&mut self, id: NodeId) {
    if matches!(self.tree.token_kind(id), Some(K::EOL_COMMENT | K::BLOCK_COMMENT)) && self.is_doc_comment(id) {
      let line = self.line_of(id);
      if let Some(frame) = self.frames.last_mut() {
        frame.leading = Some(frame.leading.map_or(line, |it| it.min(line)));
      }
    }
  }

  fn enter(&mut self, id: NodeId) -> bool {
    let mark = self.take_mark(id);
    let kind = self.tree.node_kind(id);
    let mut frame = Frame {
      scopes: 0,
      container: false,
      supertypes: 0,
      coroutine: false,
      error: false,
      leading: None,
    };
    if let Some(context) = kind.and_then(|it| self.coroutine(id, it)) {
      self.coroutines.push(context);
      frame.coroutine = true;
    }
    if kind == Some(N::ERROR_ELEMENT) {
      if self.error_depth == 0 {
        let bytes = u32::try_from(self.tree.range(id).len().max(1)).unwrap_or(u32::MAX);
        self.out.error_bytes = self.out.error_bytes.saturating_add(bytes);
      }
      self.error_depth += 1;
      frame.error = true;
    }
    // a composite of a token type: a doc comment (KDoc)
    let Some(kind) = kind else {
      self.leading_now = None;
      let line = self.line_of(id);
      if let Some(parent) = self.frames.last_mut() {
        parent.leading = Some(parent.leading.map_or(line, |it| it.min(line)));
      }
      self.frames.push(frame);
      return false;
    };
    self.leading_now = self.frames.last_mut().and_then(|it| it.leading.take());
    self.frames.push(frame);
    if matches!(mark, Some(Mark::Scope)) {
      self.push_scope(true);
    }
    match kind {
      N::PACKAGE_DIRECTIVE => {
        self.package(id);
        false
      }
      N::FILE_ANNOTATION_LIST => {
        self.file_annotation_list(id);
        true
      }
      N::IMPORT_DIRECTIVE => {
        self.import(id);
        false
      }
      N::REFERENCE_EXPRESSION => {
        self.name(id, mark);
        false
      }
      N::VALUE_ARGUMENT_NAME | N::LABEL_QUALIFIER => false,
      N::DOT_QUALIFIED_EXPRESSION | N::SAFE_ACCESS_EXPRESSION => {
        self.qualified(id, mark);
        true
      }
      N::CALLABLE_REFERENCE_EXPRESSION => {
        self.callable_reference(id, mark);
        true
      }
      N::CLASS_LITERAL_EXPRESSION => {
        // `Foo::class` and `Array<String>::class` name a type
        let receiver = self.first_composite(id).map(|it| self.type_arguments_callee(it).unwrap_or(it));
        if let Some(receiver) = receiver.filter(|it| self.is(*it, N::REFERENCE_EXPRESSION)) {
          self.mark(receiver, Mark::Role(Role::Type, None));
        }
        true
      }
      N::USER_TYPE => {
        if !self.tree.parent(id).is_some_and(|it| self.is(it, N::USER_TYPE)) {
          self.path(id, mark);
        }
        true
      }
      N::CALL_EXPRESSION => {
        if let Some(callee) = self.first_composite(id).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION) && !self.is_marked(*it)) {
          let shape = self.call_shape(id);
          self.mark(callee, Mark::Role(Role::Call, Some(shape)));
        }
        true
      }
      N::BINARY_EXPRESSION => {
        self.infix(id);
        true
      }
      N::THIS_EXPRESSION | N::SUPER_EXPRESSION => {
        self.label_type(id);
        true
      }
      N::CLASS => {
        self.declaration(id, &rule(DeclKind::Type, Opens::Members));
        true
      }
      N::OBJECT_DECLARATION => {
        if self.tree.parent(id).is_some_and(|it| self.is(it, N::OBJECT_LITERAL)) {
          self.object_literal(id);
        } else if self.modifier_texts(id).contains(&"companion") {
          let decl = DeclRule {
            default_name: Some("Companion"),
            ..rule(DeclKind::Type, Opens::Nothing)
          };
          self.declaration(id, &decl);
        } else {
          self.declaration(id, &rule(DeclKind::Type, Opens::Nothing));
        }
        true
      }
      N::TYPEALIAS => {
        self.declaration(id, &rule(DeclKind::TypeAlias, Opens::Members));
        true
      }
      N::FUN => {
        if self.name_token(id).is_some() {
          self.declaration(id, &rule(DeclKind::Function, Opens::Body));
        } else {
          // an anonymous function, as a lambda
          self.push_scope(true);
          self.push_supertypes(Vec::new());
        }
        true
      }
      N::PROPERTY => {
        if self.tree.parent(id).is_some_and(|it| self.is(it, N::WHEN)) {
          // `when (val x = …)`: visible in the entries
          self.pend_names(id, self.frames.len() - 1, None);
        } else {
          let decl = DeclRule {
            late: true,
            ..rule(DeclKind::Value, Opens::Nothing)
          };
          self.declaration(id, &decl);
          // `context(c: Ctx) val x get() = c.foo`, `val <T> T.x`: its context and type parameters are names of the
          // property alone, as a function's are of the function
          if self.child(id, N::TYPE_PARAMETER_LIST).is_some() || self.child(id, N::MODIFIER_LIST).is_some_and(|it| self.child(it, N::CONTEXT_PARAMETER_LIST).is_some()) {
            self.push_scope(false);
          }
        }
        true
      }
      N::DESTRUCTURING_DECLARATION => {
        // one of a lambda's or a `for`'s parameter binds its names
        if !self.tree.parent(id).is_some_and(|it| self.is(it, N::VALUE_PARAMETER)) {
          let decl = DeclRule {
            late: true,
            ..rule(DeclKind::Value, Opens::Nothing)
          };
          self.declaration(id, &decl);
        }
        true
      }
      N::ENUM_ENTRY => {
        self.declaration(id, &rule(DeclKind::EnumEntry, Opens::Nothing));
        // the members of its body override those of the enum, as locals
        if let Some(body) = self.child(id, N::CLASS_BODY) {
          self.mark(body, Mark::Scope);
        }
        true
      }
      N::VALUE_PARAMETER => {
        self.parameter(id);
        true
      }
      N::TYPE_CONSTRAINT => {
        // `where T : Bound` names the type parameter `T`
        if let Some(subject) = self.first_composite(id).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION)) {
          self.mark(subject, Mark::Role(Role::Type, None));
        }
        true
      }
      N::TYPE_PARAMETER => {
        if let Some(name) = self.name_token(id) {
          self.bind_token(name, None);
        }
        true
      }
      N::LAMBDA_EXPRESSION => {
        self.push_scope(true);
        self.bind_text("it");
        self.push_supertypes(Vec::new());
        true
      }
      N::PROPERTY_ACCESSOR => {
        self.accessor(id);
        true
      }
      N::SECONDARY_CONSTRUCTOR => {
        if let Some(signature) = self.signature(id) {
          let reach = reach(&self.modifier_texts(id), true);
          self.add_constructor(Constructor { signature, reach });
        }
        self.push_scope(true);
        true
      }
      N::CONSTRUCTOR_DELEGATION_CALL => {
        self.constructor_delegation(id);
        true
      }
      // the body of a `do`: its locals are the `do`'s, which its condition sees
      N::BODY if self.parent_is(id, N::DO_WHILE) => true,
      N::BODY => {
        self.push_scope(true);
        // the variable of a `for` is a name of its body, not of its range (`for (node in node.children)`)
        if let Some(variable) = self.tree.parent(id).filter(|it| self.is(*it, N::FOR)).and_then(|it| self.child(it, N::VALUE_PARAMETER)) {
          for name in self.parameter_names(variable) {
            self.bind_token(name, None);
          }
        }
        true
      }
      // the blocks of a `try` and a `finally` (that of a `catch` is in its `CATCH`)
      N::BLOCK if self.parent_is(id, N::TRY) || self.parent_is(id, N::FINALLY) => {
        self.push_scope(true);
        true
      }
      N::CLASS_INITIALIZER | N::THEN | N::ELSE | N::WHEN_ENTRY | N::FOR | N::CATCH | N::WHEN | N::DO_WHILE => {
        self.push_scope(true);
        true
      }
      N::IS_EXPRESSION | N::BINARY_WITH_TYPE => {
        if let Some(operand) = self.first_composite(id) {
          let path = self.path_of(operand);
          self.smart_cast(path);
        }
        true
      }
      N::WHEN_CONDITION_IS_PATTERN => {
        self.type_test(id);
        true
      }
      _ => true,
    }
  }

  fn leave(&mut self) {
    let Some(frame) = self.frames.pop() else { return };
    let index = self.frames.len();
    while self.marks.last().is_some_and(|it| it.2 >= index) {
      self.marks.pop();
    }
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
    if frame.coroutine {
      self.coroutines.pop();
    }
    if frame.error {
      self.error_depth -= 1;
    }
  }

  /// The context a node opens for the references below it: a function by its `suspend`, an accessor, a constructor,
  /// an initializer and a class body none; a lambda when a coroutine builder takes it, none for `sequence` and
  /// `iterator` (whose restricted scope calls no foreign suspend function), unknown for any other callee and in an
  /// anonymous function
  fn coroutine(&self, id: NodeId, kind: N) -> Option<Context> {
    match kind {
      N::FUN if self.name_token(id).is_none() => Some(Context::Unknown),
      N::FUN if self.modifier_texts(id).contains(&"suspend") => Some(Context::Coroutine),
      N::FUN | N::PROPERTY_ACCESSOR | N::SECONDARY_CONSTRUCTOR | N::CLASS_INITIALIZER | N::CLASS_BODY => Some(Context::Plain),
      N::LAMBDA_EXPRESSION => Some(match self.lambda_callee(id) {
        Some(callee) if COROUTINE_BUILDERS.contains(&callee) => Context::Coroutine,
        Some("sequence" | "iterator") => Context::Plain,
        _ => Context::Unknown,
      }),
      _ => None,
    }
  }

  /// The name of the function a lambda is an argument of: `launch` of `scope.launch { … }` and of
  /// `launch(context) { … }`, through a label or an annotation of the lambda
  fn lambda_callee(&self, lambda: NodeId) -> Option<&'t str> {
    let mut node = self.tree.parent(lambda)?;
    while self.is(node, N::LABELED_EXPRESSION) || self.is(node, N::ANNOTATED_EXPRESSION) {
      node = self.tree.parent(node)?;
    }
    let call = if self.is(node, N::LAMBDA_ARGUMENT) {
      self.tree.parent(node)?
    } else if self.is(node, N::VALUE_ARGUMENT) {
      self.tree.parent(self.tree.parent(node)?)?
    } else {
      return None;
    };
    if !self.is(call, N::CALL_EXPRESSION) {
      return None;
    }
    let callee = self.first_composite(call).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION))?;
    self.identifier_text(callee)
  }

  // --- names and references

  /// A `REFERENCE_EXPRESSION`: a reference unless claimed, with the role a parent gave it
  fn name(&mut self, id: NodeId, mark: Option<Mark>) {
    if matches!(mark, Some(Mark::Claimed)) {
      return;
    }
    let Some(text) = self.identifier_text(id) else { return };
    let (role, shape) = match mark {
      Some(Mark::Role(role, shape)) => (role, shape),
      _ => (Role::Value, None),
    };
    self.reference(text, self.line_of(id), role, None, shape);
  }

  /// `a.b`, `a?.b`, `a.b(c)`: `b` is a reference qualified by the path of `a`, a call with the shape of `(c)`
  fn qualified(&mut self, id: NodeId, mark: Option<Mark>) {
    let Some(receiver) = self.first_composite(id) else { return };
    let Some(selector) = self.selector(id) else { return };
    let (name, role, shape) = if self.is(selector, N::REFERENCE_EXPRESSION) {
      let (role, shape) = match mark {
        Some(Mark::Role(role, shape)) => (role, shape),
        _ => (Role::Value, None),
      };
      (selector, role, shape)
    } else if self.is(selector, N::CALL_EXPRESSION) {
      // `a.b()()` calls what `a.b()` returns: `b` takes the shape of its own call
      let mut call = selector;
      while let Some(inner) = self.first_composite(call).filter(|it| self.is(*it, N::CALL_EXPRESSION)) {
        call = inner;
      }
      let Some(callee) = self.first_composite(call).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION)) else {
        return;
      };
      (callee, Role::Call, Some(self.call_shape(call)))
    } else {
      return;
    };
    self.claim(name);
    if matches!(mark, Some(Mark::Claimed)) {
      return;
    }
    self.member_reference(receiver, name, role, shape);
  }

  /// The reference of the member `name` of `receiver`, with the type of its receiver if it may have one
  fn member_reference(&mut self, receiver: NodeId, name: NodeId, role: Role, shape: Option<u32>) {
    let Some(text) = self.identifier_text(name) else { return };
    let path = self.path_of(receiver);
    let qualifier = match path {
      Some(path) => self.qualifier_ids(&path),
      None => Vec::new(),
    };
    let typed = if matches!(role, Role::Call | Role::Value) {
      self.typed_receiver(receiver, &qualifier)
    } else {
      None
    };
    let before = self.out.references.len();
    self.reference(text, self.line_of(name), role, Some(qualifier), shape);
    if let Some(typed) = typed.filter(|_| self.out.references.len() > before) {
      match typed {
        TypedReceiver::Bound { serial, type_path } => self.out.bound_receiver(before, serial, type_path),
        TypedReceiver::Property => self.out.property_receiver(before),
      }
    }
  }

  /// The selector of a qualified expression: its expression after the `.` or `?.`
  fn selector(&self, id: NodeId) -> Option<NodeId> {
    let mut after = false;
    for child in self.tree.children(id) {
      match self.tree.token_kind(child) {
        Some(K::DOT | K::SAFE_ACCESS) => after = true,
        None if after && !self.is_trivia(child) => return Some(child),
        _ => {}
      }
    }
    None
  }

  /// `::f`, `a::f`, `a.B::f`: `f` qualified by the path of the receiver; a receiver of one name is a type
  /// (`String::length`), one with type arguments (`List<String>::size`) a name
  fn callable_reference(&mut self, id: NodeId, mark: Option<Mark>) {
    let mut receiver = None;
    let mut name = None;
    let mut after = false;
    for child in self.tree.children(id) {
      if self.tree.token_kind(child) == Some(K::COLONCOLON) {
        after = true;
      } else if !self.tree.is_leaf(child) && !self.is_trivia(child) {
        if after {
          name = Some(child);
        } else {
          receiver = Some(child);
        }
      }
    }
    let Some(name) = name.filter(|it| self.is(*it, N::REFERENCE_EXPRESSION)) else { return };
    self.claim(name);
    if matches!(mark, Some(Mark::Claimed)) {
      return;
    }
    let Some(receiver) = receiver else {
      if let Some(text) = self.identifier_text(name) {
        self.reference(text, self.line_of(name), Role::Value, None, None);
      }
      return;
    };
    let receiver = if let Some(callee) = self.type_arguments_callee(receiver) {
      self.mark(callee, Mark::Role(Role::Value, None));
      callee
    } else {
      if self.is(receiver, N::REFERENCE_EXPRESSION) {
        self.mark(receiver, Mark::Role(Role::Type, None));
      }
      receiver
    };
    self.member_reference(receiver, name, Role::Value, None);
  }

  /// The name of `List<String>` before a `::`, which PSI reads as a call with type arguments alone
  fn type_arguments_callee(&self, id: NodeId) -> Option<NodeId> {
    if !self.is(id, N::CALL_EXPRESSION) || self.child(id, N::VALUE_ARGUMENT_LIST).is_some() || self.child(id, N::LAMBDA_ARGUMENT).is_some() {
      return None;
    }
    self.first_composite(id).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION))
  }

  /// A type `a.b.C<T>`: each name a type qualified by the names before it, the role and shape of a mark on the last
  fn path(&mut self, id: NodeId, mark: Option<Mark>) {
    let (role, shape) = match mark {
      Some(Mark::Role(role, shape)) => (role, shape),
      _ => (Role::Type, None),
    };
    let names = self.type_names(id);
    let mut segments: Vec<&'t str> = Vec::new();
    for (index, name) in names.iter().copied().enumerate() {
      self.claim(name);
      let Some(text) = self.identifier_text(name) else { continue };
      let qualifier = if segments.is_empty() { None } else { Some(self.qualifier_ids(&segments)) };
      if !matches!(mark, Some(Mark::Claimed)) {
        let (role, shape) = if index + 1 == names.len() { (role, shape) } else { (Role::Type, None) };
        self.reference(text, self.line_of(name), role, qualifier, shape);
      }
      segments.push(text);
    }
  }

  /// The `REFERENCE_EXPRESSION`s of the segments of a `USER_TYPE`, outermost qualifier first
  fn type_names(&self, user_type: NodeId) -> Vec<NodeId> {
    let mut chain = vec![user_type];
    let mut node = user_type;
    while let Some(qualifier) = self.child(node, N::USER_TYPE) {
      chain.push(qualifier);
      node = qualifier;
    }
    chain.iter().rev().filter_map(|it| self.child(*it, N::REFERENCE_EXPRESSION)).collect()
  }

  /// `[a, b, C]` of a user type `a.b.C<T>`
  fn type_path(&self, user_type: NodeId) -> Option<Vec<&'t str>> {
    let path: Vec<&'t str> = self.type_names(user_type).into_iter().filter_map(|it| self.identifier_text(it)).collect();
    (!path.is_empty()).then_some(path)
  }

  /// `a to b` calls `to` on `a`
  fn infix(&mut self, id: NodeId) {
    let Some(operation) = self.child(id, N::OPERATION_REFERENCE) else { return };
    let Some(name) = self.child_token(operation, K::IDENTIFIER) else { return };
    let Some(left) = self.first_composite(id) else { return };
    let path = self.path_of(left);
    let qualifier = match path {
      Some(path) => self.qualifier_ids(&path),
      None => Vec::new(),
    };
    if let Some(text) = self.tree.utf8_text(name) {
      self.reference(text, self.line_of(name), Role::Call, Some(qualifier), None);
    }
  }

  /// `this@Outer`, `super@Outer`: the label names a type
  fn label_type(&mut self, id: NodeId) {
    let Some(label) = self.child(id, N::LABEL_QUALIFIER).and_then(|it| self.child(it, N::LABEL)) else {
      return;
    };
    if let Some(name) = self.child_token(label, K::IDENTIFIER)
      && let Some(text) = self.tree.utf8_text(name)
    {
      self.reference(text, self.line_of(name), Role::Type, None, None);
    }
  }

  fn reference(&mut self, name: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>, shape: Option<u32>) {
    if name.is_empty() {
      return;
    }
    let id = self.out.intern(name);
    if qualifier.is_none() && self.bindings.is_bound(id) {
      return;
    }
    let container = self.container();
    let coroutine = match self.coroutines.last() {
      Some(Context::Coroutine) => Some(true),
      Some(Context::Plain) | None => Some(false),
      Some(Context::Unknown) => None,
    };
    self.out.add_reference(id, line, role, qualifier, container, shape, coroutine);
  }

  fn qualifier_ids(&mut self, path: &[&'t str]) -> Vec<u32> {
    let ids = self.out.ids(path);
    if ids.first().is_some_and(|it| self.bindings.is_bound(*it)) { Vec::new() } else { ids }
  }

  /// `a.b.c` as `[a, b, c]`: names, `this`, `super` and qualified names of them, else `None`
  fn path_of(&self, node: NodeId) -> Option<Vec<&'t str>> {
    let mut reversed = Vec::new();
    let mut node = node;
    loop {
      match self.tree.node_kind(node)? {
        N::REFERENCE_EXPRESSION => {
          reversed.push(self.identifier_text(node)?);
          break;
        }
        N::THIS_EXPRESSION => {
          reversed.push("this");
          break;
        }
        N::SUPER_EXPRESSION => {
          reversed.push("super");
          break;
        }
        N::DOT_QUALIFIED_EXPRESSION | N::SAFE_ACCESS_EXPRESSION => {
          let selector = self.selector(node).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION))?;
          reversed.push(self.identifier_text(selector)?);
          node = self.first_composite(node)?;
        }
        // `m!!.foo` names `m` as `m?.foo` does
        N::POSTFIX_EXPRESSION if self.is_not_null_assertion(node) => node = self.first_composite(node)?,
        _ => return None,
      }
    }
    reversed.reverse();
    Some(reversed)
  }

  /// What the receiver of a member reference may have a type by: a bound name with a declared type, or, not enclosed,
  /// a property `[m]` or `[this, m]` of the qualifier
  fn typed_receiver(&self, receiver: NodeId, qualifier: &[u32]) -> Option<TypedReceiver> {
    // `m!!.foo()`
    let receiver = if self.is_not_null_assertion(receiver) {
      self.first_composite(receiver).unwrap_or(receiver)
    } else {
      receiver
    };
    if self.is(receiver, N::REFERENCE_EXPRESSION)
      && let Some(id) = self.identifier_text(receiver).and_then(|it| self.out.name_id(it))
      && self.bindings.is_bound(id)
    {
      let binding = self.bindings.binding(id)?;
      // a parameter of a primary constructor is no name in a function or an accessor: a property of its name is
      if binding.initializers.is_some_and(|depth| self.containers.iter().skip(depth).any(|it| it.code)) {
        return None;
      }
      return Some(TypedReceiver::Bound {
        serial: binding.serial,
        type_path: binding.type_path.clone()?,
      });
    }
    let this = self.out.name_id("this");
    let property = match qualifier {
      [_] => true,
      [first, _] => Some(*first) == this,
      _ => false,
    };
    (property && !self.enclosed()).then_some(TypedReceiver::Property)
  }

  /// A smart cast may give a name of `path` another type: a local or a property (`m`, `this.m`)
  fn smart_cast(&mut self, path: Option<Vec<&'t str>>) {
    let Some(path) = path else { return };
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

  /// `is Sub ->` of a `when` with a subject: a smart cast may give the subject another type
  fn type_test(&mut self, id: NodeId) {
    let Some(when) = self.tree.ancestor_or_self(id, |it| self.is(it, N::WHEN)) else {
      return;
    };
    let Some(subject) = self.when_subject(when) else { return };
    let path = if self.is(subject, N::PROPERTY) {
      self.name_token(subject).and_then(|it| self.tree.utf8_text(it)).map(|it| vec![it])
    } else {
      self.path_of(subject)
    };
    self.smart_cast(path);
  }

  /// The subject of a `when`: what its parentheses hold
  fn when_subject(&self, when: NodeId) -> Option<NodeId> {
    let mut inside = false;
    for child in self.tree.children(when) {
      match self.tree.token_kind(child) {
        Some(K::LPAR) => inside = true,
        Some(K::RPAR | K::LBRACE) => return None,
        None if inside && !self.is_trivia(child) => return Some(child),
        _ => {}
      }
    }
    None
  }

  // --- the file

  fn package(&mut self, id: NodeId) {
    if self.child_token(id, K::PACKAGE_KEYWORD).is_none() {
      return;
    }
    self.import_statement(id);
    if let Some(path) = self.first_composite(id).and_then(|it| self.path_of(it)) {
      self.out.package = Some(path.join("."));
    }
  }

  /// `@file:JvmName("Util")` names the JVM class of the top-level declarations; `@file:ApiStatus.Internal` marks them
  /// as no stable API. One annotation or a list of them (`@file:[A B]`)
  fn file_annotation_list(&mut self, id: NodeId) {
    let entries: Vec<NodeId> = self
      .tree
      .descendants(id)
      .filter(|it| self.is(*it, N::ANNOTATION_ENTRY) && self.tree.parent(*it).is_some_and(|parent| parent == id || self.is(parent, N::ANNOTATION)))
      .collect();
    for entry in entries {
      let Some(names) = self.annotation_path(entry) else { continue };
      let jvm_name = names == ["JvmName"] || names == ["kotlin", "jvm", "JvmName"];
      self.file_annotations.push(names);
      if jvm_name
        && let Some(name) = self
          .child(entry, N::VALUE_ARGUMENT_LIST)
          .and_then(|it| self.child(it, N::VALUE_ARGUMENT))
          .and_then(|it| self.child(it, N::STRING_TEMPLATE))
          .and_then(|it| self.child(it, N::LITERAL_STRING_TEMPLATE_ENTRY))
          .and_then(|it| self.tree.utf8_text(it))
      {
        self.out.jvm_name = Some(name.to_string());
      }
    }
  }

  fn import(&mut self, id: NodeId) {
    self.import_statement(id);
    let Some(expression) = self.first_composite(id).filter(|it| !self.is(*it, N::IMPORT_ALIAS)) else {
      return;
    };
    let Some(segments) = self.path_of(expression) else { return };
    let line = self.line_of(id);
    if self.child_token(id, K::MUL).is_some() {
      self.out.add_import_all(&segments, false, line);
      return;
    }
    let Some((name, path)) = segments.split_last() else { return };
    let alias = self.child(id, N::IMPORT_ALIAS).and_then(|it| self.name_token(it));
    let alias_text = alias.and_then(|it| self.tree.utf8_text(it));
    let last = if self.is(expression, N::REFERENCE_EXPRESSION) {
      Some(expression)
    } else {
      self.selector(expression)
    };
    let name_line = last.map_or(line, |it| self.line_of(it));
    self.out.add_import_name(path, name, alias_text, false, line);
    let path_ids = self.out.ids(path);
    self.reference(name, name_line, Role::Import, (!path.is_empty()).then_some(path_ids), None);
    if let (Some(alias), Some(alias_text)) = (alias, alias_text) {
      let traits = Traits {
        kind: DeclKind::Binding,
        reach: Reach::File,
        member: if self.container().is_some() { MemberKind::Static } else { MemberKind::TopLevel },
        receiver: None,
        holder: Holder::Leaf,
      };
      self.declare(id, alias, alias_text, &traits, None, JvmFields::default());
    }
  }

  /// The lines of an import or package statement
  fn import_statement(&mut self, id: NodeId) {
    let (start, end) = self.lines_of(id);
    self.out.import_lines.push(LineSpan { start, end });
  }

  // --- declarations

  fn declaration(&mut self, id: NodeId, rule: &DeclRule) {
    let index = self.frames.len() - 1;
    let leading = self.leading(id);
    let names = self.declared_names(id);
    let mut fields = self.jvm_fields(id, names.first().copied(), rule);
    if self.bindings.is_local() {
      // a local declaration binds its name in the scope around it, with its type
      let value_type = fields.value_type.take().filter(|_| names.len() == 1);
      for name in names {
        let Some(text) = self.tree.utf8_text(name) else { continue };
        let name_id = self.out.intern(text);
        if rule.late {
          self.pending.push((index, name_id, value_type.clone()));
        } else {
          self.bindings.bind(name_id, value_type.clone());
        }
      }
      if rule.opens != Opens::Nothing {
        self.push_scope(rule.opens == Opens::Body);
      }
      return;
    }
    let traits = if names.is_empty() && rule.default_name.is_none() {
      None
    } else {
      Some(self.traits(id, rule))
    };
    if let Some(traits) = traits {
      // a parse error in a class may swallow a constructor: its constructors are unknown then
      if traits.kind == DeclKind::Type && traits.holder == Holder::Class && !self.has_error(id) {
        fields.constructors = Some(self.constructors(id));
      }
      let mut first = rule
        .default_name
        .filter(|_| names.is_empty())
        .map(|name| self.declare(id, id, name, &traits, leading, std::mem::take(&mut fields)));
      for name in names {
        if let Some(text) = self.tree.utf8_text(name) {
          // a pattern of several names declares values: only the first name could take a signature or a supertype
          let decl = self.declare(id, name, text, &traits, leading, std::mem::take(&mut fields));
          first.get_or_insert(decl);
        }
      }
      if let Some(first) = first {
        self.enter_container(Some(first), traits.holder);
      }
    }
    if rule.opens != Opens::Nothing {
      self.push_scope(rule.opens == Opens::Body);
    }
  }

  /// The name tokens of a declaration: its `IDENTIFIER`, or those of the entries of a destructuring declaration
  fn declared_names(&self, id: NodeId) -> Vec<NodeId> {
    if self.is(id, N::DESTRUCTURING_DECLARATION) {
      return self
        .tree
        .children(id)
        .filter(|it| self.is(*it, N::DESTRUCTURING_DECLARATION_ENTRY))
        .filter_map(|it| self.name_token(it))
        .collect();
    }
    self.name_token(id).into_iter().collect()
  }

  /// The first line of the doc comments above a declaration: those PSI binds into it and those before it
  fn leading(&self, id: NodeId) -> Option<u32> {
    let mut leading = self.leading_now;
    for child in self.tree.children(id) {
      if !self.is_trivia(child) {
        break;
      }
      if self.tree.kind(child).token() == Some(K::DOC_COMMENT) || self.is_doc_comment(child) {
        let line = self.line_of(child);
        leading = Some(leading.map_or(line, |it| it.min(line)));
      }
    }
    leading
  }

  /// What a declaration says about overriding, before the walk enters it, local or not: an override is an `Override`
  /// reference per supertype of the class-like node around it. Then it pushes the supertypes of the declarations
  /// directly in it: a class-like node its own, a function an empty list for its body.
  fn jvm_fields(&mut self, id: NodeId, name: Option<NodeId>, rule: &DeclRule) -> JvmFields {
    let mut fields = self.overriding(id, name, rule.kind);
    fields.unstable = self.unstable(id);
    match self.supertypes_of(id) {
      Some(supertypes) => {
        if rule.kind == DeclKind::Type {
          fields.supertypes.clone_from(&supertypes);
        }
        self.push_supertypes(supertypes);
      }
      None if rule.opens == Opens::Body => self.push_supertypes(Vec::new()),
      None => {}
    }
    fields
  }

  /// The signature and the modifiers of a function or a property, and its `Override` references
  fn overriding(&mut self, id: NodeId, name: Option<NodeId>, kind: DeclKind) -> JvmFields {
    if !matches!(kind, DeclKind::Function | DeclKind::Value) {
      return JvmFields::default();
    }
    let modality = self.modality(id);
    // a local function is no declaration: only the shape of an override needs its parameters
    let signature = if kind == DeclKind::Function && (modality.overrides || !self.bindings.is_local()) {
      self.signature(id)
    } else {
      None
    };
    if let (true, Some(name)) = (modality.overrides, name) {
      self.override_references(name, kind == DeclKind::Function, modality.receiver, signature.as_ref());
    }
    let name_text = name.and_then(|it| self.tree.utf8_text(it));
    let test = kind == DeclKind::Function && (modality.test || self.junit3_test(name_text, signature.as_ref()));
    let entry_point = kind == DeclKind::Function && !self.bindings.is_local() && name_text == Some("main") && signature.as_ref().is_some_and(|it| self.entry_point(id, it));
    JvmFields {
      signature,
      overrides: modality.overrides,
      overridable: modality.overridable,
      jvm_static: modality.jvm_static,
      jvm_field: modality.jvm_field,
      jvm_name: modality.jvm_name,
      lateinit: modality.lateinit,
      test,
      entry_point,
      setter: modality.setter,
      value_type: if kind == DeclKind::Value { self.typed(id) } else { None },
      ..JvmFields::default()
    }
  }

  /// One `Override` reference per supertype of the class around the member, qualified by its path
  fn override_references(&mut self, name: NodeId, function: bool, receiver: Option<u32>, signature: Option<&Signature>) {
    let Some(supertypes) = self.supertypes.last().filter(|it| !it.is_empty()).cloned() else {
      return;
    };
    let Some(text) = self.tree.utf8_text(name) else { return };
    let parameters = signature.map_or_else(Vec::new, |it| it.parameters.iter().map(|parameter| parameter.type_ref.clone()).collect());
    let shape = self.out.intern_shape(Shape::Override { function, receiver, parameters });
    for supertype in supertypes {
      self.reference(text, self.line_of(name), Role::Override, Some(supertype), Some(shape));
    }
  }

  fn declare(&mut self, node: NodeId, name_node: NodeId, name: &'t str, traits: &Traits, leading: Option<u32>, fields: JvmFields) -> u32 {
    let name = self.out.intern(name);
    let (start, end) = self.lines_of(node);
    let has_signature = fields.signature.is_some();
    let container = self.container();
    let unstable = fields.unstable || (traits.kind != DeclKind::Binding && container.is_none() && self.file_annotations.iter().any(|it| self.out.unstable_annotation(it)));
    let header = matches!(traits.kind, DeclKind::Function | DeclKind::Value | DeclKind::Constant).then(|| self.header(node, has_signature));
    let name_line = if name_node == node { start } else { self.line_of(name_node) };
    let declaration = Declaration {
      receiver: traits.receiver,
      signature: fields.signature,
      overrides: fields.overrides,
      overridable: fields.overridable,
      jvm_static: fields.jvm_static,
      jvm_field: fields.jvm_field,
      jvm_name: fields.jvm_name,
      lateinit: fields.lateinit,
      test: fields.test,
      entry_point: fields.entry_point,
      unstable,
      setter: fields.setter,
      supertypes: fields.supertypes,
      value_constructed: fields.value_type.as_ref().is_some_and(|it| it.constructed),
      value_type: fields.value_type.map(|it| it.path),
      constructors: fields.constructors,
      header,
      start_line: leading.map_or(start, |it| it.min(start)),
      end_line: end,
      name_line,
      in_error: self.error_depth > 0,
      ..blank_declaration(name, traits.kind, traits.reach, traits.member, container)
    };
    self.out.add_declaration(declaration)
  }

  /// The kind, reach and member kind of a declaration, and what it is to its members
  fn traits(&mut self, id: NodeId, rule: &DeclRule) -> Traits {
    let modifiers = self.modifier_texts(id);
    let holder = self.holder();
    let member = holder.is_some();
    let mut kind = rule.kind;
    let mut own = Holder::Leaf;
    let mut member_kind = member_in(holder, false);
    match self.tree.node_kind(id) {
      Some(N::CLASS) => {
        own = if self.child_token(id, K::INTERFACE_KEYWORD).is_some() {
          Holder::Interface
        } else if modifiers.contains(&"enum") {
          Holder::Enum
        } else if modifiers.contains(&"annotation") {
          Holder::Annotation
        } else {
          Holder::Class
        };
        member_kind = if modifiers.contains(&"inner") { MemberKind::Instance } else { member_in(holder, true) };
      }
      Some(N::OBJECT_DECLARATION) => {
        own = Holder::Object;
        member_kind = member_in(holder, true);
      }
      Some(N::TYPEALIAS) => member_kind = member_in(holder, true),
      Some(N::ENUM_ENTRY) => member_kind = MemberKind::Static,
      Some(N::PROPERTY) if modifiers.contains(&"const") => kind = DeclKind::Constant,
      _ => {}
    }
    let receiver = self.receiver_name(id).map(|it| self.out.intern(it));
    let reach = if kind == DeclKind::EnumEntry { Reach::Public } else { reach(&modifiers, member) };
    Traits {
      kind,
      reach,
      member: member_kind,
      receiver,
      holder: own,
    }
  }

  /// `override`, `open`, `abstract`, `final` and the JVM annotations of a function or a property, and its extension
  /// receiver
  fn modality(&mut self, id: NodeId) -> Modality {
    let modifiers = self.modifier_texts(id);
    let has = |modifier: &str| modifiers.contains(&modifier);
    let overrides = has("override");
    let holder = self.holder();
    // a member of an object or a companion, or a local, can't be overridden
    let member = matches!(holder, Some(Holder::Class | Holder::Interface | Holder::Enum)) && !self.bindings.is_local();
    let open = has("open") || has("abstract") || (overrides && !has("final")) || holder == Some(Holder::Interface);
    let receiver = self.receiver_name(id).map(|it| self.out.intern(it));
    let annotations = self.annotation_names(id);
    let annotated = |path: &[&str]| {
      annotations
        .iter()
        .any(|names| names == path || names.strip_prefix(["kotlin", "jvm"].as_slice()) == Some(path))
    };
    Modality {
      overrides,
      overridable: member && open && !has("private"),
      jvm_static: annotated(&["JvmStatic"]),
      jvm_field: annotated(&["JvmField"]),
      jvm_name: annotated(&["JvmName"]) || annotated(&["JvmSynthetic"]),
      lateinit: has("lateinit"),
      test: annotations.iter().any(|path| path.last().is_some_and(|last| TEST_ANNOTATIONS.contains(last))),
      setter: self.child_token(id, K::VAR_KEYWORD).map(|_| reach(&modifiers, holder.is_some())),
      receiver,
    }
  }

  /// Whether an annotation of the declaration marks it as no stable API
  fn unstable(&self, id: NodeId) -> bool {
    self.annotation_names(id).iter().any(|path| self.out.unstable_annotation(path))
  }

  /// Whether a function of `name` and `signature` is a `JUnit` 3 test
  fn junit3_test(&self, name: Option<&str>, signature: Option<&Signature>) -> bool {
    let Some(name) = name else { return false };
    let class = self.container().and_then(|id| self.out.declarations.get(id as usize)).map(|it| self.out.name(it.name));
    let supertypes = self.supertypes.last().into_iter().flatten().filter_map(|path| path.last()).map(|&name| self.out.name(name));
    is_junit3_test(name, signature.is_some_and(|it| it.parameters.is_empty()), self.bindings.is_local(), class, supertypes)
  }

  /// A top-level `fun main` without a receiver or type parameters, returning `Unit` if it has a declared return type:
  /// without parameters, of one `Array<String>` or of a `vararg` of `String`
  fn entry_point(&self, id: NodeId, signature: &Signature) -> bool {
    if self.holder().is_some() || self.receiver_type(id).is_some() || self.child(id, N::TYPE_PARAMETER_LIST).is_some() {
      return false;
    }
    if let Some(returns) = self.declared_type(id) {
      let unit = self
        .type_element(returns)
        .filter(|it| self.is(*it, N::USER_TYPE))
        .and_then(|it| self.type_path(it))
        .is_some_and(|path| matches!(path.as_slice(), ["Unit"] | ["kotlin", "Unit"]));
      if !unit {
        return false;
      }
    }
    match signature.parameters.as_slice() {
      [] => true,
      [parameter] if parameter.vararg => self.out.type_is(parameter.type_ref.as_ref(), &["String"], 1),
      [parameter] => self.out.type_is(parameter.type_ref.as_ref(), &["Array", "String"], 0),
      _ => false,
    }
  }

  /// The parameters of the `VALUE_PARAMETER_LIST` of a function or a constructor
  fn signature(&mut self, id: NodeId) -> Option<Signature> {
    let list = self.child(id, N::VALUE_PARAMETER_LIST)?;
    let type_parameters = self.type_parameter_names(id);
    let tree = self.tree;
    let parameters = tree
      .children(list)
      .filter(|it| tree.node_kind(*it) == Some(N::VALUE_PARAMETER))
      .filter_map(|it| self.parameter_of(it, &type_parameters))
      .collect();
    Some(Signature { parameters })
  }

  /// A parameter: its name and its type, a vararg as an array, as on the JVM, optional with a default value
  fn parameter_of(&mut self, id: NodeId, type_parameters: &[&'t str]) -> Option<Parameter> {
    let name = self.name_token(id).and_then(|it| self.tree.utf8_text(it))?;
    let vararg = self.modifier_texts(id).contains(&"vararg");
    let type_ref = self.child(id, N::TYPE_REFERENCE).and_then(|it| {
      let nullable = self.type_element(it).is_some_and(|element| self.is(element, N::NULLABLE_TYPE));
      self.type_ref(it, nullable, u8::from(vararg), type_parameters)
    });
    Some(Parameter {
      name: self.out.intern(name),
      type_ref,
      optional: self.child_token(id, K::EQ).is_some(),
      vararg,
    })
  }

  /// The type of a parameter as the last name of each type in it, in source order; `None` when a type parameter is in
  /// it, one of the function's or one bound around it, or when it has no names. A named parameter of a function type
  /// is left out, as an annotation is.
  fn type_ref(&mut self, node: NodeId, nullable: bool, arrays: u8, type_parameters: &[&'t str]) -> Option<TypeRef> {
    let mut names = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
      match self.tree.node_kind(node) {
        Some(N::USER_TYPE) => {
          let path = self.type_path(node)?;
          let (first, last) = (path.first()?, path.last()?);
          if type_parameters.contains(first) || self.out.name_id(first).is_some_and(|it| self.bindings.is_bound(it)) {
            return None;
          }
          names.push(self.out.intern(last));
          // the type arguments of each segment, the outermost qualifier's first
          let mut segment = Some(node);
          while let Some(current) = segment {
            if let Some(arguments) = self.child(current, N::TYPE_ARGUMENT_LIST) {
              stack.push(arguments);
            }
            segment = self.child(current, N::USER_TYPE);
          }
          continue;
        }
        Some(N::MODIFIER_LIST | N::ANNOTATION_ENTRY | N::ANNOTATION) | None => continue,
        Some(N::VALUE_PARAMETER) if self.name_token(node).is_some() => continue,
        Some(_) => {}
      }
      let start = stack.len();
      stack.extend(self.tree.children(node).filter(|it| !self.tree.is_leaf(*it)));
      stack[start..].reverse();
    }
    (!names.is_empty()).then_some(TypeRef { names, nullable, arrays })
  }

  /// The names of the type parameters of a declaration (`<T, R>`)
  fn type_parameter_names(&self, id: NodeId) -> Vec<&'t str> {
    let Some(list) = self.child(id, N::TYPE_PARAMETER_LIST) else { return Vec::new() };
    self
      .tree
      .children(list)
      .filter(|it| self.is(*it, N::TYPE_PARAMETER))
      .filter_map(|it| self.name_token(it))
      .filter_map(|it| self.tree.utf8_text(it))
      .collect()
  }

  /// The primary constructor of a class: its parameters (a vararg by `vararg` among its modifiers, optional by a
  /// default value) and the reach of the modifiers before `constructor`; none without one
  fn constructors(&mut self, id: NodeId) -> Vec<Constructor> {
    let Some(primary) = self.child(id, N::PRIMARY_CONSTRUCTOR) else { return Vec::new() };
    let type_parameters = self.type_parameter_names(id);
    let declared: Vec<NodeId> = self
      .child(primary, N::VALUE_PARAMETER_LIST)
      .map_or_else(Vec::new, |list| self.tree.children(list).filter(|it| self.is(*it, N::VALUE_PARAMETER)).collect());
    let parameters = declared.into_iter().filter_map(|it| self.parameter_of(it, &type_parameters)).collect();
    vec![Constructor {
      signature: Signature { parameters },
      reach: reach(&self.modifier_texts(primary), true),
    }]
  }

  /// Adds a constructor that the body of the class around the current node declares; nothing in a local class
  fn add_constructor(&mut self, constructor: Constructor) {
    if self.bindings.is_local() {
      return;
    }
    if let Some(decl) = self.containers.last().and_then(|it| it.decl) {
      self.out.add_constructor(decl, constructor, false);
    }
  }

  /// The type a value declaration binds its name with, its names interned (`value_path`)
  fn typed(&mut self, id: NodeId) -> Option<Typed> {
    let (path, constructed) = self.value_path(id)?;
    Some(Typed {
      path: self.out.ids(&path),
      constructed,
    })
  }

  /// The type of a property, a parameter or a typed lambda parameter: its user type without type arguments
  /// (`a.Manager` of `a.Manager<T>?`), not a type parameter around it, a function type or another kind of type, nor of
  /// a vararg; else the type a property's initializer constructs, a call of a capitalized name (`Manager(…)`,
  /// `a.Manager(…)`), not a delegate or any other expression; with whether a call gave it
  fn value_path(&self, id: NodeId) -> Option<(Vec<&'t str>, bool)> {
    if self.modifier_texts(id).contains(&"vararg") || self.is(id, N::DESTRUCTURING_DECLARATION) || self.child(id, N::DESTRUCTURING_DECLARATION).is_some() {
      return None;
    }
    let declared = if self.is(id, N::VALUE_PARAMETER) {
      self.child(id, N::TYPE_REFERENCE)
    } else {
      self.declared_type(id)
    };
    if let Some(type_node) = declared {
      let path = self.type_path(self.simple_user_type(type_node)?)?;
      // a type parameter around it, or one of the property's own (`val <T> List<T>.head: T`), bound after this
      let type_parameter = path.len() == 1 && (self.is_bound_text(path[0]) || self.type_parameter_names(id).contains(&path[0]));
      return (!type_parameter).then_some((path, false));
    }
    if !self.is(id, N::PROPERTY) || self.child(id, N::PROPERTY_DELEGATE).is_some() {
      return None;
    }
    let initializer = self.initializer(id)?;
    let path = if self.is(initializer, N::CALL_EXPRESSION) {
      self.path_of(self.first_composite(initializer)?)?
    } else if self.is(initializer, N::DOT_QUALIFIED_EXPRESSION) || self.is(initializer, N::SAFE_ACCESS_EXPRESSION) {
      let call = self.selector(initializer).filter(|it| self.is(*it, N::CALL_EXPRESSION))?;
      let callee = self.first_composite(call).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION))?;
      let mut path = self.path_of(self.first_composite(initializer)?)?;
      path.push(self.identifier_text(callee)?);
      path
    } else {
      return None;
    };
    let capitalized = path.last().is_some_and(|last| last.starts_with(char::is_uppercase));
    (capitalized && !(path.len() == 1 && self.is_bound_text(path[0]))).then_some((path, true))
  }

  /// The expression after the `=` of a property
  fn initializer(&self, id: NodeId) -> Option<NodeId> {
    let mut after = false;
    for child in self.tree.children(id) {
      if self.tree.token_kind(child) == Some(K::EQ) {
        after = true;
      } else if after && !self.tree.is_leaf(child) && !self.is_trivia(child) {
        return Some(child);
      }
    }
    None
  }

  /// The supertypes of a class, an object or a companion, the enum of an enum entry
  fn supertypes_of(&mut self, id: NodeId) -> Option<Vec<Vec<u32>>> {
    match self.tree.node_kind(id) {
      Some(N::CLASS | N::OBJECT_DECLARATION) => Some(self.supertype_list(id)),
      Some(N::ENUM_ENTRY) => Some(self.enum_supertypes()),
      _ => None,
    }
  }

  /// The paths of the entries of the `SUPER_TYPE_LIST` of a class-like node, an empty one for a function type; a call
  /// entry `Base(…)` calls the constructor of `Base`, with the shape taken now: an object expression pushes its
  /// supertypes after this, and the arguments are of the code around it
  fn supertype_list(&mut self, id: NodeId) -> Vec<Vec<u32>> {
    let Some(list) = self.child(id, N::SUPER_TYPE_LIST) else { return Vec::new() };
    let entries: Vec<NodeId> = self
      .tree
      .children(list)
      .filter(|it| {
        matches!(
          self.tree.node_kind(*it),
          Some(N::SUPER_TYPE_ENTRY | N::SUPER_TYPE_CALL_ENTRY | N::DELEGATED_SUPER_TYPE_ENTRY)
        )
      })
      .collect();
    let mut paths = Vec::new();
    for entry in entries {
      let call = self.is(entry, N::SUPER_TYPE_CALL_ENTRY);
      let type_reference = if call {
        self.child(entry, N::CONSTRUCTOR_CALLEE).and_then(|it| self.child(it, N::TYPE_REFERENCE))
      } else {
        self.child(entry, N::TYPE_REFERENCE)
      };
      let user_type = type_reference.and_then(|it| self.type_element(it)).filter(|it| self.is(*it, N::USER_TYPE));
      if let Some(user_type) = user_type.filter(|_| call) {
        let shape = self.call_shape(entry);
        self.mark(user_type, Mark::Role(Role::Call, Some(shape)));
      }
      let path = user_type.and_then(|it| self.type_path(it)).unwrap_or_default();
      paths.push(self.out.ids(&path));
    }
    paths
  }

  /// The supertypes of the body of an enum entry: the path of the enum around it; none outside an enum
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

  /// An object expression: a scope whose members override those of its supertypes
  fn object_literal(&mut self, id: NodeId) {
    self.push_scope(true);
    let supertypes = self.supertype_list(id);
    self.push_supertypes(supertypes);
  }

  /// A parameter, by the list it is in: of a primary constructor (`class_parameter`), of a function, a constructor or
  /// a context (bound at once, with its type: in the function's scope, or the property's), of a setter or a `catch`
  /// (bound at once), of a lambda (bound after the list, with its type), of a function type (nothing); a `for`'s is
  /// bound by its body
  fn parameter(&mut self, id: NodeId) {
    let Some(list) = self.tree.parent(id) else { return };
    let owner = self.tree.parent(list).and_then(|it| self.tree.node_kind(it));
    match (self.tree.node_kind(list), owner) {
      (Some(N::VALUE_PARAMETER_LIST), Some(N::PRIMARY_CONSTRUCTOR)) => self.class_parameter(id),
      (Some(N::VALUE_PARAMETER_LIST), Some(N::FUN | N::SECONDARY_CONSTRUCTOR)) | (Some(N::CONTEXT_PARAMETER_LIST), _) => {
        if let Some(name) = self.name_token(id) {
          let type_path = self.typed(id);
          self.bind_token(name, type_path);
        }
      }
      (Some(N::VALUE_PARAMETER_LIST), Some(N::PROPERTY_ACCESSOR | N::CATCH)) => {
        if let Some(name) = self.name_token(id) {
          self.bind_token(name, None);
        }
      }
      (Some(N::VALUE_PARAMETER_LIST), Some(N::FUNCTION_LITERAL)) => {
        let type_path = if self.name_token(id).is_some() { self.typed(id) } else { None };
        self.pend_names(id, self.frames.len() - 2, type_path.as_ref());
      }
      _ => {}
    }
  }

  /// The name tokens of a parameter: its name, or those of its destructuring declaration
  fn parameter_names(&self, id: NodeId) -> Vec<NodeId> {
    match self.child(id, N::DESTRUCTURING_DECLARATION) {
      Some(pattern) => self.declared_names(pattern),
      None => self.name_token(id).into_iter().collect(),
    }
  }

  /// Binds the names of the parameter `id` when the frame at `at` is left
  fn pend_names(&mut self, id: NodeId, at: usize, type_path: Option<&Typed>) {
    for name in self.parameter_names(id) {
      if let Some(text) = self.tree.utf8_text(name) {
        let name_id = self.out.intern(text);
        self.pending.push((at, name_id, type_path.cloned()));
      }
    }
  }

  /// A `val`/`var` constructor parameter is a property; a plain one is visible in the class body, its initializers
  /// see its type
  fn class_parameter(&mut self, id: NodeId) {
    let Some(name) = self.name_token(id) else { return };
    let property = self.child_token(id, K::VAL_KEYWORD).is_some() || self.child_token(id, K::VAR_KEYWORD).is_some();
    // an `override val` of a local class overrides too
    let fields = if property {
      let mut fields = self.overriding(id, Some(name), DeclKind::Value);
      fields.unstable = self.unstable(id);
      fields
    } else {
      JvmFields::default()
    };
    if !property || self.bindings.is_local() {
      let type_path = self.typed(id);
      self.bind_token(name, type_path);
      if !property && let Some(binding) = self.bindings.last_mut() {
        binding.initializers = Some(self.containers.len());
      }
      return;
    }
    let Some(text) = self.tree.utf8_text(name) else { return };
    let traits = Traits {
      kind: DeclKind::Value,
      reach: reach(&self.modifier_texts(id), true),
      member: MemberKind::Instance,
      receiver: None,
      holder: Holder::Leaf,
    };
    self.declare(id, name, text, &traits, None, fields);
  }

  /// A getter or a setter: code of its property, where `field` is bound; `private set` narrows the setter of a `var`
  fn accessor(&mut self, id: NodeId) {
    self.push_scope(true);
    self.bind_text("field");
    self.enter_container(self.container(), Holder::Leaf);
    if let Some(container) = self.containers.last_mut() {
      container.code = true;
    }
    let modifiers = self.modifier_texts(id);
    if self.child_token(id, K::SET_KEYWORD).is_some()
      && (modifiers.contains(&"private") || modifiers.contains(&"internal") || modifiers.contains(&"protected"))
      && let Some(property) = self.container()
      && let Some(declaration) = self.out.declarations.get_mut(property as usize)
      && declaration.setter.is_some()
    {
      declaration.setter = Some(reach(&modifiers, true));
    }
  }

  /// `this(…)` and `super(…)` after a secondary constructor: a call of a constructor of the class around it, or of its
  /// supertype, the one a class without a primary constructor names without arguments (`: Base`); none beside another
  /// supertype, nor for a supertype with arguments or a delegate
  fn constructor_delegation(&mut self, id: NodeId) {
    let Some(reference) = self.child(id, N::CONSTRUCTOR_DELEGATION_REFERENCE) else { return };
    let this = self.child_token(reference, K::THIS_KEYWORD).is_some();
    if !this && self.child_token(reference, K::SUPER_KEYWORD).is_none() {
      return;
    }
    // the secondary constructor, the class body, the class
    let class = self
      .tree
      .parent(id)
      .and_then(|it| self.tree.parent(it))
      .and_then(|it| self.tree.parent(it))
      .filter(|it| self.is(*it, N::CLASS));
    let Some(class) = class else { return };
    let path = if this {
      self.name_token(class).and_then(|it| self.tree.utf8_text(it)).map(|it| vec![it])
    } else {
      let entries: Vec<NodeId> = self.child(class, N::SUPER_TYPE_LIST).map_or_else(Vec::new, |list| {
        self.tree.children(list).filter(|it| !self.tree.is_leaf(*it) && !self.is_trivia(*it)).collect()
      });
      match entries.as_slice() {
        [only] if self.is(*only, N::SUPER_TYPE_ENTRY) => self
          .child(*only, N::TYPE_REFERENCE)
          .and_then(|it| self.type_element(it))
          .filter(|it| self.is(*it, N::USER_TYPE))
          .and_then(|it| self.type_path(it)),
        _ => None,
      }
    };
    let Some(path) = path else { return };
    let Some((name, qualifier)) = path.split_last() else { return };
    let shape = self.call_shape(id);
    let qualifier = (!qualifier.is_empty()).then(|| self.out.ids(qualifier));
    self.reference(name, self.line_of(id), Role::Call, qualifier, Some(shape));
  }

  /// The arguments of a call, a supertype's constructor invocation or a constructor delegation call: those of its
  /// `VALUE_ARGUMENT_LIST` in order, a named one with its name, a spread (`*items`), and its lambda arguments last;
  /// interned, `enclosed` by the walk
  fn call_shape(&mut self, id: NodeId) -> u32 {
    let mut arguments = Vec::new();
    let mut spread = false;
    let values: Vec<NodeId> = self
      .child(id, N::VALUE_ARGUMENT_LIST)
      .map_or_else(Vec::new, |list| self.tree.children(list).filter(|it| self.is(*it, N::VALUE_ARGUMENT)).collect());
    for value_argument in values {
      let name = self
        .child(value_argument, N::VALUE_ARGUMENT_NAME)
        .and_then(|it| self.child(it, N::REFERENCE_EXPRESSION))
        .and_then(|it| self.identifier_text(it))
        .map(|it| self.out.intern(it));
      spread |= self.child_token(value_argument, K::MUL).is_some();
      let value = self
        .tree
        .children(value_argument)
        .filter(|it| !self.tree.is_leaf(*it) && !self.is_trivia(*it) && !self.is(*it, N::VALUE_ARGUMENT_NAME))
        .last();
      arguments.push((name, value.map_or(Argument::Expression, |it| self.argument(it))));
    }
    let lambdas = self.tree.children(id).filter(|it| self.is(*it, N::LAMBDA_ARGUMENT)).count();
    arguments.extend(std::iter::repeat_n((None, Argument::Lambda), lambdas));
    let enclosed = self.enclosed();
    self.out.intern_shape(Shape::Call { arguments, spread, enclosed })
  }

  /// What a value argument is: a string literal or template, an integer without a suffix (`-1` too), `true` or `false`,
  /// a lambda or an anonymous function; `null`, a real, a character, `1L` and `1u` are expressions
  fn argument(&self, value: NodeId) -> Argument {
    let integer = |it: NodeId| self.is(it, N::INTEGER_CONSTANT) && self.tree.utf8_text(it).is_some_and(|text| !text.ends_with(['l', 'L', 'u', 'U']));
    match self.tree.node_kind(value) {
      Some(N::STRING_TEMPLATE) => Argument::Text,
      Some(N::BOOLEAN_CONSTANT) => Argument::Boolean,
      Some(N::LAMBDA_EXPRESSION | N::FUN) => Argument::Lambda,
      _ if integer(value) => Argument::Number,
      Some(N::PREFIX_EXPRESSION) => {
        let sign = self
          .child(value, N::OPERATION_REFERENCE)
          .is_some_and(|it| self.child_token(it, K::MINUS).is_some() || self.child_token(it, K::PLUS).is_some());
        let operands: Vec<NodeId> = self
          .tree
          .children(value)
          .filter(|it| !self.tree.is_leaf(*it) && !self.is_trivia(*it) && !self.is(*it, N::OPERATION_REFERENCE))
          .collect();
        if sign && matches!(operands.as_slice(), [operand] if integer(*operand)) {
          Argument::Number
        } else {
          Argument::Expression
        }
      }
      _ => Argument::Expression,
    }
  }

  // --- the parts of a declaration

  /// The `IDENTIFIER` token of a declaration or a parameter
  fn name_token(&self, id: NodeId) -> Option<NodeId> {
    self.child_token(id, K::IDENTIFIER)
  }

  /// The name of a `REFERENCE_EXPRESSION`: its `IDENTIFIER`'s text; none for `this`, `super` and an empty one
  fn identifier_text(&self, id: NodeId) -> Option<&'t str> {
    let token = self.tree.first_child(id).filter(|it| self.tree.token_kind(*it) == Some(K::IDENTIFIER))?;
    self.tree.utf8_text(token)
  }

  /// The extension receiver of a function or a property: the `TYPE_REFERENCE` before its `.` and its name
  fn receiver_type(&self, id: NodeId) -> Option<NodeId> {
    let mut candidate = None;
    for child in self.tree.children(id) {
      match self.tree.token_kind(child) {
        Some(K::DOT) => return candidate,
        Some(K::IDENTIFIER | K::LPAR) => return None,
        _ if self.is(child, N::TYPE_REFERENCE) => candidate = Some(child),
        _ if self.is(child, N::VALUE_PARAMETER_LIST) => return None,
        _ => {}
      }
    }
    None
  }

  /// The type name of a simple receiver (`String`, `a.Foo<T>`, `Foo?`)
  fn receiver_name(&self, id: NodeId) -> Option<&'t str> {
    let user_type = self.simple_user_type(self.receiver_type(id)?)?;
    self.type_names(user_type).last().and_then(|it| self.identifier_text(*it))
  }

  /// The `USER_TYPE` a `TYPE_REFERENCE` holds, also inside a `NULLABLE_TYPE` (`a.Foo<T>?`, `Foo??`, `(Foo?)?`); none
  /// for a function type
  fn simple_user_type(&self, type_reference: NodeId) -> Option<NodeId> {
    let mut element = self.type_element(type_reference)?;
    while self.is(element, N::NULLABLE_TYPE) {
      element = self.first_composite(element)?;
    }
    self.is(element, N::USER_TYPE).then_some(element)
  }

  /// The declared type of a property (after its name) or the return type of a function (after its parameters): the
  /// `TYPE_REFERENCE` after a `:`
  fn declared_type(&self, id: NodeId) -> Option<NodeId> {
    let mut colon = false;
    for child in self.tree.children(id) {
      if self.is_trivia(child) {
        continue;
      }
      if self.tree.token_kind(child) == Some(K::COLON) {
        colon = true;
      } else if colon && self.is(child, N::TYPE_REFERENCE) {
        return Some(child);
      } else {
        colon = false;
      }
    }
    None
  }

  /// The type a `TYPE_REFERENCE` holds, past its modifiers: a `USER_TYPE`, a `NULLABLE_TYPE`, a `FUNCTION_TYPE`, …
  fn type_element(&self, type_reference: NodeId) -> Option<NodeId> {
    self
      .tree
      .children(type_reference)
      .find(|it| !self.tree.is_leaf(*it) && !self.is_trivia(*it) && !self.is(*it, N::MODIFIER_LIST))
  }

  /// The texts of the modifier keywords of a declaration, without its annotations
  fn modifier_texts(&self, id: NodeId) -> Vec<&'t str> {
    let Some(list) = self.child(id, N::MODIFIER_LIST) else { return Vec::new() };
    self
      .tree
      .children(list)
      .filter(|it| self.tree.is_leaf(*it) && !self.is_trivia(*it))
      .filter_map(|it| self.tree.utf8_text(it))
      .collect()
  }

  /// The names of the annotations among the modifiers of a declaration, each as its segments
  fn annotation_names(&self, id: NodeId) -> Vec<Vec<&'t str>> {
    let Some(list) = self.child(id, N::MODIFIER_LIST) else { return Vec::new() };
    let mut names = Vec::new();
    for child in self.tree.children(list) {
      match self.tree.node_kind(child) {
        Some(N::ANNOTATION_ENTRY) => names.extend(self.annotation_path(child)),
        Some(N::ANNOTATION) => names.extend(
          self
            .tree
            .children(child)
            .filter(|it| self.is(*it, N::ANNOTATION_ENTRY))
            .filter_map(|it| self.annotation_path(it)),
        ),
        _ => {}
      }
    }
    names
  }

  /// The segments of the type of an annotation entry
  fn annotation_path(&self, entry: NodeId) -> Option<Vec<&'t str>> {
    let user_type = self
      .child(entry, N::CONSTRUCTOR_CALLEE)
      .and_then(|it| self.child(it, N::TYPE_REFERENCE))
      .and_then(|it| self.type_element(it))?;
    self.is(user_type, N::USER_TYPE).then(|| self.type_path(user_type)).flatten()
  }

  /// The header of a function or a value (`Declaration::header`): the tokens of its node before its body, initializer,
  /// delegate or accessor, without comments, annotations, the modifiers of `HEADER_DROPS` and `;`; without the
  /// parameter list when `has_signature`
  fn header(&self, node: NodeId, has_signature: bool) -> String {
    let mut tokens: Vec<&'t str> = Vec::new();
    // (node, in the parameter list, in a modifier list)
    let mut stack = vec![(node, false, false)];
    while let Some((id, in_parameters, in_modifiers)) = stack.pop() {
      if self.tree.is_leaf(id) {
        let kind = self.tree.token_kind(id);
        if kind == Some(K::EQ) && !in_parameters {
          break;
        }
        if self.is_trivia(id) || kind == Some(K::SEMICOLON) {
          continue;
        }
        if let Some(text) = self.tree.utf8_text(id).filter(|it| !it.is_empty())
          && !(in_modifiers && HEADER_DROPS.contains(&text))
        {
          tokens.push(text);
        }
        continue;
      }
      let kind = self.tree.node_kind(id);
      let list = kind == Some(N::VALUE_PARAMETER_LIST) && self.tree.parent(id) == Some(node);
      match kind {
        Some(N::PROPERTY_DELEGATE | N::PROPERTY_ACCESSOR) => break,
        Some(N::BLOCK) if self.tree.parent(id) == Some(node) => break,
        Some(N::ANNOTATION_ENTRY | N::ANNOTATION) | None => continue,
        _ if list && has_signature => continue,
        _ => {}
      }
      let start = stack.len();
      stack.extend(self.tree.children(id).map(|it| (it, in_parameters || list, in_modifiers || kind == Some(N::MODIFIER_LIST))));
      stack[start..].reverse();
    }
    tokens.join(" ")
  }

  // --- scopes, containers, marks

  fn push_scope(&mut self, body: bool) {
    self.bindings.push(body);
    if let Some(frame) = self.frames.last_mut() {
      frame.scopes += 1;
    }
  }

  fn bind_text(&mut self, name: &'t str) {
    let id = self.out.intern(name);
    self.bindings.bind(id, None);
  }

  fn bind_token(&mut self, token: NodeId, type_path: Option<Typed>) {
    if let Some(text) = self.tree.utf8_text(token) {
      let id = self.out.intern(text);
      self.bindings.bind(id, type_path);
    }
  }

  fn is_bound_text(&self, text: &str) -> bool {
    self.out.name_id(text).is_some_and(|id| self.bindings.is_bound(id))
  }

  fn push_supertypes(&mut self, supertypes: Vec<Vec<u32>>) {
    self.supertypes.push(supertypes);
    if let Some(frame) = self.frames.last_mut() {
      frame.supertypes += 1;
    }
  }

  /// Makes the declaration the container of what is below the current node
  fn enter_container(&mut self, decl: Option<u32>, holder: Holder) {
    let supertypes = self.supertypes.len();
    let code = decl.and_then(|it| self.out.declarations.get(it as usize)).is_some_and(|it| it.kind == DeclKind::Function);
    self.containers.push(Container { decl, holder, code, supertypes });
    if let Some(frame) = self.frames.last_mut() {
      frame.container = true;
    }
  }

  fn holder(&self) -> Option<Holder> {
    self.containers.last().map(|it| it.holder)
  }

  fn container(&self) -> Option<u32> {
    self.containers.iter().rev().find_map(|it| it.decl)
  }

  /// Whether a lambda, an anonymous function, an object expression or a local declaration lies between the current
  /// node and the declaration around it
  fn enclosed(&self) -> bool {
    let level = self.containers.iter().rev().find(|it| it.decl.is_some()).map_or(0, |it| it.supertypes);
    self.supertypes.len() > level
  }

  fn take_mark(&mut self, id: NodeId) -> Option<Mark> {
    let index = self.marks.iter().rposition(|it| it.0 == id)?;
    Some(self.marks.remove(index).1)
  }

  fn is_marked(&self, id: NodeId) -> bool {
    self.marks.iter().any(|it| it.0 == id)
  }

  fn mark(&mut self, id: NodeId, mark: Mark) {
    let owner = self.frames.len() - 1;
    self.marks.push((id, mark, owner));
  }

  fn claim(&mut self, id: NodeId) {
    self.mark(id, Mark::Claimed);
  }

  // --- the tree

  fn is(&self, id: NodeId, kind: N) -> bool {
    self.tree.node_kind(id) == Some(kind)
  }

  fn parent_is(&self, id: NodeId, kind: N) -> bool {
    self.tree.parent(id).is_some_and(|it| self.is(it, kind))
  }

  /// `m!!`
  fn is_not_null_assertion(&self, id: NodeId) -> bool {
    self.is(id, N::POSTFIX_EXPRESSION) && self.child(id, N::OPERATION_REFERENCE).is_some_and(|it| self.child_token(it, K::EXCLEXCL).is_some())
  }

  fn child(&self, id: NodeId, kind: N) -> Option<NodeId> {
    self.tree.first_child_of_kind(id, kind)
  }

  fn child_token(&self, id: NodeId, kind: K) -> Option<NodeId> {
    self.tree.children(id).find(|it| self.tree.token_kind(*it) == Some(kind))
  }

  /// The first child that is a node (not a token, a comment or a doc comment)
  fn first_composite(&self, id: NodeId) -> Option<NodeId> {
    self.tree.children(id).find(|it| !self.tree.is_leaf(*it) && !self.is_trivia(*it))
  }

  /// Whitespace, a comment or a doc comment
  fn is_trivia(&self, id: NodeId) -> bool {
    matches!(
      self.tree.kind(id).token(),
      Some(K::WHITE_SPACE | K::EOL_COMMENT | K::BLOCK_COMMENT | K::SHEBANG_COMMENT | K::DOC_COMMENT)
    )
  }

  fn is_doc_comment(&self, id: NodeId) -> bool {
    let text = self.tree.text(id);
    (text.starts_with(b"/**") && !text.starts_with(b"/**/")) || (text.starts_with(b"///") && !text.starts_with(b"////"))
  }

  /// Whether an error element is in the subtree of `id`
  fn has_error(&self, id: NodeId) -> bool {
    let start = self.errors.partition_point(|it| *it < id.0);
    self.errors.get(start).is_some_and(|it| *it < self.tree.subtree_end(id).0)
  }

  fn line_of(&self, id: NodeId) -> u32 {
    self.lines.line(self.tree.range(id).start)
  }

  /// The first and the last line of a node, from its first token to its last, without the comments bound at its
  /// edges and the separator after an enum entry
  fn lines_of(&self, id: NodeId) -> (u32, u32) {
    let range = self.tree.own_range(id, |it| self.is_trivia(it), |it| matches!(it, K::COMMA | K::SEMICOLON));
    let start = self.lines.line(range.start);
    (start, end_line(|offset| self.lines.line(offset), self.source, range.start, range.end))
  }
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

// jscpd:ignore-end

#[cfg(test)]
#[path = "summary_test.rs"]
mod tests;
