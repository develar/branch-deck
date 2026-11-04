//! The summary of a C# file from Roslyn's syntax tree (`FileSummary`), by the rules that the head of `extract.rs`
//! states for every language (locals dropped) and the JVM model the summary is made of (`jvm`: the stack of supertypes
//! an `override` overrides, `Shape::Call` of a call, the constructors of a type). What a node answers of its slots comes
//! from `csharp_syntax::syntax_getters`; one scan over the preorder ids of the tree (`Walk::run`) with an explicit
//! stack of frames, so a file nested as deep as the parser allows takes no stack per level.
//!
//! - **The package** is the namespace of the file's first type (`namespace A.B;`, `namespace A { namespace B {`:
//!   `A.B`), else its first namespace; the declarations of another namespace of the file are read as of that one.
//! - **Imports**: `using N;` and `using static N.T;` import everything of their path (`ImportTarget::All`), an alias
//!   `using X = N.T;` the name `T` as `X` (and is an `Import` reference to it); `global using` as any. An alias of a
//!   type that is no name (`using P = (int, int);`) imports nothing.
//! - **Declarations**: a type (a class, a struct, an interface, a record, an enum, a delegate), a method, a property, an
//!   event, a field's or an event field's declarator (a `const` and a `static readonly` one a constant), an enum member,
//!   a record's positional parameter (a public property). A constructor is one of the `constructors` of its type, a
//!   record's or a class's primary constructor too; a class or a struct without one has the public `()` (`finish`); a
//!   static class has none. A nested type is static. An extension method (`this T x` of a static method) and a member
//!   of an extension block (`extension(T t)`) have the receiver `T`. Inside a body a declaration is a local: a local
//!   function, a variable, a parameter, a pattern's variable (`out var x`, `o is string s`), the range variables of a
//!   query are bound, each with its declared type, a variable declared `var` with the type its `new T(…)` constructs.
//! - **References**: a simple name (`IdentifierName`, `GenericName`), the right side of a qualified name, the name of a
//!   member access (`a.b`, `a?.b`, qualified by the path before it), each a value, a type where C# reads a type, the
//!   callee of an invocation or the type of a `new` a call with the shape of its arguments; an attribute's name is a
//!   type, by its name and by its class's (`[Obsolete]` is `ObsoleteAttribute`); `this(…)` and `base(…)` of a
//!   constructor call the type or its base class. Every segment of a path is a reference of its own.
//! - **Not read**: documentation comments (their `cref`s), directives and the branches of `#if` the file's configuration
//!   does not take, labels, the names of named arguments, of anonymous objects' members and of attribute properties,
//!   indexers, operators, conversions and destructors (no use names them), `var`, `nameof` and the discard `_`. A
//!   member assigned in an object initializer (`new T { Size = 1 }`) has no receiver type.
//! - **Errors**: Roslyn has no error nodes: a token the parser skipped counts its bytes, a missing one one byte
//!   (`error_bytes`); a declaration whose header holds one is `in_error`, and a type with one in it has unknown
//!   constructors.

use crate::jvm::{Holder, JvmFields, TypedReceiver, override_shape, type_name};
use crate::psi_walk::{Span, WalkCore, Walker, has_error_in};
use crate::summary::{Argument, Constructor, DeclKind, FileSummary, LineSpan, MemberKind, Parameter, Reach, Role, Shape, Signature, TypeRef};
use crate::summary_builder::Typed;
use csharp_syntax::read;
use csharp_syntax::syntax_getters as sg;
use csharp_syntax::{NodeId, NodeKind as N, TokenKind as T, Tree, facts};
use std::collections::HashMap;

/// The attributes of a method a test framework runs (`Declaration::test`), by the last name of the attribute without
/// its `Attribute`: xUnit's `Fact` and `Theory`, `NUnit`'s `Test`, `TestCase`, `TestCaseSource` and `Theory`, `MSTest`'s
/// `TestMethod` and `DataTestMethod`
const TEST_ATTRIBUTES: &[&str] = &["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestMethod", "DataTestMethod"];

/// The attribute that marks a declaration as no stable API (`Declaration::unstable`):
/// `System.Diagnostics.CodeAnalysis.ExperimentalAttribute`
const UNSTABLE_ATTRIBUTES: &[&str] = &["Experimental"];

/// The segments of the longest path (`Walk::path_of`); a longer one is of an expression, as in Java (`java::summary`)
const MAX_PATH: usize = 64;

/// Fills `summary` (`summary_builder::empty_summary` of the file, its size within the limits) with the facts of the
/// C# file `source`; whether it was read, else it stays `truncated`
pub(crate) fn summarize(source: &[u8], summary: &mut FileSummary) -> bool {
  let tree = super::parse(source);
  let attribute_classes = attribute_classes(&tree);
  let mut walk = Walk::new(&tree, &attribute_classes);
  // the locals of top-level statements: a scope of the file that makes no declaration a local
  walk.core.bindings.push(false);
  crate::psi_walk::run(&mut walk);
  if walk.core.out.package.is_none() {
    walk.core.out.package = walk.first_namespace.take();
  }
  // C# gives a class or a struct without a constructor a public one without parameters
  walk.core.out.finish(summary, Some(|_| Reach::Public));
  true
}

/// The class names of the attributes of the file (`Obsolete` names `ObsoleteAttribute`), sorted, so that the walk
/// interns them with the lifetime of the tree
fn attribute_classes(tree: &Tree) -> Vec<String> {
  let mut names: Vec<String> = tree
    .descendants(tree.root())
    .filter(|&id| tree.node_kind(id) == Some(N::Attribute))
    .filter_map(|id| sg::attribute::name(tree, id))
    .filter_map(|name| read::name_leaf(tree, name).and_then(|leaf| tree.value_text(leaf)))
    .filter(|name| !name.ends_with("Attribute"))
    .map(|name| format!("{name}Attribute"))
    .collect();
  names.sort_unstable();
  names.dedup();
  names
}

/// What a parent decided about a node below it before the walk reaches it
#[derive(Debug, Clone, Copy)]
enum Mark {
  /// The callee of an invocation or the type of a `new` (`Role::Call`, the shape of its arguments), an attribute's name
  /// (`Role::Type`, also by its class's name)
  Role { role: Role, shape: Option<u32>, attribute: bool },
  /// A name that is no reference, nor anything in it: a namespace's, a label's, a named argument's
  Skip,
}

/// What the walk keeps of a node (`psi_walk::Frame::extra`)
#[derive(Default)]
struct CSharpFrame {
  /// A namespace's frame: its names, popped from `Walk::namespaces` when left
  namespace: bool,
  /// An extension block's frame: its receiver, popped from `Walk::receivers` when left
  receiver: bool,
}

struct Walk<'t> {
  tree: &'t Tree,
  /// The text, the builder, the scopes, the frames, the containers and the supertypes
  core: WalkCore<'t, csharp_syntax::CSharp, CSharpFrame, Mark>,
  /// The names of the namespaces around the node, the outermost first
  namespaces: Vec<Vec<&'t str>>,
  /// The first namespace of the file, its package when it declares no type
  first_namespace: Option<String>,
  /// The receivers of the extension blocks around the node, the innermost last (`None` for one of no named type)
  receivers: Vec<Option<&'t str>>,
  /// The declaration of each declarator of a field, by the declarator's node: the container of its initializer
  declarators: HashMap<u32, u32>,
  /// The tokens the parser skipped or found missing, by their ids, in document order
  errors: Vec<u32>,
  attribute_classes: &'t [String],
}

impl<'t> Walker<'t> for Walk<'t> {
  type Lang = csharp_syntax::CSharp;
  type Extra = CSharpFrame;
  type Mark = Mark;

  fn core(&mut self) -> &mut WalkCore<'t, csharp_syntax::CSharp, CSharpFrame, Mark> {
    &mut self.core
  }

  fn enter(&mut self, id: NodeId, mark: Option<Mark>) -> bool {
    self.enter_node(id, mark)
  }

  fn left(&mut self, extra: CSharpFrame) {
    if extra.namespace {
      self.namespaces.pop();
    }
    if extra.receiver {
      self.receivers.pop();
    }
  }
}

