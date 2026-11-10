//! The summary of a Kotlin file from its PSI (`FileSummary`): one preorder walk over the tree of `kotlin-syntax`, by
//! `NodeKind` and `TokenKind`, with the rules the head of `extract.rs` states for every language and those of the JVM
//! languages (`jvm`), on the parser-neutral builders of `summary_builder`.
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
//! - **Overrides**, **calls**, **constructors**, **`enclosed`**: as `jvm` says. A class-like node pushes its
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
//! - **Bounded**: the walk keeps its own stack of frames, the parse is bounded by the parser (`kotlin`), the size
//!   by the caller (`extract::summarize`).

use crate::jvm::{Classes, Holder, JvmFields, TypedReceiver, override_shape, smart_cast, type_name};
use crate::kotlin::parse;
use crate::psi_tree::{error_bytes, written_type};
use crate::summary::{Argument, Constructor, DeclKind, FileSummary, LineSpan, MemberKind, Parameter, Reach, Role, Shape, Signature, TypeRef};
use crate::summary_builder::{Bindings, Marks, SummaryBuilder, TEST_ANNOTATIONS, Typed, blank_declaration, end_line};
use kotlin_syntax::psi;
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
  /// The entries this node pushed onto the supertypes of `Walk::classes`
  supertypes: u8,
  /// It pushed its coroutine context onto `Walk::coroutines`
  coroutine: bool,
  error: bool,
  /// The first line of the doc comments above the next child
  leading: Option<u32>,
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
  marks: Marks<NodeId, Mark>,
  bindings: Bindings,
  /// The containers and the supertypes whose members the declarations at each level override
  classes: Classes,
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
      marks: Marks::default(),
      bindings: Bindings::default(),
      classes: Classes::default(),
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
    if matches!(self.tree.token_kind(id), Some(K::EOL_COMMENT | K::BLOCK_COMMENT)) && self.tree.is_doc_comment(id) {
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
        if let Some(callee) = self.callee(id).filter(|it| !self.is_marked(*it)) {
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
        if psi::kt_object_declaration_is_object_literal(self.tree, id) {
          self.object_literal(id);
        } else if psi::kt_object_declaration_is_companion(self.tree, id) {
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
        if psi::get_name_identifier(self.tree, id).is_some() {
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
          if psi::get_type_parameter_list(self.tree, id).is_some() || psi::get_modifier_list(self.tree, id).is_some_and(|it| self.child(it, N::CONTEXT_PARAMETER_LIST).is_some()) {
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
        if let Some(body) = psi::kt_class_or_object_get_body(self.tree, id) {
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
        if let Some(subject) = psi::kt_type_constraint_get_subject_type_parameter_name(self.tree, id) {
          self.mark(subject, Mark::Role(Role::Type, None));
        }
        true
      }
      N::TYPE_PARAMETER => {
        if let Some(name) = psi::get_name_identifier(self.tree, id) {
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
          let reach = reach(self.tree, id, true);
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
    // the index the frame had
    let left = self.frames.len();
    self.marks.leave_frame(left);
    self.bindings.leave_frame(left, frame.scopes);
    if frame.container {
      self.classes.leave();
    }
    for _ in 0..frame.supertypes {
      self.classes.pop_supertypes();
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
      N::FUN if psi::get_name_identifier(self.tree, id).is_none() => Some(Context::Unknown),
      N::FUN if psi::has_modifier(self.tree, id, K::SUSPEND_KEYWORD) => Some(Context::Coroutine),
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
    self.identifier_text(self.callee(call)?)
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
    let Some(receiver) = psi::kt_qualified_expression_get_receiver_expression(self.tree, id) else {
      return;
    };
    let Some(selector) = psi::kt_qualified_expression_get_selector_expression(self.tree, id) else {
      return;
    };
    let (name, role, shape) = if self.is(selector, N::REFERENCE_EXPRESSION) {
      let (role, shape) = match mark {
        Some(Mark::Role(role, shape)) => (role, shape),
        _ => (Role::Value, None),
      };
      (selector, role, shape)
    } else if self.is(selector, N::CALL_EXPRESSION) {
      // `a.b()()` calls what `a.b()` returns: `b` takes the shape of its own call
      let mut call = selector;
      while let Some(inner) = psi::get_callee_expression(self.tree, call).filter(|it| self.is(*it, N::CALL_EXPRESSION)) {
        call = inner;
      }
      let Some(callee) = self.callee(call) else {
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
      typed.give(&mut self.out, before);
    }
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
      } else if !self.tree.is_leaf(child) && !self.tree.is_trivia(child) {
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
    if !self.is(id, N::CALL_EXPRESSION) || psi::get_value_argument_list(self.tree, id).is_some() || psi::get_lambda_arguments(self.tree, id).next().is_some() {
      return None;
    }
    self.callee(id)
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

  /// The `REFERENCE_EXPRESSION`s of the segments of a `USER_TYPE`, outermost qualifier first; not the
  /// `ENUM_ENTRY_SUPERCLASS_REFERENCE_EXPRESSION` of an enum entry's `A(1)`, which names the enum, as its entry's
  /// supertypes do (`Classes::enum_supertypes`)
  fn type_names(&self, user_type: NodeId) -> Vec<NodeId> {
    let mut chain = vec![user_type];
    let mut node = user_type;
    while let Some(qualifier) = psi::kt_user_type_get_qualifier(self.tree, node) {
      chain.push(qualifier);
      node = qualifier;
    }
    chain
      .iter()
      .rev()
      .filter_map(|it| psi::kt_user_type_get_reference_expression(self.tree, *it))
      .filter(|it| self.is(*it, N::REFERENCE_EXPRESSION))
      .collect()
  }

  /// `[a, b, C]` of a user type `a.b.C<T>`
  fn type_path(&self, user_type: NodeId) -> Option<Vec<&'t str>> {
    let path: Vec<&'t str> = self.type_names(user_type).into_iter().filter_map(|it| self.identifier_text(it)).collect();
    (!path.is_empty()).then_some(path)
  }

  /// `a to b` calls `to` on `a`
  fn infix(&mut self, id: NodeId) {
    let Some(operation) = self.child(id, N::OPERATION_REFERENCE) else { return };
    let Some(name) = self.name_element(operation) else { return };
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
    if let Some(name) = self.name_element(label)
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
    self.bindings.qualifier(ids)
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
          let selector = psi::kt_qualified_expression_get_selector_expression(self.tree, node).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION))?;
          reversed.push(self.identifier_text(selector)?);
          node = psi::kt_qualified_expression_get_receiver_expression(self.tree, node)?;
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
      if binding.initializers.is_some_and(|depth| self.classes.in_code_below(depth)) {
        return None;
      }
      return TypedReceiver::bound(binding);
    }
    TypedReceiver::property(&self.out, qualifier, self.enclosed())
  }

  /// A smart cast may give a name of `path` another type: a local or a property (`m`, `this.m`)
  fn smart_cast(&mut self, path: Option<Vec<&'t str>>) {
    if let Some(path) = path {
      smart_cast(&mut self.out, &self.bindings, self.classes.container(), &path);
    }
  }

  /// `is Sub ->` of a `when` with a subject: a smart cast may give the subject another type
  fn type_test(&mut self, id: NodeId) {
    let Some(when) = self.tree.ancestor_or_self(id, |it| self.is(it, N::WHEN)) else {
      return;
    };
    let Some(subject) = self.when_subject(when) else { return };
    let path = if self.is(subject, N::PROPERTY) {
      self.name_text(subject).map(|it| vec![it])
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
        None if inside && !self.tree.is_trivia(child) => return Some(child),
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
    let entries: Vec<NodeId> = psi::get_annotation_entries(self.tree, id).collect();
    for entry in entries {
      let Some(names) = self.annotation_path(entry) else { continue };
      let jvm_name = names == ["JvmName"] || names == ["kotlin", "jvm", "JvmName"];
      self.file_annotations.push(names);
      if jvm_name
        && let Some(name) = psi::get_value_arguments(self.tree, entry)
          .next()
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
    let Some(expression) = psi::kt_import_directive_get_imported_reference(self.tree, id) else {
      return;
    };
    let Some(segments) = self.path_of(expression) else { return };
    let line = self.line_of(id);
    if psi::kt_import_directive_is_all_under(self.tree, id) {
      self.out.add_import_all(&segments, false, line);
      return;
    }
    let Some((name, path)) = segments.split_last() else { return };
    let alias = psi::kt_import_directive_get_alias(self.tree, id).and_then(|it| psi::get_name_identifier(self.tree, it));
    let alias_text = alias.and_then(|it| self.tree.utf8_text(it));
    let last = if self.is(expression, N::REFERENCE_EXPRESSION) {
      Some(expression)
    } else {
      psi::kt_qualified_expression_get_selector_expression(self.tree, expression)
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
          self.bindings.bind_later(index, name_id, value_type.clone());
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

  /// The name tokens of a declaration: its name identifier, or those of the entries of a destructuring declaration
  /// directly in it (not those of a top-level `val (a, b) = p`, which a parse error wraps)
  fn declared_names(&self, id: NodeId) -> Vec<NodeId> {
    if self.is(id, N::DESTRUCTURING_DECLARATION) {
      return psi::kt_destructuring_declaration_get_entries(self.tree, id)
        .filter(|it| self.tree.parent(*it) == Some(id))
        .filter_map(|it| psi::get_name_identifier(self.tree, it))
        .collect();
    }
    psi::get_name_identifier(self.tree, id).into_iter().collect()
  }

  /// The first line of the doc comments above a declaration: those PSI binds into it and those before it
  fn leading(&self, id: NodeId) -> Option<u32> {
    let mut leading = self.leading_now;
    for child in self.tree.children(id) {
      if !self.tree.is_trivia(child) {
        break;
      }
      if self.tree.kind(child).token() == Some(K::DOC_COMMENT) || self.tree.is_doc_comment(child) {
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
    let test = kind == DeclKind::Function && (modality.test || self.classes.junit3_test(&self.out, self.bindings.is_local(), name_text, signature.as_ref()));
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
    let Some(supertypes) = self.classes.overridden() else { return };
    let Some(text) = self.tree.utf8_text(name) else { return };
    let shape = override_shape(&mut self.out, function, receiver, signature);
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
    // a declaration that writes no name (an anonymous object) is its own name node: the jump lands where it starts
    let name_offset = if name_node == node {
      self.tree.declaration_range(node).start
    } else {
      self.tree.range(name_node).start
    };
    let name_line = if name_node == node { start } else { self.lines.line(name_offset) };
    let mut declaration = JvmFields { unstable, ..fields }.declaration(blank_declaration(name, traits.kind, traits.reach, traits.member, container));
    declaration.receiver = traits.receiver;
    declaration.header = header;
    declaration.start_line = leading.map_or(start, |it| it.min(start));
    declaration.end_line = end;
    declaration.name_line = name_line;
    declaration.name_column = psi_builder::document::column(self.source, name_offset);
    declaration.in_error = self.error_depth > 0;
    self.out.add_declaration(declaration)
  }

  /// The kind, reach and member kind of a declaration, and what it is to its members
  fn traits(&mut self, id: NodeId, rule: &DeclRule) -> Traits {
    let tree = self.tree;
    let holder = self.holder();
    let member = holder.is_some();
    let mut kind = rule.kind;
    let mut own = Holder::Leaf;
    let mut member_kind = member_in(holder, false);
    match tree.node_kind(id) {
      Some(N::CLASS) => {
        own = if psi::kt_class_is_interface(tree, id) {
          Holder::Interface
        } else if psi::kt_class_is_enum(tree, id) {
          Holder::Enum
        } else if psi::kt_class_or_object_is_annotation(tree, id) {
          Holder::Annotation
        } else {
          Holder::Class
        };
        member_kind = if psi::kt_class_is_inner(tree, id) {
          MemberKind::Instance
        } else {
          member_in(holder, true)
        };
      }
      Some(N::OBJECT_DECLARATION) => {
        own = Holder::Object;
        member_kind = member_in(holder, true);
      }
      Some(N::TYPEALIAS) => member_kind = member_in(holder, true),
      Some(N::ENUM_ENTRY) => member_kind = MemberKind::Static,
      Some(N::PROPERTY) if psi::has_modifier(tree, id, K::CONST_KEYWORD) => kind = DeclKind::Constant,
      _ => {}
    }
    let receiver = self.receiver_name(id).map(|it| self.out.intern(it));
    let reach = if kind == DeclKind::EnumEntry { Reach::Public } else { reach(tree, id, member) };
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
    let tree = self.tree;
    let has = |modifier: K| psi::has_modifier(tree, id, modifier);
    let overrides = has(K::OVERRIDE_KEYWORD);
    let holder = self.holder();
    // a member of an object or a companion, or a local, can't be overridden
    let member = matches!(holder, Some(Holder::Class | Holder::Interface | Holder::Enum)) && !self.bindings.is_local();
    let open = has(K::OPEN_KEYWORD) || has(K::ABSTRACT_KEYWORD) || (overrides && !has(K::FINAL_KEYWORD)) || holder == Some(Holder::Interface);
    let receiver = self.receiver_name(id).map(|it| self.out.intern(it));
    let annotations = self.annotation_names(id);
    let annotated = |path: &[&str]| {
      annotations
        .iter()
        .any(|names| names == path || names.strip_prefix(["kotlin", "jvm"].as_slice()) == Some(path))
    };
    let mutable = psi::is_var(tree, id) || psi::kt_parameter_is_mutable(tree, id);
    Modality {
      overrides,
      overridable: member && open && !has(K::PRIVATE_KEYWORD),
      jvm_static: annotated(&["JvmStatic"]),
      jvm_field: annotated(&["JvmField"]),
      jvm_name: annotated(&["JvmName"]) || annotated(&["JvmSynthetic"]),
      lateinit: has(K::LATEINIT_KEYWORD),
      test: annotations.iter().any(|path| path.last().is_some_and(|last| TEST_ANNOTATIONS.contains(last))),
      setter: mutable.then(|| reach(tree, id, holder.is_some())),
      receiver,
    }
  }

  /// Whether an annotation of the declaration marks it as no stable API
  fn unstable(&self, id: NodeId) -> bool {
    self.annotation_names(id).iter().any(|path| self.out.unstable_annotation(path))
  }

  /// A top-level `fun main` without a receiver or type parameters, returning `Unit` if it has a declared return type:
  /// without parameters, of one `Array<String>` or of a `vararg` of `String`
  fn entry_point(&self, id: NodeId, signature: &Signature) -> bool {
    let tree = self.tree;
    if self.holder().is_some() || psi::get_receiver_type_reference(tree, id).is_some() || psi::get_type_parameter_list(tree, id).is_some() {
      return false;
    }
    if let Some(returns) = psi::get_type_reference(tree, id) {
      let unit = psi::kt_type_reference_get_type_element(tree, returns)
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
    let tree = self.tree;
    psi::get_value_parameter_list(tree, id)?;
    let type_parameters = self.type_parameter_names(id);
    let parameters = psi::get_value_parameters(tree, id).filter_map(|it| self.parameter_of(it, &type_parameters)).collect();
    Some(Signature { parameters })
  }

  /// A parameter: its name and its type, a vararg as an array, as on the JVM, optional with a default value (an `=`,
  /// also one without a value after it yet)
  fn parameter_of(&mut self, id: NodeId, type_parameters: &[&'t str]) -> Option<Parameter> {
    let tree = self.tree;
    let name = self.name_text(id)?;
    let vararg = psi::kt_parameter_is_var_arg(tree, id);
    let type_reference = psi::get_type_reference(tree, id);
    let type_ref = type_reference.and_then(|it| {
      let nullable = psi::kt_type_reference_get_type_element(tree, it).is_some_and(|element| self.is(element, N::NULLABLE_TYPE));
      self.type_ref(it, nullable, u8::from(vararg), type_parameters)
    });
    // the annotations of a type are none of it, its `suspend` is
    let shown_type = type_reference.and_then(|it| written_type(tree, it, |node| matches!(tree.node_kind(node), Some(N::ANNOTATION_ENTRY | N::ANNOTATION))));
    Some(Parameter {
      name: self.out.intern(name),
      type_ref,
      shown_type,
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
          names.push(type_name(&mut self.out, &self.bindings, &path, type_parameters)?);
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
        Some(N::VALUE_PARAMETER) if psi::get_name_identifier(self.tree, node).is_some() => continue,
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
    psi::get_type_parameters(self.tree, id).filter_map(|it| self.name_text(it)).collect()
  }

  /// The primary constructor of a class: its parameters (a vararg by `vararg` among its modifiers, optional by a
  /// default value) and the reach of the modifiers before `constructor`; none without one
  fn constructors(&mut self, id: NodeId) -> Vec<Constructor> {
    let tree = self.tree;
    let Some(primary) = psi::kt_class_or_object_get_primary_constructor(tree, id) else {
      return Vec::new();
    };
    let type_parameters = self.type_parameter_names(id);
    let parameters = psi::get_value_parameters(tree, primary).filter_map(|it| self.parameter_of(it, &type_parameters)).collect();
    vec![Constructor {
      signature: Signature { parameters },
      reach: reach(tree, primary, true),
    }]
  }

  /// Adds a constructor that the body of the class around the current node declares; nothing in a local class
  fn add_constructor(&mut self, constructor: Constructor) {
    if self.bindings.is_local() {
      return;
    }
    if let Some(decl) = self.classes.innermost() {
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
    let tree = self.tree;
    if psi::kt_parameter_is_var_arg(tree, id) || self.is(id, N::DESTRUCTURING_DECLARATION) || psi::kt_parameter_get_destructuring_declaration(tree, id).is_some() {
      return None;
    }
    if let Some(type_node) = psi::get_type_reference(tree, id) {
      let path = self.type_path(self.simple_user_type(type_node)?)?;
      // a type parameter around it, or one of the property's own (`val <T> List<T>.head: T`), bound after this
      let type_parameter = path.len() == 1 && (self.is_bound_text(path[0]) || self.type_parameter_names(id).contains(&path[0]));
      return (!type_parameter).then_some((path, false));
    }
    if !self.is(id, N::PROPERTY) || psi::kt_property_has_delegate(tree, id) {
      return None;
    }
    let initializer = psi::get_initializer(tree, id)?;
    let path = if self.is(initializer, N::CALL_EXPRESSION) {
      self.path_of(psi::get_callee_expression(tree, initializer)?)?
    } else if self.is(initializer, N::DOT_QUALIFIED_EXPRESSION) || self.is(initializer, N::SAFE_ACCESS_EXPRESSION) {
      let call = psi::kt_qualified_expression_get_selector_expression(tree, initializer).filter(|it| self.is(*it, N::CALL_EXPRESSION))?;
      let callee = self.callee(call)?;
      let mut path = self.path_of(psi::kt_qualified_expression_get_receiver_expression(tree, initializer)?)?;
      path.push(self.identifier_text(callee)?);
      path
    } else {
      return None;
    };
    let capitalized = path.last().is_some_and(|last| last.starts_with(char::is_uppercase));
    (capitalized && !(path.len() == 1 && self.is_bound_text(path[0]))).then_some((path, true))
  }

  /// The supertypes of a class, an object or a companion, the enum of an enum entry
  fn supertypes_of(&mut self, id: NodeId) -> Option<Vec<Vec<u32>>> {
    match self.tree.node_kind(id) {
      Some(N::CLASS | N::OBJECT_DECLARATION) => Some(self.supertype_list(id)),
      Some(N::ENUM_ENTRY) => Some(self.classes.enum_supertypes(&self.out)),
      _ => None,
    }
  }

  /// The paths of the entries of the `SUPER_TYPE_LIST` of a class-like node, an empty one for a function type; a call
  /// entry `Base(…)` calls the constructor of `Base`, with the shape taken now: an object expression pushes its
  /// supertypes after this, and the arguments are of the code around it
  fn supertype_list(&mut self, id: NodeId) -> Vec<Vec<u32>> {
    let entries: Vec<NodeId> = psi::kt_class_or_object_get_super_type_list_entries(self.tree, id).collect();
    let mut paths = Vec::new();
    for entry in entries {
      let call = self.is(entry, N::SUPER_TYPE_CALL_ENTRY);
      let user_type = psi::kt_super_type_list_entry_get_type_as_user_type(self.tree, entry);
      if let Some(user_type) = user_type.filter(|_| call) {
        let shape = self.call_shape(entry);
        self.mark(user_type, Mark::Role(Role::Call, Some(shape)));
      }
      let path = user_type.and_then(|it| self.type_path(it)).unwrap_or_default();
      paths.push(self.out.ids(&path));
    }
    paths
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
        if let Some(name) = psi::get_name_identifier(self.tree, id) {
          let type_path = self.typed(id);
          self.bind_token(name, type_path);
        }
      }
      (Some(N::VALUE_PARAMETER_LIST), Some(N::PROPERTY_ACCESSOR | N::CATCH)) => {
        if let Some(name) = psi::get_name_identifier(self.tree, id) {
          self.bind_token(name, None);
        }
      }
      (Some(N::VALUE_PARAMETER_LIST), Some(N::FUNCTION_LITERAL)) => {
        let type_path = if psi::get_name_identifier(self.tree, id).is_some() { self.typed(id) } else { None };
        self.pend_names(id, self.frames.len() - 2, type_path.as_ref());
      }
      _ => {}
    }
  }

  /// The name tokens of a parameter: its name, or those of its destructuring declaration
  fn parameter_names(&self, id: NodeId) -> Vec<NodeId> {
    match psi::kt_parameter_get_destructuring_declaration(self.tree, id) {
      Some(pattern) => self.declared_names(pattern),
      None => psi::get_name_identifier(self.tree, id).into_iter().collect(),
    }
  }

  /// Binds the names of the parameter `id` when the frame at `at` is left
  fn pend_names(&mut self, id: NodeId, at: usize, type_path: Option<&Typed>) {
    for name in self.parameter_names(id) {
      if let Some(text) = self.tree.utf8_text(name) {
        let name_id = self.out.intern(text);
        self.bindings.bind_later(at, name_id, type_path.cloned());
      }
    }
  }

  /// A `val`/`var` constructor parameter is a property; a plain one is visible in the class body, its initializers
  /// see its type
  fn class_parameter(&mut self, id: NodeId) {
    let Some(name) = psi::get_name_identifier(self.tree, id) else { return };
    let property = psi::kt_parameter_has_val_or_var(self.tree, id);
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
        binding.initializers = Some(self.classes.depth());
      }
      return;
    }
    let Some(text) = self.tree.utf8_text(name) else { return };
    let traits = Traits {
      kind: DeclKind::Value,
      reach: reach(self.tree, id, true),
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
    self.enter_container_as(self.container(), Holder::Leaf, true);
    let tree = self.tree;
    let has = |modifier: K| psi::has_modifier(tree, id, modifier);
    if psi::kt_property_accessor_is_setter(tree, id)
      && (has(K::PRIVATE_KEYWORD) || has(K::INTERNAL_KEYWORD) || has(K::PROTECTED_KEYWORD))
      && let Some(property) = self.container()
      && let Some(declaration) = self.out.declarations.get_mut(property as usize)
      && declaration.setter.is_some()
    {
      declaration.setter = Some(reach(tree, id, true));
    }
  }

  /// `this(…)` and `super(…)` after a secondary constructor: a call of a constructor of the class around it, or of its
  /// supertype, the one a class without a primary constructor names without arguments (`: Base`); none beside another
  /// supertype, nor for a supertype with arguments or a delegate
  fn constructor_delegation(&mut self, id: NodeId) {
    let tree = self.tree;
    let Some(reference) = psi::get_callee_expression(tree, id) else { return };
    let this = self.child_token(reference, K::THIS_KEYWORD).is_some();
    if !this && self.child_token(reference, K::SUPER_KEYWORD).is_none() {
      return;
    }
    // the secondary constructor, the class body, the class
    let class = tree
      .parent(id)
      .and_then(|it| tree.parent(it))
      .and_then(|it| tree.parent(it))
      .filter(|it| self.is(*it, N::CLASS));
    let Some(class) = class else { return };
    let path = if this {
      self.name_text(class).map(|it| vec![it])
    } else {
      // the composites of the list: an error element beside the entry counts as another
      let entries: Vec<NodeId> = psi::kt_class_or_object_get_super_type_list(tree, class)
        .map_or_else(Vec::new, |list| tree.children(list).filter(|it| !tree.is_leaf(*it) && !self.tree.is_trivia(*it)).collect());
      match entries.as_slice() {
        [only] if self.is(*only, N::SUPER_TYPE_ENTRY) => psi::kt_super_type_list_entry_get_type_as_user_type(tree, *only).and_then(|it| self.type_path(it)),
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
    let tree = self.tree;
    let mut arguments = Vec::new();
    let mut spread = false;
    let values: Vec<NodeId> = psi::get_value_arguments(tree, id).filter(|it| self.is(*it, N::VALUE_ARGUMENT)).collect();
    for value_argument in values {
      let name = psi::kt_value_argument_get_argument_name(tree, value_argument)
        .and_then(|it| self.child(it, N::REFERENCE_EXPRESSION))
        .and_then(|it| self.identifier_text(it))
        .map(|it| self.out.intern(it));
      spread |= psi::kt_value_argument_is_spread(tree, value_argument);
      // its last composite, an error element after the value too (`getArgumentExpression` is the first expression)
      let value = tree
        .children(value_argument)
        .filter(|it| !tree.is_leaf(*it) && !self.tree.is_trivia(*it) && !self.is(*it, N::VALUE_ARGUMENT_NAME))
        .last();
      arguments.push((name, value.map_or(Argument::Expression, |it| self.argument(it))));
    }
    let lambdas = psi::get_lambda_arguments(tree, id).count();
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
          .filter(|it| !self.tree.is_leaf(*it) && !self.tree.is_trivia(*it) && !self.is(*it, N::OPERATION_REFERENCE))
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

  /// The text of a declaration's name identifier as written, backticks kept, as a reference spells it
  /// (`identifier_text`)
  fn name_text(&self, id: NodeId) -> Option<&'t str> {
    psi::get_name_identifier(self.tree, id).and_then(|it| self.tree.utf8_text(it))
  }

  /// The `IDENTIFIER` a simple name expression (a reference, a label, an operation reference) names: none for `this`,
  /// `super`, an operator and an empty one
  fn name_element(&self, id: NodeId) -> Option<NodeId> {
    psi::get_referenced_name_element(self.tree, id).filter(|it| self.tree.token_kind(*it) == Some(K::IDENTIFIER))
  }

  /// The name of a `REFERENCE_EXPRESSION`: its `IDENTIFIER`'s text as written, backticks kept (not
  /// `get_referenced_name`, which removes them: a name is matched with the declarations as the source spells them)
  fn identifier_text(&self, id: NodeId) -> Option<&'t str> {
    self.tree.utf8_text(self.name_element(id)?)
  }

  /// The callee of a call when it is a name (`foo` of `foo()`, of `foo<T>()`)
  fn callee(&self, call: NodeId) -> Option<NodeId> {
    psi::get_callee_expression(self.tree, call).filter(|it| self.is(*it, N::REFERENCE_EXPRESSION))
  }

  /// The type name of a simple receiver (`String`, `a.Foo<T>`, `Foo?`)
  fn receiver_name(&self, id: NodeId) -> Option<&'t str> {
    let user_type = self.simple_user_type(psi::get_receiver_type_reference(self.tree, id)?)?;
    self.type_names(user_type).last().and_then(|it| self.identifier_text(*it))
  }

  /// The `USER_TYPE` a `TYPE_REFERENCE` holds, also inside a `NULLABLE_TYPE` (`a.Foo<T>?`, `Foo??`, `(Foo?)?`, past the
  /// annotations of `(@A Foo)?`: `KtTypeElement.unwrapNullability`); none for a function type
  fn simple_user_type(&self, type_reference: NodeId) -> Option<NodeId> {
    let element = psi::unwrap_nullability(self.tree, psi::kt_type_reference_get_type_element(self.tree, type_reference)?)?;
    self.is(element, N::USER_TYPE).then_some(element)
  }

  /// The names of the annotations among the modifiers of a declaration, each as its segments
  fn annotation_names(&self, id: NodeId) -> Vec<Vec<&'t str>> {
    psi::get_annotation_entries(self.tree, id).filter_map(|it| self.annotation_path(it)).collect()
  }

  /// The segments of the type of an annotation entry, each without its backticks (`KtPsiUtil.unquoteIdentifier`): the
  /// annotation `` @`JvmName` `` is `JvmName`, as the compiler reads it
  fn annotation_path(&self, entry: NodeId) -> Option<Vec<&'t str>> {
    let tree = self.tree;
    let user_type = psi::kt_type_reference_get_type_element(tree, psi::kt_annotation_entry_get_type_reference(tree, entry)?)?;
    if !self.is(user_type, N::USER_TYPE) {
      return None;
    }
    Some(self.type_path(user_type)?.into_iter().map(psi::unquote_identifier).collect())
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
        if self.tree.is_trivia(id) || kind == Some(K::SEMICOLON) {
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
    self.classes.push_supertypes(supertypes);
    if let Some(frame) = self.frames.last_mut() {
      frame.supertypes += 1;
    }
  }

  /// Makes the declaration the container of what is below the current node; a function is code
  fn enter_container(&mut self, decl: Option<u32>, holder: Holder) {
    let code = decl.and_then(|it| self.out.declarations.get(it as usize)).is_some_and(|it| it.kind == DeclKind::Function);
    self.enter_container_as(decl, holder, code);
  }

  /// Makes the declaration the container of what is below the current node, `code` for a function or an accessor
  fn enter_container_as(&mut self, decl: Option<u32>, holder: Holder, code: bool) {
    self.classes.enter(decl, holder, code);
    if let Some(frame) = self.frames.last_mut() {
      frame.container = true;
    }
  }

  fn holder(&self) -> Option<Holder> {
    self.classes.holder()
  }

  fn container(&self) -> Option<u32> {
    self.classes.container()
  }

  /// Whether a lambda, an anonymous function, an object expression or a local declaration lies between the current
  /// node and the declaration around it (`Classes::enclosed`)
  fn enclosed(&self) -> bool {
    self.classes.enclosed()
  }

  fn take_mark(&mut self, id: NodeId) -> Option<Mark> {
    self.marks.take(id)
  }

  fn is_marked(&self, id: NodeId) -> bool {
    self.marks.contains(id)
  }

  fn mark(&mut self, id: NodeId, mark: Mark) {
    self.marks.set(id, mark, self.frames.len() - 1);
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
    self.tree.composite_children(id).next()
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
    let range = self.tree.declaration_range(id);
    let start = self.lines.line(range.start);
    (start, end_line(|offset| self.lines.line(offset), self.source, range.start, range.end))
  }
}

/// The reach of a declaration by its modifiers: `private` first, then `internal`, whatever else the list holds
fn reach(tree: &Tree, owner: NodeId, member: bool) -> Reach {
  if psi::is_private(tree, owner) {
    if member { Reach::Container } else { Reach::File }
  } else if psi::has_modifier(tree, owner, K::INTERNAL_KEYWORD) {
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

#[cfg(test)]
#[path = "summary_test.rs"]
mod tests;