impl<'t> Walk<'t> {
  fn new(tree: &'t Tree, attribute_classes: &'t [String]) -> Self {
    let mut walk = Self {
      tree,
      core: WalkCore::new(tree),
      namespaces: Vec::new(),
      first_namespace: None,
      receivers: Vec::new(),
      declarators: HashMap::new(),
      errors: Vec::new(),
      attribute_classes,
    };
    let in_code = |id: NodeId| !walk.in_structured_trivia(id);
    walk.errors = tree
      .descendants(tree.root())
      .filter(|&id| (tree.is_leaf(id) && tree.is_missing(id)) || tree.node_kind(id) == Some(N::SkippedTokensTrivia))
      .filter(|&id| in_code(id))
      .map(|id| id.0)
      .collect();
    let missing = walk.errors.iter().filter(|&&id| tree.is_leaf(NodeId(id))).count();
    walk.core.out.error_bytes = u32::try_from(missing).unwrap_or(u32::MAX);
    walk
  }

  /// The namespace around the current node is the file's package if it declares the file's first type
  fn package_of_first_type(&mut self) {
    if self.core.out.package.is_none()
      && let Some(namespace) = self.namespaces.last().filter(|it| !it.is_empty())
    {
      self.core.out.package = Some(namespace.join("."));
    }
  }

  fn enter_node(&mut self, id: NodeId, mark: Option<Mark>) -> bool {
    let Some(kind) = self.kind(id) else { return false };
    if matches!(mark, Some(Mark::Skip)) {
      return false;
    }
    match kind {
      N::SkippedTokensTrivia => {
        let bytes = u32::try_from(self.tree.range(id).len()).unwrap_or(u32::MAX);
        self.core.out.error_bytes = self.core.out.error_bytes.saturating_add(bytes);
        false
      }
      // a documentation comment, a directive: no code
      kind if facts::is_trivia(kind.syntax_kind()) => false,
      N::IncompleteMember => {
        let bytes = u32::try_from(self.own_range(id).len()).unwrap_or(u32::MAX).max(1);
        self.core.out.error_bytes = self.core.out.error_bytes.saturating_add(bytes);
        true
      }
      N::NamespaceDeclaration | N::FileScopedNamespaceDeclaration => {
        self.namespace(id);
        true
      }
      N::UsingDirective => {
        self.import(id);
        false
      }
      N::ExternAliasDirective => false,
      N::GotoStatement => {
        if let Some(label) = sg::goto_statement::expression(self.tree, id).filter(|_| sg::goto_statement::case_or_default_keyword(self.tree, id).is_none()) {
          self.core.marks.insert(label.0, Mark::Skip);
        }
        true
      }
      N::NameColon | N::NameEquals => {
        // a named argument, an attribute's property, a member of an anonymous object: no reference
        let name = if kind == N::NameColon {
          sg::name_colon::name(self.tree, id)
        } else {
          sg::name_equals::name(self.tree, id)
        };
        if let Some(name) = name {
          self.core.marks.insert(name.0, Mark::Skip);
        }
        true
      }
      N::ClassDeclaration | N::StructDeclaration | N::InterfaceDeclaration | N::RecordDeclaration | N::RecordStructDeclaration | N::EnumDeclaration | N::UnionDeclaration => {
        self.type_declaration(id, kind);
        true
      }
      N::DelegateDeclaration => {
        self.delegate(id);
        true
      }
      N::ExtensionBlockDeclaration => {
        self.extension_block(id);
        true
      }
      N::MethodDeclaration => {
        self.method(id);
        true
      }
      N::ConstructorDeclaration => {
        self.constructor(id);
        true
      }
      N::PropertyDeclaration | N::EventDeclaration => {
        self.property(id);
        true
      }
      N::FieldDeclaration | N::EventFieldDeclaration => {
        self.field(id);
        true
      }
      N::VariableDeclarator => {
        self.declarator(id);
        true
      }
      N::EnumMemberDeclaration => {
        self.enum_member(id);
        true
      }
      N::GetAccessorDeclaration
      | N::SetAccessorDeclaration
      | N::InitAccessorDeclaration
      | N::AddAccessorDeclaration
      | N::RemoveAccessorDeclaration
      | N::UnknownAccessorDeclaration => {
        self.core.push_scope(true);
        if kind != N::GetAccessorDeclaration && kind != N::UnknownAccessorDeclaration {
          let value = self.core.out.intern("value");
          self.core.bindings.bind(value, None);
        }
        true
      }
      N::LocalFunctionStatement => {
        let name = sg::local_function_statement::identifier(self.tree, id);
        self.bind_name(name, None);
        self.core.push_scope(true);
        self.bind_type_parameters(id);
        true
      }
      N::Parameter => {
        self.parameter(id);
        true
      }
      N::SingleVariableDesignation => {
        let type_path = self.designation_type(id);
        let name = sg::single_variable_designation::identifier(self.tree, id);
        self.bind_name(name, type_path);
        true
      }
      N::ForEachStatement => {
        self.core.push_scope(true);
        let ty = sg::for_each_statement::type_(self.tree, id);
        let type_path = self.value_type(ty, None);
        let name = sg::for_each_statement::identifier(self.tree, id);
        self.bind_name(name, type_path);
        true
      }
      N::CatchDeclaration => {
        let ty = sg::catch_declaration::type_(self.tree, id);
        let type_path = self.value_type(ty, None);
        let name = sg::catch_declaration::identifier(self.tree, id);
        self.bind_name(name, type_path);
        true
      }
      N::FromClause | N::LetClause | N::JoinClause | N::JoinIntoClause | N::QueryContinuation => {
        let name = sg::identifier_of(self.tree, id);
        self.bind_name(name, None);
        true
      }
      N::DestructorDeclaration
      | N::OperatorDeclaration
      | N::ConversionOperatorDeclaration
      | N::IndexerDeclaration
      | N::Block
      | N::ForStatement
      | N::UsingStatement
      | N::FixedStatement
      | N::CatchClause
      | N::SwitchSection
      | N::SwitchExpressionArm
      | N::QueryExpression
      | N::SimpleLambdaExpression
      | N::ParenthesizedLambdaExpression
      | N::AnonymousMethodExpression => {
        self.core.push_scope(true);
        true
      }
      N::BaseConstructorInitializer | N::ThisConstructorInitializer => {
        self.constructor_call(id, kind);
        true
      }
      N::InvocationExpression => {
        self.call(id);
        true
      }
      N::ObjectCreationExpression => {
        self.creation(id);
        true
      }
      N::Attribute => {
        if let Some(name) = sg::attribute::name(self.tree, id) {
          let mark = Mark::Role {
            role: Role::Type,
            shape: None,
            attribute: true,
          };
          self.core.marks.insert(name.0, mark);
        }
        true
      }
      N::IdentifierName | N::GenericName => {
        self.simple_name(id, mark);
        true
      }
      N::QualifiedName => {
        self.qualified_name(id, mark);
        true
      }
      N::AliasQualifiedName => {
        self.alias_qualified_name(id, mark);
        true
      }
      N::SimpleMemberAccessExpression | N::PointerMemberAccessExpression => {
        self.member_access(id, mark);
        true
      }
      N::MemberBindingExpression => {
        self.member_binding(id, mark);
        true
      }
      _ => true,
    }
  }

  // --- declarations ---

  /// A namespace: the file's package if it is its first, its name no reference
  fn namespace(&mut self, id: NodeId) {
    let tree = self.tree;
    let name = sg::base_namespace_declaration::name(tree, id);
    if let Some(name) = name {
      self.core.marks.insert(name.0, Mark::Skip);
    }
    let path = name.and_then(|it| self.path_of(it)).unwrap_or_default();
    let mut full: Vec<&'t str> = self.namespaces.last().cloned().unwrap_or_default();
    full.extend(path);
    if self.first_namespace.is_none() && !full.is_empty() {
      self.first_namespace = Some(full.join("."));
    }
    self.namespaces.push(full);
    self.core.frame().extra.namespace = true;
    // the lines of the statement: a file-scoped one whole, a block one up to its name
    let range = self.own_range(id);
    let end = match (self.kind(id), name) {
      (Some(N::NamespaceDeclaration), Some(name)) => self.own_range(name).end,
      _ => sg::file_scoped_namespace_declaration::semicolon_token(tree, id).map_or(range.end, |it| tree.range(it).end),
    };
    let span = LineSpan {
      start: self.core.lines.line(range.start),
      end: self.core.end_line(range.start, end),
    };
    self.core.out.import_lines.push(span);
  }

  /// `using N;`, `using static N.T;`, `using X = N.T;`, `global using …`: the import, and the `Import` reference of an
  /// alias's target
  fn import(&mut self, id: NodeId) {
    let tree = self.tree;
    let range = self.own_range(id);
    let line = self.core.lines.line(range.start);
    self.core.out.import_lines.push(LineSpan {
      start: line,
      end: self.core.end_line(range.start, range.end),
    });
    let Some(segments) = sg::using_directive::namespace_or_type(tree, id).and_then(|it| self.path_of(it)) else {
      return;
    };
    let alias = sg::using_directive::alias(tree, id)
      .and_then(|it| sg::name_equals::name(tree, it))
      .and_then(|it| self.simple_name_text(it));
    match alias {
      Some(alias) => {
        let Some((name, path)) = segments.split_last() else { return };
        self.core.out.add_import_name(path, name, Some(alias), false, line);
        let qualifier = (!path.is_empty()).then(|| self.core.out.ids(path));
        self.reference(name, line, Role::Import, qualifier, None);
      }
      None => self.core.out.add_import_all(&segments, false, line),
    }
  }

  /// A class, a struct, an interface, a record or an enum; its type parameters are visible in it, and a class's
  /// primary constructor's parameters
  fn type_declaration(&mut self, id: NodeId, kind: N) {
    let tree = self.tree;
    let own = match kind {
      N::InterfaceDeclaration => Holder::Interface,
      N::EnumDeclaration => Holder::Enum,
      _ => Holder::Class,
    };
    if self.core.holder().is_none() {
      self.package_of_first_type();
    }
    let name = sg::identifier_of(tree, id);
    let supertypes = self.supertypes_of(id);
    let fields = JvmFields {
      unstable: self.unstable(id),
      supertypes: supertypes.clone(),
      constructors: self.constructors_of(id, kind),
      ..JvmFields::default()
    };
    self.core.push_supertypes(supertypes);
    if let Some(name) = name {
      let traits = self.traits(id, DeclKind::Type, own);
      let span = self.member_span(id, name);
      let decl = self.declare(name, traits, &span, fields, None);
      self.core.enter_container(decl, own);
    }
    self.core.push_scope(false);
    self.bind_type_parameters(id);
  }

  /// The constructors a type starts with: a record's or a class's primary constructor, else none, to which its declared
  /// ones are added (`finish` gives one without any a public `()`); `None` for an interface, an enum, a static class and
  /// a type with a parse error in it
  fn constructors_of(&mut self, id: NodeId, kind: N) -> Option<Vec<Constructor>> {
    let tree = self.tree;
    let constructible = matches!(kind, N::ClassDeclaration | N::StructDeclaration | N::RecordDeclaration | N::RecordStructDeclaration);
    if !constructible || sg::has_modifier(tree, id, T::StaticKeyword) || self.has_error(id) {
      return None;
    }
    // a primary constructor is public, whatever the type's reach
    let primary = sg::type_declaration::parameter_list(tree, id).map(|list| Constructor {
      signature: self.signature(Some(list)),
      reach: Reach::Public,
    });
    Some(primary.into_iter().collect())
  }

  /// A delegate: a type
  fn delegate(&mut self, id: NodeId) {
    if self.core.holder().is_none() {
      self.package_of_first_type();
    }
    let name = sg::delegate_declaration::identifier(self.tree, id);
    if let Some(name) = name {
      let fields = JvmFields {
        unstable: self.unstable(id),
        ..JvmFields::default()
      };
      let traits = self.traits(id, DeclKind::Type, Holder::Leaf);
      let span = self.member_span(id, name);
      let decl = self.declare(name, traits, &span, fields, None);
      self.core.enter_container(decl, Holder::Leaf);
    }
    self.core.push_scope(false);
    self.bind_type_parameters(id);
  }

  /// An extension block (`extension<T>(Bar<T> b) { … }`): its members have the receiver `Bar`, its parameter is seen
  /// in them
  fn extension_block(&mut self, id: NodeId) {
    let tree = self.tree;
    let receiver = sg::extension_block_declaration::parameter_list(tree, id)
      .and_then(|list| sg::base_parameter_list::parameters(tree, list).next())
      .and_then(|parameter| sg::parameter::type_(tree, parameter))
      .and_then(|ty| self.type_path(ty))
      .and_then(|path| path.last().copied());
    self.receivers.push(receiver);
    self.core.frame().extra.receiver = true;
    self.core.push_scope(false);
    self.bind_type_parameters(id);
  }

  /// A method: a function; its type parameters, parameters and body are local
  fn method(&mut self, id: NodeId) {
    let tree = self.tree;
    let name = sg::method_declaration::identifier(tree, id);
    self.core.push_scope(true);
    self.bind_type_parameters(id);
    let list = sg::method_declaration::parameter_list(tree, id);
    let signature = self.signature(list);
    let receiver = self.extension_receiver(id, list);
    let overrides = self.overrides(id);
    if let (true, Some(name)) = (overrides, name) {
      let receiver_id = receiver.map(|it| self.core.out.intern(it));
      self.override_references(id, name, true, receiver_id, Some(&signature));
    }
    let text = name.and_then(|it| self.text(it));
    let attributes = self.attribute_paths(id);
    let test = attributes
      .iter()
      .any(|path| path.last().is_some_and(|last| TEST_ATTRIBUTES.contains(&strip_attribute(last))));
    let entry_point = text == Some("Main") && self.entry_point(id, &signature);
    let fields = JvmFields {
      signature: Some(signature),
      overrides,
      overridable: self.overridable(id),
      test,
      entry_point,
      unstable: self.unstable(id),
      ..JvmFields::default()
    };
    // the method's own entry of the supertypes, before it is entered: its code overrides nothing and is not enclosed
    self.core.push_supertypes(Vec::new());
    if let Some(name) = name {
      let traits = self.traits(id, DeclKind::Function, Holder::Leaf);
      let span = self.member_span(id, name);
      let header = self.header(id, true);
      let decl = self.declare_with_receiver(name, traits, &span, fields, Some(header), receiver);
      self.core.enter_container(decl, Holder::Leaf);
    }
  }

  /// A constructor of the type around it, its parameters and body local; a static one is no constructor
  fn constructor(&mut self, id: NodeId) {
    let tree = self.tree;
    self.core.push_scope(true);
    if sg::has_modifier(tree, id, T::StaticKeyword) {
      return;
    }
    let signature = self.signature(sg::constructor_declaration::parameter_list(tree, id));
    let reach = self.reach_of(id);
    if let Some(decl) = self.core.classes.innermost() {
      self.core.out.add_constructor(decl, Constructor { signature, reach }, false);
    }
  }

  /// `: this(…)` calls a constructor of the type around it, `: base(…)` one of its base class: its first base type,
  /// unless that is named as an interface (`IDisposable`)
  fn constructor_call(&mut self, id: NodeId, kind: N) {
    let tree = self.tree;
    let shape = self.call_shape(sg::constructor_initializer::argument_list(tree, id));
    let Some(keyword) = sg::constructor_initializer::this_or_base_keyword(tree, id) else {
      return;
    };
    let line = self.core.line_at(keyword);
    let Some(owner) = self.core.classes.innermost().and_then(|it| self.core.out.declarations.get(it as usize)) else {
      return;
    };
    let path: Vec<u32> = if kind == N::ThisConstructorInitializer {
      vec![owner.name]
    } else {
      match owner.supertypes.first() {
        Some(path) if path.last().is_some_and(|last| !is_interface_name(self.core.out.name(*last))) => path.clone(),
        _ => return,
      }
    };
    let Some((&name, qualifier)) = path.split_last() else { return };
    let name = self.core.out.name(name);
    let qualifier = (!qualifier.is_empty()).then(|| qualifier.to_vec());
    self.reference(name, line, Role::Call, qualifier, shape);
  }

  /// A property or an event with accessors: a value, its accessors' code local
  fn property(&mut self, id: NodeId) {
    let tree = self.tree;
    let name = sg::identifier_of(tree, id);
    let ty = sg::type_of(tree, id);
    let receiver = self.receivers.last().copied().flatten();
    let overrides = self.overrides(id);
    if let (true, Some(name)) = (overrides, name) {
      let receiver_id = receiver.map(|it| self.core.out.intern(it));
      self.override_references(id, name, false, receiver_id, None);
    }
    let fields = JvmFields {
      overrides,
      overridable: self.overridable(id),
      unstable: self.unstable(id),
      value_type: self.value_type(ty, sg::property_declaration::initializer(tree, id).and_then(|it| sg::equals_value_clause::value(tree, it))),
      ..JvmFields::default()
    };
    if let Some(name) = name {
      let traits = self.traits(id, DeclKind::Value, Holder::Leaf);
      let span = self.member_span(id, name);
      let header = self.header(id, false);
      let decl = self.declare_with_receiver(name, traits, &span, fields, Some(header), receiver);
      self.core.enter_container(decl, Holder::Leaf);
    }
  }

  /// A field or an event field: a declaration of each of its declarators, which is the container of its initializer
  fn field(&mut self, id: NodeId) {
    let tree = self.tree;
    let Some(declaration) = sg::base_field_declaration::declaration(tree, id) else { return };
    let ty = sg::variable_declaration::type_(tree, declaration);
    let constant = sg::has_modifier(tree, id, T::ConstKeyword) || (sg::has_modifier(tree, id, T::StaticKeyword) && sg::has_modifier(tree, id, T::ReadOnlyKeyword));
    let kind = if constant { DeclKind::Constant } else { DeclKind::Value };
    let traits = self.traits(id, kind, Holder::Leaf);
    let unstable = self.unstable(id);
    let head = self.field_head(id, declaration);
    for (index, declarator) in sg::variable_declaration::variables(tree, declaration).enumerate() {
      let Some(name) = sg::variable_declarator::identifier(tree, declarator) else { continue };
      let initializer = sg::variable_declarator::initializer(tree, declarator).and_then(|it| sg::equals_value_clause::value(tree, it));
      let fields = JvmFields {
        unstable,
        value_type: self.value_type(ty, initializer),
        ..JvmFields::default()
      };
      // the first declarator starts with its field, its attributes and doc comment; a later one at its name
      let declarator_range = self.own_range(declarator);
      let start = if index == 0 {
        self.member_span(id, name).start
      } else {
        self.core.lines.line(declarator_range.start)
      };
      let span = Span {
        start,
        end: self.core.end_line(declarator_range.start, declarator_range.end),
        name: self.core.line_at(name),
      };
      let mut header = head.clone();
      if let Some(text) = self.text(name) {
        header.push(text);
      }
      let decl = self.declare(name, traits, &span, fields, Some(header.join(" ")));
      self.declarators.insert(declarator.0, decl);
    }
  }

  /// A declarator: a field's is the container of its initializer, a local's binds its name with its type
  fn declarator(&mut self, id: NodeId) {
    if let Some(&decl) = self.declarators.get(&id.0) {
      self.core.enter_container(decl, Holder::Leaf);
      return;
    }
    let tree = self.tree;
    let ty = tree
      .parent(id)
      .filter(|it| self.kind(*it) == Some(N::VariableDeclaration))
      .and_then(|it| sg::variable_declaration::type_(tree, it));
    let initializer = sg::variable_declarator::initializer(tree, id).and_then(|it| sg::equals_value_clause::value(tree, it));
    let type_path = self.value_type(ty, initializer);
    self.bind_name(sg::variable_declarator::identifier(tree, id), type_path);
  }

  /// An enum member: a static constant of its enum
  fn enum_member(&mut self, id: NodeId) {
    let Some(name) = sg::enum_member_declaration::identifier(self.tree, id) else { return };
    let fields = JvmFields {
      unstable: self.unstable(id),
      ..JvmFields::default()
    };
    let span = self.member_span(id, name);
    let decl = self.declare(name, (DeclKind::EnumEntry, Reach::Public, MemberKind::Static), &span, fields, None);
    self.core.enter_container(decl, Holder::Leaf);
  }

  /// A parameter: a record's positional one is a public property of it, any other binds its name with its declared
  /// type (a `params` one none: an array)
  fn parameter(&mut self, id: NodeId) {
    let tree = self.tree;
    let name = sg::parameter::identifier(tree, id);
    let ty = sg::parameter::type_(tree, id);
    let owner = tree.parent(id).and_then(|list| tree.parent(list)).and_then(|it| self.kind(it));
    if matches!(owner, Some(N::RecordDeclaration | N::RecordStructDeclaration)) {
      let Some(name) = name else { return };
      let range = self.own_range(id);
      let span = Span {
        start: self.core.lines.line(range.start),
        end: self.core.end_line(range.start, range.end),
        name: self.core.line_at(name),
      };
      let fields = JvmFields {
        unstable: self.unstable(id),
        value_type: self.value_type(ty, None),
        ..JvmFields::default()
      };
      let header = self.header(id, false);
      self.declare(name, (DeclKind::Value, Reach::Public, MemberKind::Instance), &span, fields, Some(header));
      return;
    }
    let params = sg::has_modifier(tree, id, T::ParamsKeyword);
    let type_path = if params { None } else { self.value_type(ty, None) };
    self.bind_name(name, type_path);
  }

  /// Binds the type parameters of a declaration in the scope it opened, before its return type and parameters are read
  fn bind_type_parameters(&mut self, id: NodeId) {
    let tree = self.tree;
    let Some(list) = sg::type_parameter_list_of(tree, id) else { return };
    let names: Vec<Option<NodeId>> = sg::type_parameter_list::parameters(tree, list).map(|it| sg::type_parameter::identifier(tree, it)).collect();
    for name in names {
      self.bind_name(name, None);
    }
  }

  /// Binds `name` in the current scope with its type
  fn bind_name(&mut self, name: Option<NodeId>, type_path: Option<Typed>) {
    let Some(text) = name.and_then(|it| self.text(it)) else { return };
    let name = self.core.out.intern(text);
    self.core.bindings.bind(name, type_path);
  }

  /// The kind, reach and member kind of a declaration of `kind`, given what it is to its own members
  fn traits(&self, id: NodeId, kind: DeclKind, own: Holder) -> (DeclKind, Reach, MemberKind) {
    let tree = self.tree;
    let holder = self.core.holder();
    let shared = sg::has_modifier(tree, id, T::StaticKeyword) || sg::has_modifier(tree, id, T::ConstKeyword);
    let member = match holder {
      None => MemberKind::TopLevel,
      // a nested type, a constant, a static member, a member of an extension block
      Some(_) if own != Holder::Leaf || kind == DeclKind::Type || shared || !self.receivers.is_empty() => MemberKind::Static,
      Some(_) => MemberKind::Instance,
    };
    (kind, self.reach_of(id), member)
  }

  /// The reach of a declaration's modifiers: `public`, `protected` and `protected internal` are public, `internal` and
  /// `private protected` of the assembly, `private` of the container, `file` of the file; without any, a member of an
  /// interface or an enum is public, any other member private, a type of a namespace internal
  fn reach_of(&self, id: NodeId) -> Reach {
    let tree = self.tree;
    let has = |kind| sg::has_modifier(tree, id, kind);
    let member = self.core.holder().is_some() && self.kind(id) != Some(N::Parameter);
    if has(T::PublicKeyword) || (has(T::ProtectedKeyword) && !has(T::PrivateKeyword)) {
      Reach::Public
    } else if has(T::InternalKeyword) || (has(T::PrivateKeyword) && has(T::ProtectedKeyword)) {
      Reach::Module
    } else if has(T::PrivateKeyword) {
      Reach::Container
    } else if has(T::FileKeyword) {
      Reach::File
    } else if !member || matches!(self.core.holder(), Some(Holder::Interface | Holder::Enum)) {
      if member { Reach::Public } else { Reach::Module }
    } else {
      Reach::Container
    }
  }

  fn declare(&mut self, name: NodeId, traits: (DeclKind, Reach, MemberKind), span: &Span, fields: JvmFields, header: Option<String>) -> u32 {
    self.declare_with_receiver(name, traits, span, fields, header, None)
  }

  fn declare_with_receiver(
    &mut self,
    name: NodeId,
    traits: (DeclKind, Reach, MemberKind),
    span: &Span,
    fields: JvmFields,
    header: Option<String>,
    receiver: Option<&'t str>,
  ) -> u32 {
    let text = self.text(name).unwrap_or_default();
    let in_error = self.tree.is_missing(name) || self.in_error(name);
    let decl = self.core.declare(text, traits, span, fields, header, in_error);
    if let Some(receiver) = receiver {
      let receiver = self.core.out.intern(receiver);
      self.core.out.declarations[decl as usize].receiver = Some(receiver);
    }
    decl
  }

  /// The lines of a declaration: from its first token, or a doc comment above it, to its last
  fn member_span(&self, id: NodeId, name: NodeId) -> Span {
    let doc = read::doc_comment(self.tree, id).map(|it| self.tree.range(it).start);
    self.core.span(self.own_range(id), self.tree.range(name).start, doc)
  }

  // --- the facts of a declaration ---

  /// The types of a type's base list (`: Base<T>, IFoo`, a record's `Base(x)`), each as written without type arguments
  fn supertypes_of(&mut self, id: NodeId) -> Vec<Vec<u32>> {
    let tree = self.tree;
    let Some(list) = sg::base_list_of(tree, id) else { return Vec::new() };
    let paths: Vec<Vec<&'t str>> = sg::base_list::types(tree, list)
      .filter_map(|base| sg::base_type::type_(tree, base))
      .filter_map(|ty| self.type_path(ty))
      .collect();
    paths.iter().map(|it| self.core.out.ids(it)).collect()
  }

  /// Whether a member overrides: `override`, or an explicit implementation of an interface's member (`void IFoo.Bar()`)
  fn overrides(&self, id: NodeId) -> bool {
    sg::has_modifier(self.tree, id, T::OverrideKeyword) || sg::explicit_interface_specifier_of(self.tree, id).is_some()
  }

  /// Whether a subtype may override a method, a property or an event: one of an interface that is not `static`,
  /// `private` or `sealed`, or one of a class that is `virtual`, `abstract` or an `override` without `sealed`
  fn overridable(&self, id: NodeId) -> bool {
    let tree = self.tree;
    let has = |kind| sg::has_modifier(tree, id, kind);
    if has(T::StaticKeyword) || has(T::PrivateKeyword) || has(T::SealedKeyword) || sg::explicit_interface_specifier_of(tree, id).is_some() {
      return false;
    }
    match self.core.holder() {
      Some(Holder::Interface) => true,
      Some(Holder::Class) => has(T::VirtualKeyword) || has(T::AbstractKeyword) || has(T::OverrideKeyword),
      _ => false,
    }
  }

  /// The `Override` references of an overriding member: of each supertype of the type around it, or of the interface
  /// it explicitly implements
  fn override_references(&mut self, id: NodeId, name: NodeId, function: bool, receiver: Option<u32>, signature: Option<&Signature>) {
    let tree = self.tree;
    let explicit = sg::explicit_interface_specifier_of(tree, id)
      .and_then(|it| sg::explicit_interface_specifier::name(tree, it))
      .and_then(|it| self.type_path(it))
      .map(|path| vec![self.core.out.ids(&path)]);
    let Some(supertypes) = explicit.or_else(|| self.core.classes.overridden()) else {
      return;
    };
    let Some(text) = self.text(name) else { return };
    let shape = override_shape(&mut self.core.out, function, receiver, signature);
    let line = self.core.line_at(name);
    for supertype in supertypes {
      self.reference(text, line, Role::Override, Some(supertype), Some(shape));
    }
  }

  /// The receiver of a method: the type of the `this` parameter of an extension method, or of the extension block
  /// around it (by its last name)
  fn extension_receiver(&self, id: NodeId, list: Option<NodeId>) -> Option<&'t str> {
    let tree = self.tree;
    if let Some(receiver) = self.receivers.last() {
      return *receiver;
    }
    if !sg::has_modifier(tree, id, T::StaticKeyword) {
      return None;
    }
    let first = list.and_then(|it| sg::base_parameter_list::parameters(tree, it).next())?;
    if !sg::has_modifier(tree, first, T::ThisKeyword) {
      return None;
    }
    sg::parameter::type_(tree, first).and_then(|ty| self.type_path(ty)).and_then(|path| path.last().copied())
  }

  /// `static void Main()`, `static int Main(string[] args)`, `static async Task Main(…)` of a type: the entry point of a
  /// program
  fn entry_point(&self, id: NodeId, signature: &Signature) -> bool {
    let tree = self.tree;
    let returns = sg::method_declaration::return_type(tree, id)
      .and_then(|it| self.type_path(it))
      .and_then(|path| path.last().copied());
    self.core.holder().is_some()
      && sg::has_modifier(tree, id, T::StaticKeyword)
      && returns.is_some_and(|it| matches!(it, "void" | "int" | "Task" | "ValueTask"))
      && match signature.parameters.as_slice() {
        [] => true,
        [parameter] => self.core.out.type_is(parameter.type_ref.as_ref(), &["string"], 1),
        _ => false,
      }
  }

  /// Whether an attribute of the declaration marks it as no stable API
  fn unstable(&self, id: NodeId) -> bool {
    self
      .attribute_paths(id)
      .iter()
      .any(|path| path.last().is_some_and(|last| UNSTABLE_ATTRIBUTES.contains(&strip_attribute(last))))
  }

  /// The names of the attributes of a declaration, each as its path (`[System, Obsolete]`)
  fn attribute_paths(&self, id: NodeId) -> Vec<Vec<&'t str>> {
    let tree = self.tree;
    sg::attribute_lists_of(tree, id)
      .flat_map(|list| sg::attribute_list::attributes(tree, list))
      .filter_map(|attribute| sg::attribute::name(tree, attribute))
      .filter_map(|name| self.path_of(name))
      .collect()
  }

  /// The parameters of a parameter list
  fn signature(&mut self, list: Option<NodeId>) -> Signature {
    let tree = self.tree;
    let items: Vec<NodeId> = list.map_or_else(Vec::new, |it| sg::base_parameter_list::parameters(tree, it).collect());
    let mut parameters = Vec::new();
    for parameter in items {
      let Some(name) = sg::parameter::identifier(tree, parameter).and_then(|it| self.text(it)) else {
        continue;
      };
      let ty = sg::parameter::type_(tree, parameter);
      let type_ref = ty.and_then(|it| self.type_ref(it));
      parameters.push(Parameter {
        name: self.core.out.intern(name),
        type_ref,
        optional: sg::parameter::default(tree, parameter).is_some(),
        vararg: sg::has_modifier(tree, parameter, T::ParamsKeyword),
      });
    }
    Signature { parameters }
  }

  /// The type of a parameter as the last name of each type in it, in source order (`TypeRef`): `[Dictionary, string,
  /// Item]` of `Dictionary<string, Models.Item>`, `[int]` of `int[]` with one array; `None` when a type parameter (bound
  /// around it) is in it, or it has no names
  fn type_ref(&mut self, ty: NodeId) -> Option<TypeRef> {
    let tree = self.tree;
    let (mut top, mut nullable, mut arrays) = (ty, false, 0u8);
    loop {
      match self.kind(top) {
        Some(N::NullableType) if !nullable => {
          nullable = true;
          top = sg::nullable_type::element_type(tree, top)?;
        }
        Some(N::ArrayType) if arrays == 0 => {
          arrays = u8::try_from(sg::array_type::rank_specifiers(tree, top).count()).unwrap_or(u8::MAX);
          top = sg::array_type::element_type(tree, top)?;
        }
        Some(N::RefType | N::ScopedType) => top = sg::type_of(tree, top)?,
        _ => break,
      }
    }
    let mut names = Vec::new();
    let mut stack = vec![top];
    while let Some(node) = stack.pop() {
      let children: Vec<NodeId> = match self.kind(node) {
        Some(N::PredefinedType) => {
          if let Some(text) = sg::predefined_type::keyword(tree, node).and_then(|it| self.text(it)) {
            names.push(self.core.out.intern(text));
          }
          continue;
        }
        Some(N::IdentifierName | N::GenericName | N::QualifiedName | N::AliasQualifiedName) => {
          let path = self.type_path(node)?;
          names.push(type_name(&mut self.core.out, &self.core.bindings, &path, &[])?);
          // the type arguments of the last name; those of a qualifier are of no name here
          match self
            .last_simple_name(node)
            .filter(|it| self.kind(*it) == Some(N::GenericName))
            .and_then(|it| sg::generic_name::type_argument_list(tree, it))
          {
            Some(arguments) => sg::type_argument_list::arguments(tree, arguments).collect(),
            None => continue,
          }
        }
        Some(_) => tree.children(node).filter(|it| !tree.is_leaf(*it) && !read::is_trivia(tree, *it)).collect(),
        None => continue,
      };
      stack.extend(children.into_iter().rev());
    }
    (!names.is_empty()).then_some(TypeRef { names, nullable, arrays })
  }

  /// The type a value is declared with, as written without type arguments (`Models.Item` of `Models.Item<T>?`), not a
  /// type parameter, a predefined type or an array; of `var` and a target-typed `new()` the type its `new T(…)`
  /// constructs (of `new()` the declared one)
  fn value_type(&mut self, ty: Option<NodeId>, initializer: Option<NodeId>) -> Option<Typed> {
    let tree = self.tree;
    let ty = ty?;
    let ty = if self.kind(ty) == Some(N::NullableType) {
      sg::nullable_type::element_type(tree, ty)?
    } else {
      ty
    };
    let inferred = self.kind(ty) == Some(N::IdentifierName) && self.simple_name_text(ty) == Some("var");
    let path = if inferred {
      let created = initializer.filter(|it| self.kind(*it) == Some(N::ObjectCreationExpression))?;
      self.type_path(sg::object_creation_expression::type_(tree, created)?)?
    } else {
      self.type_path(ty)?
    };
    self.core.typed(&path, inferred)
  }

  /// The type of a designation's variable: of `out Foo x`, `o is Foo f`, `case Foo f`
  fn designation_type(&mut self, id: NodeId) -> Option<Typed> {
    let tree = self.tree;
    let parent = tree.parent(id)?;
    let ty = match self.kind(parent)? {
      N::DeclarationExpression => sg::declaration_expression::type_(tree, parent),
      N::DeclarationPattern => sg::declaration_pattern::type_(tree, parent),
      N::RecursivePattern => sg::recursive_pattern::type_(tree, parent),
      _ => None,
    };
    self.value_type(ty, None)
  }

  /// The header of a declaration (`Declaration::header`): its tokens before its body, its accessors or its initializer,
  /// without comments, attributes, the modifiers no use depends on and, when `drop_parameters`, the parameter list
  fn header(&self, id: NodeId, drop_parameters: bool) -> String {
    let mut tokens: Vec<&'t str> = Vec::new();
    self.header_tokens(id, drop_parameters, &mut tokens);
    tokens.join(" ")
  }

  /// The header of a field before its declarators: its modifiers and its type
  fn field_head(&self, id: NodeId, declaration: NodeId) -> Vec<&'t str> {
    let tree = self.tree;
    let mut tokens: Vec<&'t str> = sg::modifiers_of(tree, id)
      .filter(|it| tree.token_kind(*it).is_some_and(|kind| !is_dropped(kind)))
      .filter_map(|it| self.text(it))
      .collect();
    if let Some(ty) = sg::variable_declaration::type_(tree, declaration) {
      self.header_tokens(ty, false, &mut tokens);
    }
    tokens
  }

  /// The header tokens of `id` and below, until a body, an arrow, an initializer or accessors
  fn header_tokens(&self, id: NodeId, drop_parameters: bool, tokens: &mut Vec<&'t str>) {
    let tree = self.tree;
    let prune = |node: NodeId| {
      read::is_trivia(tree, node) || self.kind(node) == Some(N::AttributeList) || (drop_parameters && matches!(self.kind(node), Some(N::ParameterList | N::BracketedParameterList)))
    };
    for node in tree.descendants_pruned(id, prune) {
      if node != id && matches!(self.kind(node), Some(N::Block | N::ArrowExpressionClause | N::EqualsValueClause | N::AccessorList)) {
        break;
      }
      let Some(token) = tree.token_kind(node) else { continue };
      if is_dropped(token) {
        continue;
      }
      if let Some(text) = self.text(node).filter(|it| !it.is_empty()) {
        tokens.push(text);
      }
    }
  }

  // --- references ---

  /// An invocation: its callee's reference gets the shape of the arguments
  fn call(&mut self, id: NodeId) {
    let tree = self.tree;
    let Some(callee) = sg::invocation_expression::expression(tree, id) else { return };
    let shape = self.call_shape(sg::invocation_expression::argument_list(tree, id));
    if matches!(
      self.kind(callee),
      Some(
        N::IdentifierName
          | N::GenericName
          | N::SimpleMemberAccessExpression
          | N::PointerMemberAccessExpression
          | N::MemberBindingExpression
          | N::QualifiedName
          | N::AliasQualifiedName
      )
    ) {
      let mark = Mark::Role {
        role: Role::Call,
        shape,
        attribute: false,
      };
      self.core.marks.insert(callee.0, mark);
    }
  }

  /// `new T(…)` calls the type with the shape of its arguments (`new T { … }` with none)
  fn creation(&mut self, id: NodeId) {
    let tree = self.tree;
    let Some(ty) = sg::object_creation_expression::type_(tree, id) else { return };
    let list = sg::object_creation_expression::argument_list(tree, id);
    let shape = match list {
      Some(_) => self.call_shape(list),
      None => Some(self.core.out.intern_shape(Shape::Call {
        arguments: Vec::new(),
        spread: false,
        enclosed: self.core.classes.enclosed(),
      })),
    };
    let mark = Mark::Role {
      role: Role::Call,
      shape,
      attribute: false,
    };
    self.core.marks.insert(ty.0, mark);
  }

  /// The interned `Shape::Call` of the arguments `list`: a named one with its name
  fn call_shape(&mut self, list: Option<NodeId>) -> Option<u32> {
    let tree = self.tree;
    let list = list?;
    let items: Vec<NodeId> = sg::base_argument_list::arguments(tree, list).collect();
    let mut arguments = Vec::new();
    for item in items {
      let name = sg::argument::name_colon(tree, item)
        .and_then(|it| sg::name_colon::name(tree, it))
        .and_then(|it| self.simple_name_text(it))
        .map(|it| self.core.out.intern(it));
      let kind = match sg::argument::ref_kind_keyword(tree, item) {
        Some(_) => Argument::Expression,
        None => sg::argument::expression(tree, item).map_or(Argument::Expression, |it| self.argument(it)),
      };
      arguments.push((name, kind));
    }
    Some(self.core.out.intern_shape(Shape::Call {
      arguments,
      spread: false,
      enclosed: self.core.classes.enclosed(),
    }))
  }

  /// What an argument is: a string, an integer without a suffix (`-1` too), `true` or `false`, a lambda or an anonymous
  /// method; `null`, a real, a character, `1L` and any other are expressions
  fn argument(&self, value: NodeId) -> Argument {
    let tree = self.tree;
    match self.kind(value) {
      Some(N::StringLiteralExpression | N::InterpolatedStringExpression | N::Utf8StringLiteralExpression) => Argument::Text,
      Some(N::NumericLiteralExpression) if self.is_plain_integer(value) => Argument::Number,
      Some(N::TrueLiteralExpression | N::FalseLiteralExpression) => Argument::Boolean,
      Some(N::UnaryMinusExpression | N::UnaryPlusExpression) => {
        let operand = sg::prefix_unary_expression::operand(tree, value);
        if operand.is_some_and(|it| self.kind(it) == Some(N::NumericLiteralExpression) && self.is_plain_integer(it)) {
          Argument::Number
        } else {
          Argument::Expression
        }
      }
      Some(N::SimpleLambdaExpression | N::ParenthesizedLambdaExpression | N::AnonymousMethodExpression) => Argument::Lambda,
      _ => Argument::Expression,
    }
  }

  /// An integer literal without a suffix: decimal, `0x`, `0b`, with separators
  fn is_plain_integer(&self, literal: NodeId) -> bool {
    let Some(text) = sg::literal_expression::token(self.tree, literal).and_then(|it| self.text(it)) else {
      return false;
    };
    let digits = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"));
    if let Some(hex) = digits {
      return !hex.is_empty() && hex.bytes().all(|it| it.is_ascii_hexdigit() || it == b'_');
    }
    let digits = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")).unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|it| it.is_ascii_digit() || it == b'_')
  }

  /// A simple name (`IdentifierName`, `GenericName`) that no parent names as its member: a value, a type where C#
  /// reads one, a call or an attribute by its mark
  fn simple_name(&mut self, id: NodeId, mark: Option<Mark>) {
    let tree = self.tree;
    if let Some(parent) = tree.parent(id) {
      let named_by_parent = match self.kind(parent) {
        Some(N::QualifiedName) => sg::qualified_name::right(tree, parent) == Some(id),
        Some(N::SimpleMemberAccessExpression | N::PointerMemberAccessExpression) => sg::member_access_expression::name(tree, parent) == Some(id),
        Some(N::MemberBindingExpression) => sg::member_binding_expression::name(tree, parent) == Some(id),
        // `alias::` is no name of the code (`global`, an extern alias)
        Some(N::AliasQualifiedName) => true,
        _ => false,
      };
      if named_by_parent {
        return;
      }
    }
    let role = if self.is_type_name(id) { Role::Type } else { Role::Value };
    let Some((leaf, text, role, shape, attribute)) = self.named(Some(id), mark, role) else {
      return;
    };
    let line = self.core.line_at(leaf);
    if matches!(text, "var" | "dynamic") && role == Role::Type || (text == "nameof" && role == Role::Call) {
      return;
    }
    self.reference(text, line, role, None, shape);
    if attribute {
      self.attribute_class_reference(text, line, None);
    }
  }

  /// `a.B` in a type, a `using`'s or an attribute's name: `B`, qualified by the path of `a`
  fn qualified_name(&mut self, id: NodeId, mark: Option<Mark>) {
    let tree = self.tree;
    let Some((leaf, text, role, shape, attribute)) = self.named(sg::qualified_name::right(tree, id), mark, Role::Type) else {
      return;
    };
    let qualifier = sg::qualified_name::left(tree, id).and_then(|it| self.path_of(it)).map(|it| self.core.out.ids(&it));
    let line = self.core.line_at(leaf);
    self.reference(text, line, role, Some(qualifier.clone().unwrap_or_default()), shape);
    if attribute {
      self.attribute_class_reference(text, line, Some(qualifier.unwrap_or_default()));
    }
  }

  /// `alias::B`, `global::B`: `B`, qualified by the alias (none for `global`)
  fn alias_qualified_name(&mut self, id: NodeId, mark: Option<Mark>) {
    let tree = self.tree;
    let role = if self.is_type_name(id) { Role::Type } else { Role::Value };
    let Some((leaf, text, role, shape, _)) = self.named(sg::alias_qualified_name::name(tree, id), mark, role) else {
      return;
    };
    let alias = sg::alias_qualified_name::alias(tree, id).and_then(|it| self.simple_name_text(it));
    let qualifier = alias.filter(|it| *it != "global").map(|it| vec![self.core.out.intern(it)]);
    let line = self.core.line_at(leaf);
    self.reference(text, line, role, qualifier, shape);
  }

  /// `a.b`, `a->b`: `b` qualified by the path of `a` (empty for an expression), with the type of its receiver
  fn member_access(&mut self, id: NodeId, mark: Option<Mark>) {
    let tree = self.tree;
    let Some((leaf, text, role, shape, _)) = self.named(sg::member_access_expression::name(tree, id), mark, Role::Value) else {
      return;
    };
    let line = self.core.line_at(leaf);
    let object = sg::member_access_expression::expression(tree, id);
    let qualifier = object.map_or_else(Vec::new, |it| self.qualifier_of(it));
    let bound = object.filter(|it| self.kind(*it) == Some(N::IdentifierName)).and_then(|it| self.simple_name_text(it));
    let receiver = self.core.typed_receiver(bound, &qualifier);
    if is_reference_name(text) {
      self.core.member_reference(text, line, role, qualifier, shape, None, receiver);
    }
  }

  /// `a?.b`: `b` of the expression before `?`, an empty qualifier, with the type of a bound name before it
  fn member_binding(&mut self, id: NodeId, mark: Option<Mark>) {
    let tree = self.tree;
    let Some((leaf, text, role, shape, _)) = self.named(sg::member_binding_expression::name(tree, id), mark, Role::Value) else {
      return;
    };
    // the receiver is the expression of the innermost conditional access whose `?.` this binding follows
    let object = std::iter::successors(tree.parent(id), |it| tree.parent(*it))
      .find(|it| self.kind(*it) == Some(N::ConditionalAccessExpression))
      .and_then(|it| sg::conditional_access_expression::expression(tree, it));
    let receiver = object.and_then(|it| self.bound_receiver(it));
    let line = self.core.line_at(leaf);
    if is_reference_name(text) {
      self.core.member_reference(text, line, role, Vec::new(), shape, None, receiver);
    }
  }

  /// Whether a name is a type, or a segment before the name of a type (`Models` of `Models.Item item`), as the segments
  /// of a JVM type are: the qualifier of a type is a namespace or a type, which the summary reads as a type
  fn is_type_name(&self, name: NodeId) -> bool {
    let tree = self.tree;
    let mut node = name;
    loop {
      if sg::is_in_type_slot(tree, node) {
        return true;
      }
      match tree.parent(node) {
        Some(parent) if self.kind(parent) == Some(N::QualifiedName) && sg::qualified_name::left(tree, parent) == Some(node) => node = parent,
        _ => return false,
      }
    }
  }

  /// The identifier of the simple name `name`, its text, and the role, the shape and whether it is an attribute's name
  /// as the mark of a parent says, else of the role `role`
  fn named(&self, name: Option<NodeId>, mark: Option<Mark>, role: Role) -> Option<(NodeId, &'t str, Role, Option<u32>, bool)> {
    let leaf = read::name_leaf(self.tree, name?)?;
    let text = self.text(leaf)?;
    Some(match mark {
      Some(Mark::Role { role, shape, attribute }) => (leaf, text, role, shape, attribute),
      _ => (leaf, text, role, None, false),
    })
  }

  /// The class of an attribute's name: `ObsoleteAttribute` of `[Obsolete]`, qualified as the name
  fn attribute_class_reference(&mut self, name: &str, line: u32, qualifier: Option<Vec<u32>>) {
    let class = format!("{name}Attribute");
    let Ok(index) = self.attribute_classes.binary_search(&class) else { return };
    let class: &'t str = &self.attribute_classes[index];
    self.reference(class, line, Role::Type, qualifier, None);
  }

  /// The receiver `object` when it is a bound name with a declared type
  fn bound_receiver(&self, object: NodeId) -> Option<TypedReceiver> {
    if self.kind(object) != Some(N::IdentifierName) {
      return None;
    }
    let id = self.core.out.name_id(self.simple_name_text(object)?)?;
    TypedReceiver::bound(self.core.bindings.binding(id)?)
  }

  /// A reference, unless its name is the discard or a bare name bound around it
  fn reference(&mut self, name: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>, shape: Option<u32>) {
    if is_reference_name(name) {
      self.core.reference(name, line, role, qualifier, shape, None);
    }
  }

  /// The qualifier of a member of `object`: its path of names, or empty for an expression or a path from a local
  fn qualifier_of(&mut self, object: NodeId) -> Vec<u32> {
    let path = self.path_of(object);
    self.core.qualifier_ids(path)
  }

  /// `a.b.c` as `[a, b, c]`: simple names, qualified names, member accesses, `this`, `base`, a predefined type
  /// (`int.Parse`), `alias::name` (without `global`); else `None`, also past `MAX_PATH` segments
  fn path_of(&self, id: NodeId) -> Option<Vec<&'t str>> {
    let tree = self.tree;
    let mut reversed = Vec::new();
    let mut node = id;
    loop {
      if reversed.len() == MAX_PATH {
        return None;
      }
      match self.kind(node)? {
        N::IdentifierName | N::GenericName => {
          reversed.push(self.simple_name_text(node)?);
          break;
        }
        N::QualifiedName => {
          reversed.push(self.simple_name_text(sg::qualified_name::right(tree, node)?)?);
          node = sg::qualified_name::left(tree, node)?;
        }
        N::SimpleMemberAccessExpression => {
          reversed.push(self.simple_name_text(sg::member_access_expression::name(tree, node)?)?);
          node = sg::member_access_expression::expression(tree, node)?;
        }
        N::AliasQualifiedName => {
          reversed.push(self.simple_name_text(sg::alias_qualified_name::name(tree, node)?)?);
          let alias = sg::alias_qualified_name::alias(tree, node).and_then(|it| self.simple_name_text(it))?;
          if alias != "global" {
            reversed.push(alias);
          }
          break;
        }
        N::ThisExpression => {
          reversed.push("this");
          break;
        }
        N::BaseExpression => {
          reversed.push("base");
          break;
        }
        N::PredefinedType => {
          reversed.push(sg::predefined_type::keyword(tree, node).and_then(|it| self.text(it))?);
          break;
        }
        _ => return None,
      }
    }
    reversed.reverse();
    Some(reversed)
  }

  /// The path of a type, without its type arguments: of a name, a nullable type's element (`Item?`)
  fn type_path(&self, ty: NodeId) -> Option<Vec<&'t str>> {
    match self.kind(ty)? {
      N::NullableType => self.type_path(sg::nullable_type::element_type(self.tree, ty)?),
      N::IdentifierName | N::GenericName | N::QualifiedName | N::AliasQualifiedName | N::PredefinedType => self.path_of(ty),
      _ => None,
    }
  }

  /// The last simple name of a name: itself, the right of a qualified name, the name of an alias-qualified one
  fn last_simple_name(&self, name: NodeId) -> Option<NodeId> {
    match self.kind(name)? {
      N::IdentifierName | N::GenericName => Some(name),
      N::QualifiedName => sg::qualified_name::right(self.tree, name),
      N::AliasQualifiedName => sg::alias_qualified_name::name(self.tree, name),
      _ => None,
    }
  }

  // --- reading the tree ---

  fn kind(&self, id: NodeId) -> Option<N> {
    self.tree.node_kind(id)
  }

  /// The text of a simple name: its identifier's value (`class` of `@class`)
  fn simple_name_text(&self, name: NodeId) -> Option<&'t str> {
    read::name_leaf(self.tree, name).and_then(|it| self.text(it))
  }

  /// The text of a leaf: an identifier's value (`class` of `@class`), else the leaf as written
  fn text(&self, id: NodeId) -> Option<&'t str> {
    if self.tree.token_kind(id) == Some(T::IdentifierToken) {
      return read::word(self.tree, id);
    }
    self.core.text_of(id)
  }

  /// Whether a node lies in a structured trivia: a documentation comment, a directive
  fn in_structured_trivia(&self, id: NodeId) -> bool {
    std::iter::successors(self.tree.parent(id), |it| self.tree.parent(*it)).any(|it| read::is_structured_trivia(self.tree, it))
  }

  /// A declaration's range from its first token to its last, without the trivia at its edges and a separator
  fn own_range(&self, id: NodeId) -> std::ops::Range<usize> {
    self
      .tree
      .own_range(id, |it| read::is_trivia(self.tree, it), |it| matches!(it, T::SemicolonToken | T::CommaToken))
  }

  /// Whether the subtree of `id` holds a token the parser skipped or found missing
  fn has_error(&self, id: NodeId) -> bool {
    has_error_in(&self.errors, id.0, self.tree.subtree_end(id).0)
  }

  /// Whether the declaration whose name is `name` was recovered around: a token skipped or missing between the start of
  /// the node it names and its name
  fn in_error(&self, name: NodeId) -> bool {
    let Some(owner) = self.tree.parent(name) else { return false };
    let index = self.errors.partition_point(|it| *it < owner.0);
    self.errors.get(index).is_some_and(|it| *it <= name.0)
  }
}

/// Whether a name is one a reference may have: not empty, not the discard `_`
fn is_reference_name(name: &str) -> bool {
  !name.is_empty() && name != "_"
}

/// An attribute's name without the `Attribute` its class has (`Obsolete` of `ObsoleteAttribute`)
fn strip_attribute(name: &str) -> &str {
  name.strip_suffix("Attribute").filter(|it| !it.is_empty()).unwrap_or(name)
}

/// A name in the convention of an interface: `I` and a capital (`IDisposable`)
fn is_interface_name(name: &str) -> bool {
  let mut chars = name.chars();
  chars.next() == Some('I') && chars.next().is_some_and(char::is_uppercase)
}

/// The modifiers a header leaves out: those that the reach, `overridable` and `overrides` say or that no use depends on
const fn is_dropped(token: T) -> bool {
  matches!(
    token,
    T::PublicKeyword
      | T::ProtectedKeyword
      | T::PrivateKeyword
      | T::InternalKeyword
      | T::FileKeyword
      | T::VirtualKeyword
      | T::OverrideKeyword
      | T::AbstractKeyword
      | T::SealedKeyword
      | T::ReadOnlyKeyword
      | T::VolatileKeyword
      | T::ExternKeyword
      | T::UnsafeKeyword
      | T::NewKeyword
      | T::PartialKeyword
      | T::AsyncKeyword
      | T::RequiredKeyword
      | T::SemicolonToken
  )
}

#[cfg(test)]
#[path = "summary_test.rs"]
mod tests;
