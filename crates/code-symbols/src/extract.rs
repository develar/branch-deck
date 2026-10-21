//! One preorder walk over a tree-sitter tree, driven by per-language tables of node kinds.
//!
//! - **Tables, not queries**: each language lists its node kinds with a `Rule` from the shared vocabulary below
//!   (`kotlin.rs`, `java.rs`, `rust_lang.rs`, `typescript.rs`); the few constructs the vocabulary cannot say are a
//!   `Rule::Special` with a function of that language. The rules are resolved once per grammar into a table indexed by kind id, and
//!   `every_spec_kind_exists_in_its_grammar` checks every kind and field name against the grammar, so a grammar
//!   upgrade that renames a node fails a test instead of losing names silently.
//! - **Iterative**: a `TreeCursor` walk with an explicit frame stack, so a deeply nested file cannot overflow the
//!   stack. A parent decides what its children are before the walk reaches them (a declaration's name, a label, the
//!   callee of a call) by marking them.
//! - **Locals are dropped**: a name bound in a scope (a parameter, a local variable, a pattern, a type parameter) is no
//!   reference anywhere in that scope, and a declaration inside a function body is a local, not a declaration. A
//!   binding is visible from its declaration to the end of its scope, except in TypeScript, where `let`, `const`,
//!   `class` and `function` are visible in their whole block and `var` in its whole function (`Spec::hoist`).
//! - **Overrides** (`Spec::jvm`, Kotlin and Java): a class-like node, local or not, pushes its supertypes onto a
//!   stack that its frame pops, and a function or a Kotlin lambda pushes an empty list, so a member sees the supertypes
//!   of the class it is directly in. A Kotlin `override` or Java `@Override` member is a `Role::Override` reference per
//!   supertype on top of the stack, with its parameter types as a `Shape::Override`.
//! - **Calls** (`Spec::call_shape`, Kotlin and Java): a call gives the reference of its callee, through the mark of the
//!   callee, a `Shape::Call` of its arguments. It is `enclosed` when the stack of supertypes is deeper than at the
//!   declaration around it: a Kotlin lambda or anonymous function, an object expression, an anonymous class or a local
//!   declaration lies between them, each of which pushes onto it.
//! - **Constructors** (`JvmHooks::constructors`, Kotlin and Java): a class (not an interface, an object, an enum or
//!   an annotation) has the constructors its header declares, those its body adds (`Walk::add_constructor`: a Kotlin
//!   secondary constructor, a Java constructor), and without any the implicit `()`, added when the walk is done. Those
//!   of a class with a parse error in it, which may have swallowed one, are unknown. A
//!   Kotlin constructor invocation of a supertype and a Java `new` call the type (`Walk::give_call`), with the shape
//!   taken before an object expression or an anonymous class pushes its supertypes: they and their arguments are of the
//!   code around it.
//! - **Bounded**: files over `MAX_BYTES`, `MAX_LINES` or `MAX_AVERAGE_LINE` (minified) are not parsed, and the parse
//!   stops after `PARSE_BUDGET` progress callbacks, a count of parser operations, so the same bytes always give the same
//!   summary, unlike a wall-clock timeout. Either gives a `truncated` summary.

use crate::language::Language;
use crate::summary::{Argument, Constructor, DeclKind, Declaration, FileSummary, Import, ImportTarget, MemberKind, Reach, Reference, Role, Shape, Signature, TypeRef};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::sync::OnceLock;
use tree_sitter::{Node, ParseOptions, ParseState, Parser, Tree};

/// The version of the extraction rules; bump it on any rule change, so that cached summaries are made again
pub const RULES_VERSION: u32 = 17;
/// Files above this size are not parsed
pub const MAX_BYTES: usize = 1 << 20;
/// Files with more lines are not parsed
pub const MAX_LINES: u32 = 20_000;
/// Files whose average line is longer are minified or generated, and not parsed
pub const MAX_AVERAGE_LINE: u32 = 500;
/// The progress callbacks a parse may take; tree-sitter calls it every 100 parser operations. The densest of 922 files
/// of the IntelliJ monorepo took 19 per KiB, so a file at `MAX_BYTES` needs about 20 000; the rest is for error
/// recovery, which this cuts at 5 million operations.
pub const PARSE_BUDGET: u32 = 50_000;

/// The summary of one file; never fails, a file that cannot be summarized gives a `truncated` summary
pub fn summarize(language: Language, source: &[u8]) -> FileSummary {
  summarize_with_budget(language, source, PARSE_BUDGET)
}

pub(crate) fn summarize_with_budget(language: Language, source: &[u8], budget: u32) -> FileSummary {
  let lines = count_lines(source);
  let mut summary = FileSummary {
    language,
    package: None,
    jvm_name: None,
    names: Vec::new(),
    imports: Vec::new(),
    declarations: Vec::new(),
    references: Vec::new(),
    shapes: Vec::new(),
    error_bytes: 0,
    total_bytes: u32::try_from(source.len()).unwrap_or(u32::MAX),
    lines,
    truncated: true,
  };
  let Some(spec) = spec_of(language) else {
    return summary;
  };
  if source.len() > MAX_BYTES || lines > MAX_LINES || summary.total_bytes > lines.saturating_mul(MAX_AVERAGE_LINE) {
    return summary;
  }
  let table = table_of(spec);
  let Some(tree) = parse(table, source, budget) else {
    return summary;
  };
  let mut walk = Walk::new(spec, table, source);
  walk.run(tree.root_node());
  walk.finish(&mut summary);
  summary.truncated = false;
  summary
}

/// A stable hash of the rules and the grammars of this build: a cached summary is valid only for the same value. FNV-1a
/// over `RULES_VERSION` and the `node-types.json` of each enabled grammar, so it is the same across runs and compilers.
pub fn extractor_version() -> u32 {
  let mut hash = Fnv::new();
  hash.write(&RULES_VERSION.to_le_bytes());
  for language in Language::ALL {
    if let Some(spec) = spec_of(language) {
      hash.write(&[language.id()]);
      hash.write(spec.node_types.as_bytes());
    }
  }
  hash.finish()
}

/// The tree-sitter grammar of a language in this build, for tools that look at the syntax tree itself
pub fn grammar(language: Language) -> Option<tree_sitter::Language> {
  spec_of(language).map(|spec| (spec.grammar)())
}

struct Fnv(u32);

impl Fnv {
  const fn new() -> Self {
    Self(0x811c_9dc5)
  }

  fn write(&mut self, bytes: &[u8]) {
    for byte in bytes {
      self.0 ^= u32::from(*byte);
      self.0 = self.0.wrapping_mul(0x0100_0193);
    }
  }

  const fn finish(&self) -> u32 {
    self.0
  }
}

fn count_lines(source: &[u8]) -> u32 {
  let newlines = source.iter().fold(0, |count, it| count + usize::from(*it == b'\n'));
  let partial = usize::from(source.last().is_some_and(|it| *it != b'\n'));
  u32::try_from(newlines + partial).unwrap_or(u32::MAX)
}

pub(crate) fn spec_of(language: Language) -> Option<&'static Spec> {
  match language {
    #[cfg(feature = "kotlin")]
    Language::Kotlin => Some(&crate::kotlin::SPEC),
    #[cfg(feature = "java")]
    Language::Java => Some(&crate::java::SPEC),
    #[cfg(feature = "rust")]
    Language::Rust => Some(&crate::rust_lang::SPEC),
    #[cfg(feature = "typescript")]
    Language::TypeScript => Some(&crate::typescript::SPEC),
    #[cfg_attr(
      all(feature = "kotlin", feature = "java", feature = "rust", feature = "typescript"),
      expect(unreachable_patterns, reason = "every arm above is behind a feature")
    )]
    _ => None,
  }
}

fn parse(table: &Table, source: &[u8], budget: u32) -> Option<Tree> {
  parse_language(&table.language, source, budget)
}

/// A parse that stops after `budget` progress callbacks, on one parser per thread; the highlighting parses with it too
pub(crate) fn parse_language(language: &tree_sitter::Language, source: &[u8], budget: u32) -> Option<Tree> {
  thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
  }
  PARSER.with_borrow_mut(|parser| {
    parser.set_language(language).ok()?;
    let mut calls = 0u32;
    let mut progress = |_: &ParseState| {
      calls += 1;
      if calls > budget { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    };
    let options = ParseOptions::new().progress_callback(&mut progress);
    let tree = parser.parse_with_options(&mut |offset, _| source.get(offset..).unwrap_or_default(), None, Some(options));
    if tree.is_none() {
      // a cancelled parse keeps its state to resume; the next file must start afresh
      parser.reset();
    }
    tree
  })
}

/// One language: its grammar, its rules and the code of its special constructs
pub(crate) struct Spec {
  pub(crate) language: Language,
  pub(crate) grammar: fn() -> tree_sitter::Language,
  pub(crate) node_types: &'static str,
  pub(crate) rules: &'static [(&'static str, Rule)],
  /// The identifier kinds a binding pattern binds
  pub(crate) binding_names: &'static [&'static str],
  /// Names that are never references (`var` in Java, `Self` in Rust)
  pub(crate) reserved: &'static [&'static str],
  /// The named node kinds, anonymous ones (keywords, operators) and fields the language code reads by name besides
  /// those of the rules, checked against the grammar like the rules
  pub(crate) kinds: &'static [&'static str],
  pub(crate) tokens: &'static [&'static str],
  pub(crate) fields: &'static [&'static str],
  /// The kind, reach and member kind of a declaration; `None` when the node declares nothing here
  pub(crate) traits: for<'t> fn(&mut Walk<'t>, Node<'t>, &DeclRule) -> Option<Traits>,
  /// Whether a name in a pattern binds (Rust: a capitalized one is a variant or a constant)
  pub(crate) binds: fn(&str) -> bool,
  /// Binds the names visible in a whole scope before the walk enters it
  pub(crate) hoist: Option<for<'t> fn(&mut Walk<'t>, Node<'t>)>,
  /// The hooks of a JVM language (Kotlin, Java): signatures, overriding, supertypes and constructors
  pub(crate) jvm: Option<JvmHooks>,
  /// What the header of a function or a value is (`Declaration::header`)
  pub(crate) header: Header,
  /// Of a JVM language, the coroutine context a node opens for the references below it (`Reference::coroutine`):
  /// `Some(context)` for a function or a lambda, `None` for any other node; outside them the context is `Some(false)`.
  /// `None` for a language without suspend functions
  pub(crate) coroutine: Option<CoroutineRule>,
  /// The arguments of a call node (a `Rule::Call`, a `Rule::Member` of the role `Call`, a call of `Walk::give_call`),
  /// for the `Shape::Call` of the reference of its callee; `None` when it says nothing of them
  pub(crate) call_shape: Option<for<'t> fn(&mut Walk<'t>, Node<'t>) -> Option<CallArguments>>,
}

/// The node kinds that end and shorten the header of a function or a value (`Declaration::header`); a token is matched by
/// its text, a named node by its kind
pub(crate) struct Header {
  /// Named kinds that end it: a body, a delegate, an accessor
  pub(crate) ends: &'static [&'static str],
  /// Tokens that end it outside a parameter list: the `=` of an initializer
  pub(crate) end_tokens: &'static [&'static str],
  /// Named kinds left out with all below them: annotations, and the modifiers that the reach or the overriding say
  pub(crate) drops: &'static [&'static str],
  /// Tokens left out: modifiers no use depends on
  pub(crate) drop_tokens: &'static [&'static str],
  /// The kinds of a parameter list: left out where `signature` holds the parameters, else kept, `=` included
  pub(crate) parameters: &'static [&'static str],
}

/// The coroutine context of the code a node holds (`Spec::coroutine`, `Reference::coroutine`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
  /// A suspend call is allowed: a `suspend` function, the lambda of a coroutine builder
  Coroutine,
  /// It isn't: any other function, an initializer, an accessor
  Plain,
  /// A lambda that a suspend function type may take
  Unknown,
}

/// The context a node opens for the references below it, `None` for a node that opens none
pub(crate) type CoroutineRule = for<'t> fn(&Walk<'t>, Node<'t>) -> Option<Context>;

/// The type of a value as `JvmHooks::value_type` reads it: its path, and whether a constructor call gave it
pub(crate) type ValueType<'t> = (Vec<&'t str>, bool);

/// The arguments of a call in source order, each with its name if it has one, and whether one spreads an array
pub(crate) type CallArguments = (Vec<(Option<u32>, Argument)>, bool);

/// What a declaration says about overriding, asked at the start of `Walk::declaration`, local or not, and by the
/// language code that declares around it (a Kotlin constructor property), and the constructors of a class
pub(crate) struct JvmHooks {
  /// The parameters of a function declaration
  pub(crate) signature: for<'t> fn(&mut Walk<'t>, Node<'t>) -> Option<Signature>,
  /// The modifiers of a function or property declaration
  pub(crate) overrides: for<'t> fn(&mut Walk<'t>, Node<'t>) -> Modality,
  /// `Some` for a class-like node, also an empty list: the supertypes whose members the declarations directly in it
  /// override (an enum entry: its enum)
  pub(crate) supertypes: for<'t> fn(&mut Walk<'t>, Node<'t>) -> Option<Vec<Vec<u32>>>,
  /// The named leaves that are types besides type names (Java `int`), for `TypeRef`
  pub(crate) primitives: &'static [&'static str],
  /// The constructors the header of a class declares, given the reach of the class: a Kotlin primary constructor, the
  /// canonical constructor of a Java record; its body adds the others (`Walk::add_constructor`)
  pub(crate) constructors: for<'t> fn(&mut Walk<'t>, Node<'t>, Reach) -> Vec<Constructor>,
  /// The reach of the implicit `()` of a class that declares no constructor, given the reach of the class
  pub(crate) implicit_constructor: fn(Reach) -> Reach,
  /// The type of a declaration of a value (a property, a field, a local, a parameter): as written, without type
  /// arguments, else the type its initializer constructs; `None` for a type parameter, a function type and an
  /// inferred type of any other expression (`Declaration::value_type`); with whether a constructor call gave it
  pub(crate) value_type: for<'t> fn(&Walk<'t>, Node<'t>) -> Option<ValueType<'t>>,
  /// The node a receiver of a member names a value by: a name inside a Kotlin `m!!`, else the node itself
  pub(crate) receiver_node: for<'t> fn(&Walk<'t>, Node<'t>) -> Node<'t>,
}

/// What the modifiers of a member say about overriding
#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a declaration")]
pub(crate) struct Modality {
  pub(crate) overrides: bool,
  pub(crate) overridable: bool,
  /// A Kotlin `@JvmStatic` member
  pub(crate) jvm_static: bool,
  /// A Kotlin `@JvmField` property
  pub(crate) jvm_field: bool,
  /// A Kotlin member that `@JvmName` (also `@get:` or `@set:`) names otherwise for Java, or `@JvmSynthetic` hides
  pub(crate) jvm_name: bool,
  /// A Kotlin `lateinit` property
  pub(crate) lateinit: bool,
  /// The reach of the setter of a Kotlin `var`
  pub(crate) setter: Option<Reach>,
  /// The extension receiver of a Kotlin member, for the shape of its override
  pub(crate) receiver: Option<u32>,
}

/// The fields of a `Declaration` that `JvmHooks` fills
#[derive(Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a declaration")]
pub(crate) struct JvmFields {
  signature: Option<Signature>,
  overrides: bool,
  overridable: bool,
  jvm_static: bool,
  jvm_field: bool,
  jvm_name: bool,
  lateinit: bool,
  setter: Option<Reach>,
  supertypes: Vec<Vec<u32>>,
  value_type: Option<Typed>,
  constructors: Option<Vec<Constructor>>,
}

/// What a node kind means; the shared vocabulary of the language tables
#[derive(Debug, Clone, Copy)]
pub(crate) enum Rule {
  /// No names below; a doc comment (`/**`, `///`) extends the declaration that follows it
  Comment,
  /// No names below: labels, lifetimes, the token patterns of a macro
  Skip,
  /// An identifier; a bare one is a reference with this role
  Name(Role),
  /// `this`, `self`, `super`, `crate`: a qualifier segment, never a reference
  Keyword(&'static str),
  /// `object.name`, `path::name`: `name` is a reference qualified by the path of `object`; `object` is walked
  Member {
    object: Child,
    name: Child,
    role: Option<Role>,
  },
  /// Kotlin `a.b.C<T>.D`: each type name qualified by the names before it
  Path,
  /// A call or an annotation: the callee (a name, a member or a `Forward`) gets `role`
  Call {
    callee: Child,
    role: Role,
  },
  /// A generic wrapper (`foo::<T>`, `List<T>`) passes the role it gets on to this child
  Forward(Child),
  /// This child is a label, not a reference: a named argument, an object key
  Label(Child),
  /// A scope and the names it binds
  Bind(Binder),
  /// These children of a pattern are no bindings: a struct pattern's type, a default value
  Unbind(&'static [Child]),
  /// A statement around declarators (`int a, b;`): its start and doc comment go to its first declaration
  Wrapper,
  Decl(DeclRule),
  /// A construct of one language, handled by its function, which returns whether the walk descends into it
  Special(Special),
}

pub(crate) type Special = for<'t> fn(&mut Walk<'t>, Node<'t>) -> bool;

/// A child selected by a rule
#[derive(Debug, Clone, Copy)]
pub(crate) enum Child {
  Itself,
  Field(&'static str),
  /// The first named child of one of these kinds
  Kind(&'static [&'static str]),
  /// The n-th named child
  Nth(u8),
  Last,
}

/// A scope (`scope`) and the names bound in it or in the enclosing one
#[derive(Debug, Clone, Copy)]
pub(crate) struct Binder {
  pub(crate) scope: bool,
  /// Names or patterns that bind
  pub(crate) names: &'static [Child],
  /// The names become visible when this node is left (`let x = x;`), not when the pattern is
  pub(crate) late: bool,
  /// Names the scope binds without a declaration: Kotlin `it`, `field`
  pub(crate) implicit: &'static [&'static str],
  /// Children that are no references (the name of a Java constructor)
  pub(crate) labels: &'static [Child],
  /// A lambda: the declarations in it override nothing of the class around it, also where the grammar misreads the
  /// body of a one-line object expression as a lambda, unless the class marked it as its body (`Walk::mark_class_body`)
  pub(crate) lambda: bool,
}

pub(crate) const SCOPE: Rule = Rule::Bind(binder(true, &[]));

pub(crate) const fn binder(scope: bool, names: &'static [Child]) -> Binder {
  Binder {
    scope,
    names,
    late: false,
    implicit: &[],
    labels: &[],
    lambda: false,
  }
}

/// The scope a declaration opens for its children
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Opens {
  Nothing,
  /// Type parameters and constructor parameters; the members are still declarations
  Members,
  /// A function: everything inside is local
  Body,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeclRule {
  pub(crate) kind: DeclKind,
  pub(crate) name: Child,
  pub(crate) opens: Opens,
  /// In a local scope, the name becomes visible when the node is left (`val x = x`), not at once (recursion)
  pub(crate) late: bool,
  /// A child that is a pattern of several names (Kotlin `val (a, b)`, TypeScript `const { a, b }`)
  pub(crate) pattern: Option<Child>,
  /// The name of a declaration without one in the source (Kotlin `companion object`)
  pub(crate) default_name: Option<&'static str>,
  /// A child whose members are locals (the body of an enum entry overrides members declared elsewhere)
  pub(crate) local_body: Option<Child>,
}

/// The rules of node kinds that several grammars have
pub(crate) const IDENTIFIER: (&str, Rule) = ("identifier", Rule::Name(Role::Value));
pub(crate) const TYPE_IDENTIFIER: (&str, Rule) = ("type_identifier", Rule::Name(Role::Type));
pub(crate) const LINE_COMMENT: (&str, Rule) = ("line_comment", Rule::Comment);
pub(crate) const BLOCK_COMMENT: (&str, Rule) = ("block_comment", Rule::Comment);

/// A type, whose type parameters are visible in it
pub(crate) const fn type_decl(name: Child) -> Rule {
  Rule::Decl(DeclRule {
    opens: Opens::Members,
    ..decl(DeclKind::Type, name)
  })
}

/// A function, whose parameters and body are local
pub(crate) const fn function_decl(name: Child) -> Rule {
  Rule::Decl(DeclRule {
    opens: Opens::Body,
    ..decl(DeclKind::Function, name)
  })
}

pub(crate) const fn decl(kind: DeclKind, name: Child) -> DeclRule {
  DeclRule {
    kind,
    name,
    opens: Opens::Nothing,
    late: false,
    pattern: None,
    default_name: None,
    local_body: None,
  }
}

/// What `Spec::traits` says about a declaration
#[derive(Debug, Clone, Copy)]
pub(crate) struct Traits {
  pub(crate) kind: DeclKind,
  pub(crate) reach: Reach,
  pub(crate) member: MemberKind,
  pub(crate) receiver: Option<u32>,
  /// What the declaration is to its own members
  pub(crate) holder: Holder,
}

/// An item that language code found among the tokens of a macro (`Walk::add_token_declaration`)
pub(crate) struct TokenItem<'t> {
  /// Its first token, or the first doc comment or attribute above it
  pub(crate) first: Node<'t>,
  pub(crate) name: Node<'t>,
  pub(crate) last: Node<'t>,
  /// The header of a function, a value or a constant (`Walk::tokens_header`)
  pub(crate) header: String,
}

/// What a container is to the declarations in it, for their member kind and reach
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Holder {
  Class,
  /// Its Java fields are constants and its members public
  Interface,
  /// A Kotlin object or companion: its members are static
  Object,
  Enum,
  Annotation,
  Trait,
  Impl,
  TraitImpl,
  Module,
  /// A TypeScript type alias of an object type
  Alias,
  /// A function, an enum entry, a property: nothing in it is a member
  Leaf,
}

/// The grammar and the rules of one language, indexed by kind id
pub(crate) struct Table {
  pub(crate) language: tree_sitter::Language,
  rules: Vec<Option<Rule>>,
  binding: Vec<bool>,
  /// Field name to id, sorted by name
  fields: Vec<(Box<str>, u16)>,
}

impl Table {
  fn new(spec: &Spec) -> Self {
    let language = (spec.grammar)();
    let by_name: HashMap<&str, Rule> = spec.rules.iter().copied().collect();
    let count = language.node_kind_count();
    let mut rules = vec![None; count];
    let mut binding = vec![false; count];
    for id in 0..count {
      let Ok(id16) = u16::try_from(id) else { break };
      if !language.node_kind_is_named(id16) {
        continue;
      }
      if let Some(kind) = language.node_kind_for_id(id16) {
        rules[id] = by_name.get(kind).copied();
        binding[id] = spec.binding_names.contains(&kind);
      }
    }
    let mut fields: Vec<(Box<str>, u16)> = (1..=language.field_count())
      .filter_map(|id| u16::try_from(id).ok())
      .filter_map(|id| language.field_name_for_id(id).map(|name| (Box::from(name), id)))
      .collect();
    fields.sort_unstable();
    Self { language, rules, binding, fields }
  }

  fn rule(&self, kind_id: u16) -> Option<Rule> {
    self.rules.get(usize::from(kind_id)).copied().flatten()
  }

  fn is_binding(&self, kind_id: u16) -> bool {
    self.binding.get(usize::from(kind_id)).copied().unwrap_or(false)
  }

  pub(crate) fn field_id(&self, name: &str) -> Option<u16> {
    let index = self.fields.binary_search_by(|it| (*it.0).cmp(name)).ok()?;
    Some(self.fields[index].1)
  }
}

impl Spec {
  /// Every node kind and field name the rules select by, besides the kinds they are for
  pub(crate) fn rule_names(&self) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut kinds = Vec::new();
    let mut fields = Vec::new();
    let mut add = |child: &Child| match child {
      Child::Field(name) => fields.push(*name),
      Child::Kind(names) => kinds.extend_from_slice(names),
      Child::Itself | Child::Nth(_) | Child::Last => {}
    };
    for (_, rule) in self.rules {
      match rule {
        Rule::Member { object, name, .. } => {
          add(object);
          add(name);
        }
        Rule::Call { callee: child, .. } | Rule::Forward(child) | Rule::Label(child) => add(child),
        Rule::Bind(binder) => binder.names.iter().chain(binder.labels).for_each(&mut add),
        Rule::Unbind(children) => children.iter().for_each(&mut add),
        Rule::Decl(decl) => {
          add(&decl.name);
          decl.pattern.iter().chain(&decl.local_body).for_each(&mut add);
        }
        Rule::Comment | Rule::Skip | Rule::Name(_) | Rule::Keyword(_) | Rule::Path | Rule::Wrapper | Rule::Special(_) => {}
      }
    }
    (kinds, fields)
  }
}

pub(crate) fn table_of(spec: &'static Spec) -> &'static Table {
  static TABLES: [OnceLock<Table>; 4] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
  TABLES[usize::from(spec.language.id())].get_or_init(|| Table::new(spec))
}

/// What a parent decided about a child before the walk reaches it
#[derive(Debug, Clone, Copy)]
enum Mark {
  /// A name that is no reference: a declaration's own name, a label, a key
  Claimed,
  /// The role a call, an annotation or a JSX tag gives a name or a member, with the shape of a call's arguments
  Role(Role, Option<u32>),
  /// The root of a binding pattern; its names become visible when the frame at this index is left (`None`: the root)
  Pattern(Option<usize>),
  /// Leaves a pattern: a pattern's type, a default value, a member's object
  Expr,
  /// A body whose declarations are locals
  Scope,
  /// Walked without its rule: a Kotlin lambda that is the misread body of an object
  Plain,
  /// A lambda that is the misread body of the class around it: its declarations override the supertypes of that class
  ClassBody,
}

struct Frame<'t> {
  node: Node<'t>,
  /// Names below bind, and become visible when the frame at this index is left
  pattern: Option<usize>,
  scopes: u8,
  container: bool,
  /// The entries this node pushed onto `Walk::supertypes`
  supertypes: u8,
  /// It pushed its coroutine context onto `Walk::coroutines`
  coroutine: bool,
  error: bool,
  /// The first line of the doc comments and attributes above the next child
  leading: Option<u32>,
  /// The last declaration among the children, for a Kotlin accessor that the grammar puts beside its property
  last_decl: Option<u32>,
  /// Inside a Rust token tree: the path of the `a::b::` (or `a.b.`) tokens before the next one
  tokens: Option<Tokens<'t>>,
}

#[derive(Default)]
struct Tokens<'t> {
  path: Vec<&'t str>,
  after_colons: bool,
  /// The reference of the last name, which a `!` makes a macro
  last: Option<usize>,
}

struct Container {
  /// `None` for a Rust `impl`, which declares nothing itself
  decl: Option<u32>,
  holder: Holder,
  receiver: Option<u32>,
  /// A function or an accessor, whose code sees no parameter of a primary constructor
  code: bool,
  /// The depth of `Walk::supertypes` when it was entered, its own entry included
  supertypes: usize,
}

struct Scope {
  bound_start: usize,
  body: bool,
}

/// A type of a value (`JvmHooks::value_type`): its path, and whether a constructor call of the initializer gave it
#[derive(Debug, Clone, Default)]
pub(crate) struct Typed {
  path: Vec<u32>,
  constructed: bool,
}

/// A name bound in a scope, with the type it was declared with (`JvmHooks::value_type`)
struct Binding {
  name: u32,
  type_path: Option<Typed>,
  /// A parameter of a Kotlin primary constructor, which the initializers of the class see but not its functions and
  /// accessors: the depth of `Walk::containers` it was bound at
  initializers: Option<usize>,
  /// Unique per binding of the walk, for the smart casts that drop its type
  serial: u32,
}

/// A member reference whose receiver may have a type (`Reference::receiver_type`), decided when the walk is done
enum TypedReceiver {
  /// A bound name of this type and binding
  Bound { reference: usize, serial: u32, type_path: Typed },
  /// A property or field `[m]` or `[this, m]`, not enclosed
  Property { reference: usize },
}

/// The state of one walk; the language code reads and writes it through the methods below
pub(crate) struct Walk<'t> {
  spec: &'static Spec,
  table: &'static Table,
  source: &'t [u8],
  name_ids: HashMap<&'t str, u32>,
  names: Vec<&'t str>,
  package: Option<String>,
  jvm_name: Option<String>,
  imports: Vec<Import>,
  declarations: Vec<Declaration>,
  references: Vec<Reference>,
  frames: Vec<Frame<'t>>,
  /// (node id, mark, index of the frame that set it)
  marks: Vec<(usize, Mark, usize)>,
  scopes: Vec<Scope>,
  bound: Vec<Binding>,
  bound_counts: HashMap<u32, u32>,
  serials: u32,
  /// Name node id -> the type its binding gets, set by the language code before the walk binds it
  binding_types: HashMap<usize, Typed>,
  /// Name node ids of the parameters of a Kotlin primary constructor (`Binding::initializers`)
  initializer_names: HashSet<usize>,
  receivers: Vec<TypedReceiver>,
  /// The bindings, and the (container, name) of properties, that a smart cast may give another type
  cast_bindings: HashSet<u32>,
  cast_names: HashSet<(Option<u32>, u32)>,
  body_scopes: u32,
  /// (frame index, name, type): bindings that become visible when that frame is left
  pending: Vec<(usize, u32, Option<Typed>)>,
  containers: Vec<Container>,
  /// The supertypes whose members the declarations at this level override; a class-like node pushes its own, a
  /// function body and a lambda an empty list
  supertypes: Vec<Vec<Vec<u32>>>,
  /// The coroutine contexts of the functions and lambdas around the current node (`Spec::coroutine`)
  coroutines: Vec<Context>,
  shapes: Vec<Shape>,
  shape_ids: HashMap<Shape, u32>,
  error_bytes: u32,
  error_depth: u32,
  /// The leading line taken from the parent for the node being entered
  leading_now: Option<u32>,
  /// TypeScript `export { x }`: top-level names whose declarations become public
  exported: Vec<u32>,
}

impl<'t> Walk<'t> {
  fn new(spec: &'static Spec, table: &'static Table, source: &'t [u8]) -> Self {
    Self {
      spec,
      table,
      source,
      name_ids: HashMap::new(),
      names: Vec::new(),
      package: None,
      jvm_name: None,
      imports: Vec::new(),
      declarations: Vec::new(),
      references: Vec::new(),
      frames: Vec::new(),
      marks: Vec::new(),
      scopes: Vec::new(),
      bound: Vec::new(),
      bound_counts: HashMap::new(),
      serials: 0,
      binding_types: HashMap::new(),
      initializer_names: HashSet::new(),
      receivers: Vec::new(),
      cast_bindings: HashSet::new(),
      cast_names: HashSet::new(),
      body_scopes: 0,
      pending: Vec::new(),
      containers: Vec::new(),
      supertypes: Vec::new(),
      coroutines: Vec::new(),
      shapes: Vec::new(),
      shape_ids: HashMap::new(),
      error_bytes: 0,
      error_depth: 0,
      leading_now: None,
      exported: Vec::new(),
    }
  }

  fn run(&mut self, root: Node<'t>) {
    let mut cursor = root.walk();
    'nodes: loop {
      let node = cursor.node();
      let named = node.is_named();
      let descend = if named { self.enter(node) } else { self.token(node) };
      if descend && cursor.goto_first_child() {
        continue;
      }
      if named {
        self.leave();
      }
      loop {
        if cursor.goto_next_sibling() {
          continue 'nodes;
        }
        if !cursor.goto_parent() {
          break 'nodes;
        }
        self.leave();
      }
    }
  }

  /// An anonymous node; it has no frame and is never descended into
  fn token(&mut self, node: Node<'t>) -> bool {
    if node.is_missing() && self.error_depth == 0 {
      self.error_bytes = self.error_bytes.saturating_add(1);
    }
    if let Some(tokens) = self.frames.last_mut().and_then(|it| it.tokens.as_mut()) {
      match node.kind() {
        "::" | "." => tokens.after_colons = true,
        kind => {
          if let (Some(last), "!") = (tokens.last, kind) {
            self.references[last].role = Role::Macro;
          }
          tokens.path.clear();
          tokens.after_colons = false;
        }
      }
      tokens.last = None;
    }
    false
  }

  fn enter(&mut self, node: Node<'t>) -> bool {
    let index = self.frames.len();
    let mark = self.take_mark(node.id());
    let mut pattern = self.frames.last().and_then(|it| it.pattern);
    match mark {
      Some(Mark::Expr) => pattern = None,
      Some(Mark::Pattern(at)) => pattern = Some(at.unwrap_or(index)),
      _ => {}
    }
    let mut frame = Frame {
      node,
      pattern,
      scopes: 0,
      container: false,
      supertypes: 0,
      coroutine: false,
      error: false,
      leading: None,
      last_decl: None,
      tokens: None,
    };
    if let Some(context) = self.spec.coroutine.and_then(|opens| opens(self, node)) {
      self.coroutines.push(context);
      frame.coroutine = true;
    }
    if node.is_error() || node.is_missing() {
      if self.error_depth == 0 {
        let bytes = u32::try_from(node.byte_range().len()).unwrap_or(u32::MAX).max(u32::from(node.is_missing()));
        self.error_bytes = self.error_bytes.saturating_add(bytes);
      }
      self.error_depth += 1;
      frame.error = true;
    }
    let rule = if matches!(mark, Some(Mark::Plain)) { None } else { self.table.rule(node.kind_id()) };
    self.leading_now = match rule {
      Some(Rule::Comment) => None,
      _ => self.frames.last_mut().and_then(|it| it.leading.take()),
    };
    self.frames.push(frame);
    if matches!(mark, Some(Mark::Scope)) {
      self.push_scope(true);
    }
    match rule {
      None => !matches!(mark, Some(Mark::Claimed)),
      Some(Rule::Comment) => {
        if self.is_doc_comment(node) {
          self.extend_leading(node);
        }
        false
      }
      Some(Rule::Skip) => false,
      Some(Rule::Name(role)) => {
        self.name(node, mark, role);
        false
      }
      Some(Rule::Keyword(keyword)) => {
        if let Some(tokens) = self.frames.len().checked_sub(2).and_then(|it| self.frames[it].tokens.as_mut()) {
          if !tokens.after_colons {
            tokens.path.clear();
          }
          tokens.path.push(keyword);
          tokens.after_colons = false;
        }
        true
      }
      Some(Rule::Member { object, name, role }) => {
        self.member(node, mark, object, name, role);
        true
      }
      Some(Rule::Path) => {
        self.path(node, mark);
        true
      }
      Some(Rule::Call { callee, role }) => {
        if let Some(callee) = self.child(node, callee) {
          let shape = if role == Role::Call { self.call_shape(node) } else { None };
          self.mark(callee, Mark::Role(role, shape));
        }
        true
      }
      Some(Rule::Forward(child)) => {
        if let (Some(Mark::Role(role, shape)), Some(child)) = (mark, self.child(node, child)) {
          self.mark(child, Mark::Role(role, shape));
        }
        true
      }
      Some(Rule::Label(child)) => {
        if let Some(child) = self.child(node, child).filter(|it| self.is_name(*it)) {
          self.claim(child);
        }
        true
      }
      Some(Rule::Bind(binder)) => {
        self.bind(node, &binder);
        if binder.lambda && !matches!(mark, Some(Mark::ClassBody)) {
          self.push_supertypes(Vec::new());
        }
        true
      }
      Some(Rule::Unbind(children)) => {
        for child in children {
          if let Some(child) = self.child(node, *child) {
            self.mark(child, Mark::Expr);
          }
        }
        true
      }
      Some(Rule::Wrapper) => {
        self.pass_leading(node);
        true
      }
      Some(Rule::Decl(rule)) => {
        self.declaration(node, &rule);
        true
      }
      Some(Rule::Special(handler)) => handler(self, node),
    }
  }

  fn leave(&mut self) {
    let Some(frame) = self.frames.pop() else { return };
    let index = self.frames.len();
    while self.marks.last().is_some_and(|it| it.2 >= index) {
      self.marks.pop();
    }
    for _ in 0..frame.scopes {
      self.pop_scope();
    }
    while self.pending.last().is_some_and(|(at, ..)| *at >= index) {
      if let Some((_, name, type_path)) = self.pending.pop() {
        self.bind_typed(name, type_path);
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

  fn name(&mut self, node: Node<'t>, mark: Option<Mark>, role: Role) {
    if matches!(mark, Some(Mark::Claimed)) {
      return;
    }
    let Some(text) = self.text(node) else { return };
    let frame = self.frames.len() - 1;
    let binds = matches!(mark, Some(Mark::Pattern(_)))
      || (self.frames[frame].pattern.is_some() && self.table.is_binding(node.kind_id()) && (self.spec.binds)(text) && !matches!(mark, Some(Mark::Role(..))));
    if binds {
      let at = self.frames[frame].pattern.unwrap_or(frame);
      let id = self.intern(text);
      let type_path = self.binding_types.remove(&node.id());
      self.pending.push((at, id, type_path));
      return;
    }
    let (role, shape) = match mark {
      Some(Mark::Role(role, shape)) => (role, shape),
      _ => (role, None),
    };
    let qualifier = self.token_qualifier(text);
    let before = self.references.len();
    self.reference(text, line_of(node), role, qualifier, shape);
    if let Some(tokens) = self.frames.len().checked_sub(2).and_then(|it| self.frames[it].tokens.as_mut()) {
      tokens.last = (self.references.len() > before).then_some(before);
    }
  }

  /// In a Rust token tree, `a::b::c` are three names, `c` qualified by `a::b`
  fn token_qualifier(&mut self, text: &'t str) -> Option<Vec<u32>> {
    let parent = self.frames.len().checked_sub(2)?;
    let tokens = self.frames[parent].tokens.as_mut()?;
    let qualifier = (tokens.after_colons && !tokens.path.is_empty()).then(|| tokens.path.clone());
    if !tokens.after_colons {
      tokens.path.clear();
    }
    tokens.path.push(text);
    tokens.after_colons = false;
    let qualifier = qualifier?;
    Some(self.qualifier_ids(&qualifier))
  }

  fn member(&mut self, node: Node<'t>, mark: Option<Mark>, object: Child, name: Child, role: Option<Role>) {
    let Some(name_node) = self.child(node, name).and_then(|it| self.name_node(it)) else {
      return;
    };
    let object = self.child(node, object);
    if let Some(object) = object {
      self.mark(object, Mark::Expr);
    }
    self.claim(name_node);
    if matches!(mark, Some(Mark::Claimed)) {
      return;
    }
    let Some(text) = self.text(name_node) else { return };
    let (role, shape) = match (mark, role) {
      (Some(Mark::Role(role, shape)), _) => (role, shape),
      // a Java method invocation is a call of its own
      (_, Some(Role::Call)) => (Role::Call, self.call_shape(node)),
      (_, role) => (role.unwrap_or_else(|| self.default_role(name_node)), None),
    };
    let qualifier = object.map(|object| self.qualifier_of(object));
    let receiver = object
      .filter(|_| matches!(role, Role::Call | Role::Value))
      .and_then(|object| self.typed_receiver(object, qualifier.as_deref()));
    let before = self.references.len();
    self.reference(text, line_of(name_node), role, qualifier, shape);
    if let Some(receiver) = receiver.filter(|_| self.references.len() > before) {
      self.receivers.push(match receiver {
        TypedReceiver::Bound { serial, type_path, .. } => TypedReceiver::Bound {
          reference: before,
          serial,
          type_path,
        },
        TypedReceiver::Property { .. } => TypedReceiver::Property { reference: before },
      });
    }
  }

  /// What the receiver `object` of a member reference may have a type by: a bound name with a declared type, or, not
  /// enclosed, a property `[m]` or `[this, m]` of the qualifier, decided when the walk is done (`receiver_types`)
  fn typed_receiver(&self, object: Node<'t>, qualifier: Option<&[u32]>) -> Option<TypedReceiver> {
    let hooks = self.spec.jvm.as_ref()?;
    let object = (hooks.receiver_node)(self, object);
    if self.is_name(object) {
      let id = *self.name_ids.get(self.text(object)?)?;
      if self.is_bound(id) {
        let binding = self.bound.iter().rev().find(|it| it.name == id)?;
        // a parameter of a primary constructor is no name in a function or an accessor: a property of its name is
        if binding.initializers.is_some_and(|depth| self.containers.iter().skip(depth).any(|it| it.code)) {
          return None;
        }
        return Some(TypedReceiver::Bound {
          reference: 0,
          serial: binding.serial,
          type_path: binding.type_path.clone()?,
        });
      }
    }
    let this = self.name_ids.get("this").copied();
    let property = match qualifier? {
      [_] => true,
      [first, _] => Some(*first) == this,
      _ => false,
    };
    (property && !self.enclosed()).then_some(TypedReceiver::Property { reference: 0 })
  }

  /// The interned `Shape::Call` of the arguments of a call node, by the language's `Spec::call_shape`, `enclosed` by
  /// the walk
  fn call_shape(&mut self, node: Node<'t>) -> Option<u32> {
    let (arguments, spread) = (self.spec.call_shape?)(self, node)?;
    let enclosed = self.enclosed();
    Some(self.intern_shape(Shape::Call { arguments, spread, enclosed }))
  }

  /// A type path, each name qualified by the names before it, which are types: the role and the shape of a mark go to
  /// the last name, the type a Kotlin constructor invocation `a.b.Base(…)` calls (a Kotlin call of `a.b.C(…)` is of a
  /// navigation, not of a type)
  fn path(&mut self, node: Node<'t>, mark: Option<Mark>) {
    let (role, shape) = match mark {
      Some(Mark::Role(role, shape)) => (role, shape),
      _ => (Role::Type, None),
    };
    let mut cursor = node.walk();
    let names: Vec<Node<'t>> = node.named_children(&mut cursor).filter(|it| self.is_name(*it)).collect();
    let mut segments: Vec<&'t str> = Vec::new();
    for (index, child) in names.iter().copied().enumerate() {
      self.claim(child);
      let Some(text) = self.text(child) else { continue };
      let qualifier = if segments.is_empty() { None } else { Some(self.qualifier_ids(&segments)) };
      if !matches!(mark, Some(Mark::Claimed)) {
        let (role, shape) = if index + 1 == names.len() { (role, shape) } else { (Role::Type, None) };
        self.reference(text, line_of(child), role, qualifier, shape);
      }
      segments.push(text);
    }
  }

  fn bind(&mut self, node: Node<'t>, binder: &Binder) {
    let index = self.frames.len() - 1;
    if binder.scope {
      self.push_scope(true);
    }
    for name in binder.implicit {
      self.bind_text(name);
    }
    for label in binder.labels {
      if let Some(label) = self.child(node, *label) {
        self.claim(label);
      }
    }
    for child in binder.names {
      let Some(child) = self.child(node, *child) else { continue };
      if child.id() == node.id() {
        self.frames[index].pattern = Some(index);
      } else {
        self.mark(child, Mark::Pattern(binder.late.then_some(index)));
      }
    }
  }

  fn declaration(&mut self, node: Node<'t>, rule: &DeclRule) {
    let index = self.frames.len() - 1;
    let leading = self.leading_now;
    let mut fields = self.jvm_fields(node, rule);
    if self.body_scopes > 0 {
      // a local declaration binds its name in the scope around it, with its type
      let value_type = fields.value_type.take();
      match rule.pattern.and_then(|it| self.child(node, it)) {
        Some(pattern) => {
          if let (Some(type_path), [name]) = (value_type, self.pattern_names(pattern).as_slice()) {
            self.binding_types.insert(name.id(), type_path);
          }
          self.mark(pattern, Mark::Pattern(rule.late.then_some(index)));
        }
        None => {
          if let Some(name) = self.child(node, rule.name).and_then(|it| self.name_node(it)) {
            self.claim(name);
            if let Some(text) = self.text(name) {
              let id = self.intern(text);
              if rule.late {
                self.pending.push((index, id, value_type));
              } else {
                self.bind_typed(id, value_type);
              }
            }
          }
        }
      }
      if rule.opens != Opens::Nothing {
        self.push_scope(rule.opens == Opens::Body);
      }
      return;
    }
    let names = self.declared_names(node, rule);
    let traits = if names.is_empty() && rule.default_name.is_none() {
      None
    } else {
      (self.spec.traits)(self, node, rule)
    };
    for name in &names {
      self.claim(*name);
    }
    if let Some(traits) = traits {
      let spec = self.spec;
      // a parse error in a class may swallow a constructor: its constructors are unknown then
      if let (Some(hooks), DeclKind::Type, Holder::Class, false) = (&spec.jvm, traits.kind, traits.holder, node.has_error()) {
        fields.constructors = Some((hooks.constructors)(self, node, traits.reach));
      }
      let mut first = rule
        .default_name
        .filter(|_| names.is_empty())
        .map(|name| self.declare(node, node, name, &traits, leading, std::mem::take(&mut fields)));
      for name in names {
        if let Some(text) = self.text(name) {
          // a pattern of several names declares values: only the first name could take a signature or a supertype
          let id = self.declare(node, name, text, &traits, leading, std::mem::take(&mut fields));
          first.get_or_insert(id);
        }
      }
      if let Some(first) = first {
        self.enter_container(Some(first), traits.holder, traits.receiver);
      }
    }
    if let Some(body) = rule.local_body.and_then(|it| self.child(node, it)) {
      self.mark(body, Mark::Scope);
    }
    if rule.opens != Opens::Nothing {
      self.push_scope(rule.opens == Opens::Body);
    }
  }

  /// The name nodes of a declaration: one, or every name of its pattern
  fn declared_names(&self, node: Node<'t>, rule: &DeclRule) -> Vec<Node<'t>> {
    let Some(target) = self.child(node, rule.name) else { return Vec::new() };
    if self.is_name(target) {
      return vec![target];
    }
    if rule.pattern.is_some() {
      return self.pattern_names(target);
    }
    self.name_node(target).into_iter().collect()
  }

  /// The names a pattern binds, without its default values, types and keys
  pub(crate) fn pattern_names(&self, pattern: Node<'t>) -> Vec<Node<'t>> {
    let mut names = Vec::new();
    let mut stack = vec![pattern];
    while let Some(node) = stack.pop() {
      if self.table.is_binding(node.kind_id()) {
        if self.text(node).is_some_and(self.spec.binds) {
          names.push(node);
        }
        continue;
      }
      let skipped: Vec<usize> = match self.table.rule(node.kind_id()) {
        Some(Rule::Member { .. }) => continue,
        Some(Rule::Unbind(children)) => children.iter().filter_map(|it| self.child(node, *it)).map(|it| it.id()).collect(),
        Some(Rule::Label(child)) => self.child(node, child).map(|it| it.id()).into_iter().collect(),
        _ => Vec::new(),
      };
      let mut cursor = node.walk();
      // in source order when popped
      let start = stack.len();
      stack.extend(node.named_children(&mut cursor).filter(|it| !skipped.contains(&it.id())));
      stack[start..].reverse();
    }
    names
  }

  /// What a declaration says about overriding, before the walk enters it, local or not: an override is an `Override`
  /// reference per supertype of the class-like node around it. Then it pushes the supertypes of the declarations
  /// directly in it: a class-like node its own, a function an empty list for its body.
  fn jvm_fields(&mut self, node: Node<'t>, rule: &DeclRule) -> JvmFields {
    let spec = self.spec;
    let Some(hooks) = &spec.jvm else { return JvmFields::default() };
    let name = self.child(node, rule.name).and_then(|it| self.name_node(it));
    let mut fields = self.overriding(hooks, node, name, rule.kind);
    match (hooks.supertypes)(self, node) {
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
  fn overriding(&mut self, hooks: &JvmHooks, node: Node<'t>, name: Option<Node<'t>>, kind: DeclKind) -> JvmFields {
    if !matches!(kind, DeclKind::Function | DeclKind::Value) {
      return JvmFields::default();
    }
    let modality = (hooks.overrides)(self, node);
    // a local function is no declaration: only the shape of an override needs its parameters
    let signature = if kind == DeclKind::Function && (modality.overrides || !self.is_local()) {
      (hooks.signature)(self, node)
    } else {
      None
    };
    if let (true, Some(name)) = (modality.overrides, name) {
      self.override_references(name, kind == DeclKind::Function, modality.receiver, signature.as_ref());
    }
    JvmFields {
      signature,
      overrides: modality.overrides,
      overridable: modality.overridable,
      jvm_static: modality.jvm_static,
      jvm_field: modality.jvm_field,
      jvm_name: modality.jvm_name,
      lateinit: modality.lateinit,
      setter: modality.setter,
      value_type: if kind == DeclKind::Value { self.value_type(hooks, node) } else { None },
      ..JvmFields::default()
    }
  }

  /// One `Override` reference per supertype of the class around the member, qualified by its path
  fn override_references(&mut self, name: Node<'t>, function: bool, receiver: Option<u32>, signature: Option<&Signature>) {
    let Some(supertypes) = self.supertypes.last().filter(|it| !it.is_empty()).cloned() else {
      return;
    };
    let Some(text) = self.text(name) else { return };
    let parameters = signature.map_or_else(Vec::new, |it| it.parameters.iter().map(|parameter| parameter.type_ref.clone()).collect());
    let shape = self.intern_shape(Shape::Override { function, receiver, parameters });
    for supertype in supertypes {
      self.reference(text, line_of(name), Role::Override, Some(supertype), Some(shape));
    }
  }

  fn intern_shape(&mut self, shape: Shape) -> u32 {
    if let Some(id) = self.shape_ids.get(&shape) {
      return *id;
    }
    let id = u32::try_from(self.shapes.len()).unwrap_or(u32::MAX);
    self.shapes.push(shape.clone());
    self.shape_ids.insert(shape, id);
    id
  }

  fn declare(&mut self, node: Node<'t>, name_node: Node<'t>, name: &'t str, traits: &Traits, leading: Option<u32>, fields: JvmFields) -> u32 {
    let name = self.intern(name);
    let start = row_line(node.start_position().row);
    let end_line = end_line_of(node);
    let has_signature = fields.signature.is_some();
    let declaration = Declaration {
      name,
      kind: traits.kind,
      container: self.container(),
      receiver: traits.receiver,
      reach: traits.reach,
      member: traits.member,
      signature: fields.signature,
      overrides: fields.overrides,
      overridable: fields.overridable,
      jvm_static: fields.jvm_static,
      jvm_field: fields.jvm_field,
      jvm_name: fields.jvm_name,
      lateinit: fields.lateinit,
      setter: fields.setter,
      supertypes: fields.supertypes,
      value_constructed: fields.value_type.as_ref().is_some_and(|it| it.constructed),
      value_type: fields.value_type.map(|it| it.path),
      constructors: fields.constructors,
      header: matches!(traits.kind, DeclKind::Function | DeclKind::Value | DeclKind::Constant).then(|| self.header(node, has_signature)),
      start_line: leading.map_or(start, |it| it.min(start)),
      end_line,
      name_line: line_of(name_node),
      in_error: self.error_depth > 0,
    };
    let id = u32::try_from(self.declarations.len()).unwrap_or(u32::MAX);
    self.declarations.push(declaration);
    if let Some(parent) = self.frames.len().checked_sub(2) {
      self.frames[parent].last_decl = Some(id);
    }
    id
  }

  /// The header of the function or value `node` (`Declaration::header`): the tokens of its node before its body or
  /// initializer, after those of a `Rule::Wrapper` around it before its first declarator (Java's `int a, b;`, the
  /// `const` of TypeScript), without what `Spec::header` drops; without the parameter list when `has_signature`
  fn header(&self, node: Node<'t>, has_signature: bool) -> String {
    let mut tokens: Vec<&'t str> = Vec::new();
    if let Some(parent) = node.parent().filter(|it| matches!(self.table.rule(it.kind_id()), Some(Rule::Wrapper))) {
      let mut cursor = parent.walk();
      let first = parent
        .named_children(&mut cursor)
        .find(|it| it.kind_id() == node.kind_id())
        .map_or_else(|| node.id(), |it| it.id());
      let mut cursor = parent.walk();
      for child in parent.children(&mut cursor).take_while(|it| it.id() != first) {
        if self.header_tokens(child, has_signature, false, &mut tokens).is_break() {
          break;
        }
      }
    }
    let _ = self.header_tokens(node, has_signature, false, &mut tokens);
    tokens.join(" ")
  }

  /// The header tokens of `node` and below, `Break` at the end of the header
  fn header_tokens(&self, node: Node<'t>, has_signature: bool, in_parameters: bool, tokens: &mut Vec<&'t str>) -> ControlFlow<()> {
    let header = &self.spec.header;
    let kind = node.kind();
    if node.is_named() {
      if header.ends.contains(&kind) {
        return ControlFlow::Break(());
      }
      if header.drops.contains(&kind) || matches!(self.table.rule(node.kind_id()), Some(Rule::Comment)) {
        return ControlFlow::Continue(());
      }
    } else if header.end_tokens.contains(&kind) && !in_parameters {
      return ControlFlow::Break(());
    }
    let parameters = header.parameters.contains(&kind);
    if parameters && has_signature {
      return ControlFlow::Continue(());
    }
    if node.child_count() == 0 {
      if (node.is_named() || !header.drop_tokens.contains(&kind))
        && let Some(text) = self.text(node).filter(|it| !it.is_empty())
      {
        tokens.push(text);
      }
      return ControlFlow::Continue(());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
      self.header_tokens(child, has_signature, in_parameters || parameters, tokens)?;
    }
    ControlFlow::Continue(())
  }

  fn reference(&mut self, name: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>, shape: Option<u32>) {
    if name.is_empty() || self.spec.reserved.contains(&name) {
      return;
    }
    let id = self.intern(name);
    if qualifier.is_none() && self.is_bound(id) {
      return;
    }
    let container = self.container();
    self.references.push(Reference {
      name: id,
      line,
      role,
      qualifier,
      container,
      shape,
      receiver_type: None,
      receiver_constructed: false,
      coroutine: self.spec.coroutine.and_then(|_| match self.coroutines.last() {
        Some(Context::Coroutine) => Some(true),
        Some(Context::Plain) | None => Some(false),
        Some(Context::Unknown) => None,
      }),
    });
  }

  /// The qualifier of a member of `object`: its path of names, or empty for an expression or a local
  pub(crate) fn qualifier_of(&mut self, object: Node<'t>) -> Vec<u32> {
    match self.path_of(object) {
      Some(path) => self.qualifier_ids(&path),
      None => Vec::new(),
    }
  }

  fn qualifier_ids(&mut self, path: &[&'t str]) -> Vec<u32> {
    let ids: Vec<u32> = path.iter().map(|it| self.intern(it)).collect();
    if ids.first().is_some_and(|it| self.is_bound(*it)) { Vec::new() } else { ids }
  }

  /// `a.b.c` as `[a, b, c]`: names, keywords and members of them, else `None`
  pub(crate) fn path_of(&self, object: Node<'t>) -> Option<Vec<&'t str>> {
    let mut reversed = Vec::new();
    let mut node = object;
    loop {
      match self.table.rule(node.kind_id()) {
        Some(Rule::Name(_)) => {
          reversed.push(self.text(node)?);
          break;
        }
        Some(Rule::Keyword(keyword)) => {
          reversed.push(keyword);
          break;
        }
        Some(Rule::Member { object, name, .. }) => {
          reversed.push(self.text(self.name_node(self.child(node, name)?)?)?);
          node = self.child(node, object)?;
        }
        Some(Rule::Forward(child)) => node = self.child(node, child)?,
        _ => return None,
      }
    }
    reversed.reverse();
    Some(reversed)
  }

  fn default_role(&self, node: Node<'t>) -> Role {
    match self.table.rule(node.kind_id()) {
      Some(Rule::Name(role)) => role,
      _ => Role::Value,
    }
  }

  fn push_scope(&mut self, body: bool) {
    self.scopes.push(Scope {
      bound_start: self.bound.len(),
      body,
    });
    if body {
      self.body_scopes += 1;
    }
    let spec = self.spec;
    if let (Some(hoist), Some(frame)) = (spec.hoist, self.frames.last()) {
      let node = frame.node;
      hoist(self, node);
    }
    if let Some(frame) = self.frames.last_mut() {
      frame.scopes += 1;
    }
  }

  fn pop_scope(&mut self) {
    let Some(scope) = self.scopes.pop() else { return };
    if scope.body {
      self.body_scopes -= 1;
    }
    for Binding { name, .. } in self.bound.drain(scope.bound_start..) {
      if let Some(count) = self.bound_counts.get_mut(&name) {
        *count -= 1;
        if *count == 0 {
          self.bound_counts.remove(&name);
        }
      }
    }
  }

  fn bind_id(&mut self, name: u32) {
    self.bind_typed(name, None);
  }

  fn bind_typed(&mut self, name: u32, type_path: Option<Typed>) {
    if self.scopes.is_empty() {
      return;
    }
    self.serials += 1;
    self.bound.push(Binding {
      name,
      type_path,
      initializers: None,
      serial: self.serials,
    });
    *self.bound_counts.entry(name).or_insert(0) += 1;
  }

  fn is_bound(&self, name: u32) -> bool {
    self.bound_counts.contains_key(&name)
  }

  /// Makes the declaration (`None` for a Rust `impl`) the container of what is below the current node
  pub(crate) fn enter_container(&mut self, decl: Option<u32>, holder: Holder, receiver: Option<u32>) {
    let supertypes = self.supertypes.len();
    let code = decl.and_then(|it| self.declarations.get(it as usize)).is_some_and(|it| it.kind == DeclKind::Function);
    self.containers.push(Container {
      decl,
      holder,
      receiver,
      code,
      supertypes,
    });
    if let Some(frame) = self.frames.last_mut() {
      frame.container = true;
    }
  }

  /// Makes the property `decl` (`None`: the container of the current node) the container of an accessor of it below
  /// the current node: code, as a function is
  pub(crate) fn enter_accessor(&mut self, decl: Option<u32>) {
    let decl = decl.or_else(|| self.container());
    self.enter_container(decl, Holder::Leaf, None);
    if let Some(container) = self.containers.last_mut() {
      container.code = true;
    }
  }

  /// The innermost declaration around the current node
  pub(crate) fn current_container(&self) -> Option<u32> {
    self.container()
  }

  fn container(&self) -> Option<u32> {
    self.containers.iter().rev().find_map(|it| it.decl)
  }

  /// Whether a lambda, an anonymous function, an object expression, an anonymous class or a local declaration lies
  /// between the current node and the declaration around it: each pushes onto `supertypes`, and the declaration's own
  /// entry (a type's supertypes, a function's empty list) is pushed before it is entered
  fn enclosed(&self) -> bool {
    let level = self.containers.iter().rev().find(|it| it.decl.is_some()).map_or(0, |it| it.supertypes);
    self.supertypes.len() > level
  }

  fn take_mark(&mut self, id: usize) -> Option<Mark> {
    let index = self.marks.iter().rposition(|it| it.0 == id)?;
    Some(self.marks.remove(index).1)
  }

  fn mark(&mut self, node: Node<'t>, mark: Mark) {
    let owner = self.frames.len() - 1;
    self.marks.push((node.id(), mark, owner));
  }

  fn is_name(&self, node: Node<'t>) -> bool {
    matches!(self.table.rule(node.kind_id()), Some(Rule::Name(_)))
  }

  /// The name a rule selected: the node itself when it is a name or an identifier-like leaf (a TypeScript
  /// `property_identifier`, which is no reference on its own), else its first named child that is a name, else the name
  /// of its only child (the `string_fragment` of `declare module "m"`)
  fn name_node(&self, node: Node<'t>) -> Option<Node<'t>> {
    let mut node = node;
    loop {
      if self.is_name(node) {
        return Some(node);
      }
      if node.named_child_count() == 0 {
        let identifier = self
          .text(node)
          .and_then(|it| it.chars().next())
          .is_some_and(|it| it.is_alphabetic() || it == '_' || it == '$' || it == '#');
        let keyword = matches!(self.table.rule(node.kind_id()), Some(Rule::Keyword(_)));
        return (identifier && !keyword && !node.is_error() && !node.is_missing()).then_some(node);
      }
      let mut cursor = node.walk();
      if let Some(name) = node.named_children(&mut cursor).find(|it| self.is_name(*it)) {
        return Some(name);
      }
      if node.named_child_count() != 1 {
        return None;
      }
      node = node.named_child(0)?;
    }
  }

  pub(crate) fn is_doc_comment(&self, node: Node<'t>) -> bool {
    self
      .text(node)
      .is_some_and(|it| (it.starts_with("/**") && !it.starts_with("/**/")) || (it.starts_with("///") && !it.starts_with("////")))
  }

  fn finish(mut self, summary: &mut FileSummary) {
    self.resolve_impl_members();
    self.publish_exported();
    self.add_implicit_constructors();
    self.receiver_types();
    let mut order: Vec<u32> = (0..u32::try_from(self.names.len()).unwrap_or(u32::MAX)).collect();
    order.sort_unstable_by_key(|it| self.names[*it as usize]);
    let mut remap = vec![0u32; order.len()];
    for (new, old) in order.iter().enumerate() {
      remap[*old as usize] = u32::try_from(new).unwrap_or(u32::MAX);
    }
    let map = |id: &mut u32| *id = remap[*id as usize];
    let map_type = |type_ref: &mut Option<TypeRef>| type_ref.iter_mut().flat_map(|it| &mut it.names).for_each(map);
    for import in &mut self.imports {
      import.path.iter_mut().for_each(map);
      if let ImportTarget::Name(name) = &mut import.target {
        map(name);
      }
      import.alias.iter_mut().for_each(map);
    }
    for declaration in &mut self.declarations {
      map(&mut declaration.name);
      declaration.receiver.iter_mut().for_each(map);
      let constructors = declaration.constructors.iter_mut().flatten().map(|it| &mut it.signature);
      for parameter in declaration.signature.iter_mut().chain(constructors).flat_map(|it| &mut it.parameters) {
        map(&mut parameter.name);
        map_type(&mut parameter.type_ref);
      }
      declaration.supertypes.iter_mut().flatten().for_each(map);
      declaration.value_type.iter_mut().flatten().for_each(map);
    }
    for shape in &mut self.shapes {
      match shape {
        Shape::Override { receiver, parameters, .. } => {
          receiver.iter_mut().for_each(map);
          parameters.iter_mut().for_each(map_type);
        }
        Shape::Call { arguments, .. } => arguments.iter_mut().filter_map(|(name, _)| name.as_mut()).for_each(map),
      }
    }
    for reference in &mut self.references {
      map(&mut reference.name);
      reference.qualifier.iter_mut().flatten().for_each(map);
      reference.receiver_type.iter_mut().flatten().for_each(map);
    }
    self.references.sort_unstable();
    self.references.dedup();
    summary.names = order.iter().map(|it| self.names[*it as usize].to_string()).collect();
    summary.package = self.package;
    summary.jvm_name = self.jvm_name;
    summary.imports = self.imports;
    summary.declarations = self.declarations;
    summary.references = self.references;
    summary.shapes = self.shapes;
    summary.error_bytes = self.error_bytes;
  }

  /// The types of the receivers of member references (`Reference::receiver_type`): a bound name's, unless a smart
  /// cast may change it; a property's or a field's of the class around the reference that declares it, the innermost
  /// first, not past a class with a supertype (whose `m` comes first), a nested class that isn't inner or a companion
  /// (whose code sees no `m` of the class around it) or an extension function or property of another receiver (whose
  /// members come first; `this` is that receiver), and for `this.m` only of the innermost class, with no extension
  /// between
  fn receiver_types(&mut self) {
    let this = self.name_ids.get("this").copied();
    // (class, name) -> the property or field of that name it declares
    let properties: HashMap<(u32, u32), &Declaration> = self
      .declarations
      .iter()
      .filter(|declaration| declaration.kind == DeclKind::Value)
      .filter_map(|declaration| Some(((declaration.container?, declaration.name), declaration)))
      .collect();
    let mut types = Vec::new();
    for receiver in &self.receivers {
      types.push(match receiver {
        TypedReceiver::Bound { reference, serial, type_path } => (*reference, (!self.cast_bindings.contains(serial)).then(|| type_path.clone())),
        TypedReceiver::Property { reference } => (*reference, self.property_type(&self.references[*reference], this, &properties)),
      });
    }
    for (index, type_path) in types {
      let reference = &mut self.references[index];
      reference.receiver_constructed = type_path.as_ref().is_some_and(|it| it.constructed);
      reference.receiver_type = type_path.map(|it| it.path);
    }
  }

  fn property_type(&self, reference: &Reference, this: Option<u32>, properties: &HashMap<(u32, u32), &Declaration>) -> Option<Typed> {
    let (own, name) = match reference.qualifier.as_deref()? {
      [name] => (false, *name),
      [first, name] if Some(*first) == this => (true, *name),
      _ => return None,
    };
    if self.cast_names.contains(&(reference.container, name)) {
      return None;
    }
    let at = |index: u32| self.declarations.get(index as usize).map(|declaration| (index, declaration));
    let around = std::iter::successors(reference.container.and_then(at), |(_, declaration)| declaration.container.and_then(at));
    // the receiver of an extension function or property around the use, whose `this` and members come first
    let mut extension = None;
    for (index, declaration) in around {
      if !matches!(declaration.kind, DeclKind::Type | DeclKind::EnumEntry) {
        if let Some(receiver) = declaration.receiver.filter(|_| extension.is_none()) {
          extension = Some(receiver);
        }
        continue;
      }
      if (own && extension.is_some()) || extension.is_some_and(|receiver| receiver != declaration.name) {
        return None;
      }
      if let Some(property) = properties.get(&(index, name)) {
        return property.value_type.clone().map(|path| Typed {
          path,
          constructed: property.value_constructed,
        });
      }
      // an inherited `m` comes first; the code of a nested class, a companion or an enum entry sees no `m` around it
      if own || declaration.kind == DeclKind::EnumEntry || !declaration.supertypes.is_empty() || (declaration.member == MemberKind::Static && declaration.container.is_some()) {
        return None;
      }
    }
    None
  }

  /// A member of a Rust `impl` belongs to the declaration of its type when this file has it
  fn resolve_impl_members(&mut self) {
    let types: HashMap<(u32, Option<u32>), u32> = self
      .declarations
      .iter()
      .enumerate()
      .filter(|(_, it)| it.kind == DeclKind::Type)
      .filter_map(|(index, it)| Some(((it.name, it.container), u32::try_from(index).ok()?)))
      .collect();
    for declaration in &mut self.declarations {
      if let (Some(receiver), true) = (declaration.receiver, self.spec.language == Language::Rust)
        && let Some(owner) = types.get(&(receiver, declaration.container))
      {
        declaration.container = Some(*owner);
      }
    }
  }

  fn publish_exported(&mut self) {
    for declaration in &mut self.declarations {
      if declaration.container.is_none() && declaration.reach == Reach::File && self.exported.contains(&declaration.name) {
        declaration.reach = Reach::Public;
      }
    }
  }

  /// A class that declares no constructor has the implicit `()`
  fn add_implicit_constructors(&mut self) {
    let Some(hooks) = &self.spec.jvm else { return };
    for declaration in &mut self.declarations {
      if let Some(constructors) = declaration.constructors.as_mut().filter(|it| it.is_empty()) {
        constructors.push(Constructor {
          signature: Signature { parameters: Vec::new() },
          reach: (hooks.implicit_constructor)(declaration.reach),
        });
      }
    }
  }
}

/// The API of the walk for the language code
impl<'t> Walk<'t> {
  pub(crate) fn text(&self, node: Node<'t>) -> Option<&'t str> {
    std::str::from_utf8(self.source.get(node.byte_range())?).ok()
  }

  pub(crate) fn intern(&mut self, name: &'t str) -> u32 {
    if let Some(id) = self.name_ids.get(name) {
      return *id;
    }
    let id = u32::try_from(self.names.len()).unwrap_or(u32::MAX);
    self.names.push(name);
    self.name_ids.insert(name, id);
    id
  }

  pub(crate) fn child(&self, node: Node<'t>, child: Child) -> Option<Node<'t>> {
    match child {
      Child::Itself => Some(node),
      Child::Field(name) => self.field(node, name),
      Child::Kind(kinds) => {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).find(|it| kinds.contains(&it.kind()))
      }
      Child::Nth(n) => node.named_child(u32::from(n)),
      Child::Last => node.named_child(node.named_child_count().checked_sub(1)?.try_into().ok()?),
    }
  }

  /// The child in this field; the field must be in the rules or `Spec::fields`, which a test checks against the grammar
  pub(crate) fn field(&self, node: Node<'t>, name: &str) -> Option<Node<'t>> {
    debug_assert!(
      self.spec.fields.contains(&name) || self.spec.rule_names().1.contains(&name),
      "{name} is not a field in the spec of {:?}",
      self.spec.language
    );
    node.child_by_field_id(self.table.field_id(name)?)
  }

  /// Whether the node is of this kind; the kind must be in the rules or `Spec::kinds`, which a test checks
  pub(crate) fn is(&self, node: Node<'t>, kind: &str) -> bool {
    debug_assert!(
      self.spec.kinds.contains(&kind) || self.spec.rules.iter().any(|it| it.0 == kind) || self.spec.rule_names().0.contains(&kind),
      "{kind} is not in the spec of {:?}",
      self.spec.language
    );
    node.kind() == kind
  }

  /// Whether the anonymous node is this token; the token must be in `Spec::tokens`, which a test checks
  pub(crate) fn is_token(&self, node: Node<'t>, token: &str) -> bool {
    debug_assert!(self.spec.tokens.contains(&token), "{token} is not a token in the spec of {:?}", self.spec.language);
    !node.is_named() && node.kind() == token
  }

  /// Whether the node has this token among its children (a keyword such as `static`)
  pub(crate) fn has_token(&self, node: Node<'t>, token: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|it| self.is_token(it, token))
  }

  /// The node of the frame `depth` levels above the current node (0 is the current node)
  pub(crate) fn ancestor(&self, depth: usize) -> Option<Node<'t>> {
    let index = self.frames.len().checked_sub(depth + 1)?;
    Some(self.frames[index].node)
  }

  pub(crate) fn holder(&self) -> Option<Holder> {
    self.containers.last().map(|it| it.holder)
  }

  pub(crate) fn has_holder(&self, holder: Holder) -> bool {
    self.containers.iter().any(|it| it.holder == holder)
  }

  /// The receiver of the innermost container (a Rust `impl`)
  pub(crate) fn container_receiver(&self) -> Option<u32> {
    self.containers.last().and_then(|it| it.receiver)
  }

  pub(crate) const fn is_local(&self) -> bool {
    self.body_scopes > 0
  }

  pub(crate) fn claim(&mut self, node: Node<'t>) {
    self.mark(node, Mark::Claimed);
  }

  pub(crate) fn give_role(&mut self, node: Node<'t>, role: Role) {
    self.mark(node, Mark::Role(role, None));
  }

  /// Gives `callee` the role `Call` with the shape of the arguments of `call` (`Spec::call_shape`), taken now: a Kotlin
  /// constructor invocation of a supertype, a Java `new`, before an object expression or an anonymous class pushes its
  /// supertypes
  pub(crate) fn give_call(&mut self, callee: Node<'t>, call: Node<'t>) {
    let shape = self.call_shape(call);
    self.mark(callee, Mark::Role(Role::Call, shape));
  }

  /// Whether the node is a comment, which a list of arguments may hold among them
  pub(crate) fn is_comment(&self, node: Node<'t>) -> bool {
    matches!(self.table.rule(node.kind_id()), Some(Rule::Comment))
  }

  pub(crate) fn mark_scope(&mut self, node: Node<'t>) {
    self.mark(node, Mark::Scope);
  }

  pub(crate) fn mark_plain(&mut self, node: Node<'t>) {
    self.mark(node, Mark::Plain);
  }

  /// Marks a pattern whose names become visible when it is left
  pub(crate) fn mark_pattern(&mut self, node: Node<'t>) {
    self.mark(node, Mark::Pattern(None));
  }

  pub(crate) fn open_scope(&mut self) {
    self.push_scope(true);
  }

  pub(crate) fn open_members_scope(&mut self) {
    self.push_scope(false);
  }

  pub(crate) fn bind_text(&mut self, name: &'t str) {
    let id = self.intern(name);
    self.bind_id(id);
  }

  pub(crate) fn bind_node(&mut self, node: Node<'t>) {
    self.claim(node);
    if let Some(text) = self.text(node) {
      let id = self.intern(text);
      let type_path = self.binding_types.remove(&node.id());
      self.bind_typed(id, type_path);
      if self.initializer_names.remove(&node.id())
        && let Some(binding) = self.bound.last_mut()
      {
        binding.initializers = Some(self.containers.len());
      }
    }
  }

  /// Marks the name `node` that the walk binds next as a parameter of a Kotlin primary constructor
  /// (`Binding::initializers`)
  pub(crate) fn initializer_binding(&mut self, node: Node<'t>) {
    self.initializer_names.insert(node.id());
  }

  /// Gives the name `node` that the walk binds next the type `declaration` was declared with (`JvmHooks::value_type`):
  /// a parameter, a typed lambda parameter
  pub(crate) fn type_binding(&mut self, node: Node<'t>, declaration: Node<'t>) {
    let Some(hooks) = &self.spec.jvm else { return };
    if let Some(type_path) = self.value_type(hooks, declaration) {
      self.binding_types.insert(node.id(), type_path);
    }
  }

  fn value_type(&mut self, hooks: &JvmHooks, declaration: Node<'t>) -> Option<Typed> {
    let (path, constructed) = (hooks.value_type)(self, declaration)?;
    Some(Typed {
      path: self.ids(&path),
      constructed,
    })
  }

  /// Whether `text` is a name bound around the current node: a local, a parameter, a type parameter
  pub(crate) fn is_bound_text(&self, text: &str) -> bool {
    self.name_ids.get(text).is_some_and(|id| self.is_bound(*id))
  }

  /// The operand of a Kotlin `is` or `as` or a Java `instanceof` or cast: a smart cast may give a name of it, a local
  /// or a property (`m`, `this.m`), another type in its scope or function, which the walk can't follow
  pub(crate) fn smart_cast(&mut self, operand: Node<'t>) {
    let Some(path) = self.path_of(operand) else { return };
    let name = match path.as_slice() {
      [name] | ["this", name] => *name,
      _ => return,
    };
    let Some(&id) = self.name_ids.get(name) else { return };
    if path.len() == 1
      && let Some(binding) = self.bound.iter().rev().find(|it| it.name == id)
    {
      self.cast_bindings.insert(binding.serial);
      return;
    }
    let container = self.container();
    self.cast_names.insert((container, id));
  }

  pub(crate) fn emit(&mut self, node: Node<'t>, role: Role, qualifier: Option<Vec<u32>>) {
    self.claim(node);
    if let Some(text) = self.text(node) {
      self.reference(text, line_of(node), role, qualifier, None);
    }
  }

  pub(crate) fn emit_text(&mut self, text: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>) {
    self.reference(text, line, role, qualifier, None);
  }

  /// A call of a constructor of the type `path` (its last name the type's, those before it its qualifier) on `line`,
  /// with the shape of the arguments of `call`: `this(…)` and `super(…)` of a constructor, which name no type to give
  /// the call to (`give_call`)
  pub(crate) fn emit_constructor_call(&mut self, path: &[&'t str], line: u32, call: Node<'t>) {
    let Some((name, qualifier)) = path.split_last() else { return };
    let shape = self.call_shape(call);
    let qualifier = (!qualifier.is_empty()).then(|| self.ids(qualifier));
    self.reference(name, line, Role::Call, qualifier, shape);
  }

  pub(crate) fn ids(&mut self, path: &[&'t str]) -> Vec<u32> {
    path.iter().map(|it| self.intern(it)).collect()
  }

  pub(crate) fn set_package(&mut self, package: String) {
    self.package = Some(package);
  }

  /// The reach of the setter of the property `decl` that an accessor of it narrows (`private set`)
  pub(crate) fn set_setter_reach(&mut self, decl: u32, reach: Reach) {
    if let Some(declaration) = self.declarations.get_mut(decl as usize)
      && declaration.setter.is_some()
    {
      declaration.setter = Some(reach);
    }
  }

  pub(crate) fn set_jvm_name(&mut self, name: String) {
    self.jvm_name = Some(name);
  }

  pub(crate) fn add_import(&mut self, import: Import) {
    self.imports.push(import);
  }

  /// `import a.b.C as D`, `use a::b::C as D`: the `Import` of `name` under `path`, and the `Import` reference to it
  pub(crate) fn import_name(&mut self, path: &[&'t str], name: &'t str, alias: Option<&'t str>, exported: bool, line: u32, name_line: u32) {
    let path_ids = self.ids(path);
    let target = self.intern(name);
    let alias = alias.map(|it| self.intern(it));
    self.imports.push(Import {
      path: path_ids.clone(),
      target: ImportTarget::Name(target),
      alias,
      exported,
      specifier: None,
      line,
    });
    self.reference(name, name_line, Role::Import, (!path.is_empty()).then_some(path_ids), None);
  }

  /// `import a.b.*`, `use a::b::*`
  pub(crate) fn import_all(&mut self, path: &[&'t str], exported: bool, line: u32) {
    let path = self.ids(path);
    self.imports.push(Import {
      path,
      target: ImportTarget::All,
      alias: None,
      exported,
      specifier: None,
      line,
    });
  }

  /// A `Binding` declaration of a name an import brings in; a local import only binds it in its scope
  pub(crate) fn add_binding(&mut self, node: Node<'t>, name_node: Node<'t>, name: &'t str, reach: Reach) {
    if self.is_local() {
      self.bind_text(name);
      return;
    }
    let traits = Traits {
      kind: DeclKind::Binding,
      reach,
      member: self.member_kind_at_top(),
      receiver: None,
      holder: Holder::Leaf,
    };
    self.declare(node, name_node, name, &traits, None, JvmFields::default());
  }

  fn member_kind_at_top(&self) -> MemberKind {
    if self.container().is_some() { MemberKind::Static } else { MemberKind::TopLevel }
  }

  /// Declares a member from language code with what `member_jvm_fields` found (a Kotlin `val` constructor parameter),
  /// or with `JvmFields::default()` (a Java record component, a TypeScript parameter property)
  pub(crate) fn add_declaration(&mut self, node: Node<'t>, name_node: Node<'t>, traits: &Traits, fields: JvmFields) -> Option<u32> {
    let name = self.text(name_node)?;
    self.claim(name_node);
    Some(self.declare(node, name_node, name, traits, None, fields))
  }

  /// Declares an item that language code found among the tokens of a macro (an item in the token tree of a Rust item
  /// macro): from the line of its first token to the end of its last, in `container`, else in the container of the
  /// current node
  pub(crate) fn add_token_declaration(&mut self, container: Option<u32>, item: TokenItem<'t>, traits: &Traits) -> Option<u32> {
    let name = self.text(item.name)?;
    self.claim(item.name);
    let id = self.declare(item.name, item.name, name, traits, Some(line_of(item.first)), JvmFields::default());
    let declaration = self.declarations.get_mut(id as usize)?;
    declaration.end_line = declaration.end_line.max(end_line_of(item.last));
    if container.is_some() {
      declaration.container = container;
    }
    if declaration.header.is_some() {
      declaration.header = Some(item.header);
    }
    Some(id)
  }

  /// The header (`Declaration::header`) of an item among the tokens of a macro: the header tokens of `nodes` in order,
  /// until one ends it (the `=` of an initializer)
  pub(crate) fn tokens_header(&self, nodes: &[Node<'t>]) -> String {
    let mut tokens: Vec<&'t str> = Vec::new();
    for node in nodes {
      if self.header_tokens(*node, false, false, &mut tokens).is_break() {
        break;
      }
    }
    tokens.join(" ")
  }

  /// Adds a constructor that the body of the class around the current node declares (a Kotlin secondary constructor, a
  /// Java constructor); a `canonical` one, the canonical constructor of a Java record declared again or compact,
  /// replaces the one of its header, the first. Any other is added, also beside one of the same types as the summary
  /// has them (`Stamp(java.util.Date)` and `Stamp(java.sql.Date)` of the same last names, `T` and `List<T>` of any
  /// type), which sync-core then can't tell apart. Nothing in a local class, nor in a type whose constructors are
  /// unknown or that has none (an enum)
  pub(crate) fn add_constructor(&mut self, constructor: Constructor, canonical: bool) {
    if self.is_local() {
      return;
    }
    let Some(constructors) = self
      .containers
      .last()
      .and_then(|it| it.decl)
      .and_then(|it| self.declarations.get_mut(it as usize))
      .and_then(|it| it.constructors.as_mut())
    else {
      return;
    };
    match constructors.first_mut().filter(|_| canonical) {
      Some(header) => *header = constructor,
      None => constructors.push(constructor),
    }
  }

  /// What a member that language code declares says about overriding (a Kotlin `override val` constructor parameter,
  /// local or not), and its `Override` references
  pub(crate) fn member_jvm_fields(&mut self, node: Node<'t>, name_node: Node<'t>, kind: DeclKind) -> JvmFields {
    let spec = self.spec;
    match &spec.jvm {
      Some(hooks) => self.overriding(hooks, node, Some(name_node), kind),
      None => JvmFields::default(),
    }
  }

  /// Declares a type the grammar misread as an expression, with the doc comments and annotations above it. Its
  /// supertypes are unknown, so its members override nothing
  pub(crate) fn declare_misread(&mut self, node: Node<'t>, name_node: Node<'t>, name: &'t str, traits: &Traits) -> u32 {
    let leading = self.leading_now;
    if self.spec.jvm.is_some() {
      self.push_supertypes(Vec::new());
    }
    self.declare(node, name_node, name, traits, leading, JvmFields::default())
  }

  /// Makes `supertypes` those whose members the declarations directly below the current node override, until the walk
  /// leaves it
  pub(crate) fn push_supertypes(&mut self, supertypes: Vec<Vec<u32>>) {
    self.supertypes.push(supertypes);
    if let Some(frame) = self.frames.last_mut() {
      frame.supertypes += 1;
    }
  }

  /// Marks a lambda as the misread body of the class of the current node: it keeps the supertypes of that class
  pub(crate) fn mark_class_body(&mut self, lambda: Node<'t>) {
    self.mark(lambda, Mark::ClassBody);
  }

  /// The paths of the supertypes that `types` name, without type arguments: `[a, Base]` of Kotlin `a.Base<T>` and of
  /// Java `a.Base<T>`; a node that names no type path (a comment, a function type) is skipped
  pub(crate) fn supertype_paths(&mut self, types: &[Node<'t>]) -> Vec<Vec<u32>> {
    let paths: Vec<Vec<&'t str>> = types.iter().filter_map(|it| self.type_path(*it)).collect();
    paths.iter().map(|it| self.ids(it)).collect()
  }

  /// `[a, b, C]` of a Kotlin user type `a.b.C<T>` (a `Rule::Path`), else the `path_of` the node (a Java type)
  pub(crate) fn type_path(&self, node: Node<'t>) -> Option<Vec<&'t str>> {
    if !matches!(self.table.rule(node.kind_id()), Some(Rule::Path)) {
      return self.path_of(node);
    }
    let mut cursor = node.walk();
    let path: Vec<&'t str> = node.named_children(&mut cursor).filter(|it| self.is_name(*it)).filter_map(|it| self.text(it)).collect();
    (!path.is_empty()).then_some(path)
  }

  /// The type of a parameter as the last name of each type in it, in source order (`TypeRef`), with the `arrays` of the
  /// parameter the language counted. `None` when a type parameter is in it, one of the function's (`type_parameters`) or
  /// one bound around it (of the class), or when it has no names: such a type matches any.
  pub(crate) fn type_ref(&mut self, node: Node<'t>, nullable: bool, arrays: u8, type_parameters: &[&'t str]) -> Option<TypeRef> {
    let primitives = self.spec.jvm.as_ref().map_or(&[][..], |it| it.primitives);
    let mut names = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
      let rule = self.table.rule(node.kind_id());
      match rule {
        Some(Rule::Path | Rule::Name(Role::Type) | Rule::Member { .. }) => {
          let path = self.type_path(node)?;
          let (first, last) = (path.first()?, path.last()?);
          if type_parameters.contains(first) || self.name_ids.get(first).is_some_and(|it| self.is_bound(*it)) {
            return None;
          }
          names.push(self.intern(last));
          // a Java path has no type arguments in it, a Kotlin one has them beside its names
          if !matches!(rule, Some(Rule::Path)) {
            continue;
          }
        }
        Some(Rule::Forward(_)) | None => {
          if node.named_child_count() == 0 {
            if primitives.iter().any(|kind| self.is(node, kind))
              && let Some(text) = self.text(node)
            {
              names.push(self.intern(text));
            }
            continue;
          }
        }
        // an annotation; a named parameter of a Kotlin function type, whose type is left out
        Some(_) => continue,
      }
      let mut cursor = node.walk();
      // in source order when popped, without the names of a path
      let start = stack.len();
      stack.extend(node.named_children(&mut cursor).filter(|it| !(matches!(rule, Some(Rule::Path)) && self.is_name(*it))));
      stack[start..].reverse();
    }
    (!names.is_empty()).then_some(TypeRef { names, nullable, arrays })
  }

  /// The names of the type parameters in `list` (`<T, R>`)
  pub(crate) fn type_parameter_names(&self, list: Option<Node<'t>>) -> Vec<&'t str> {
    let Some(list) = list else { return Vec::new() };
    let mut cursor = list.walk();
    list
      .named_children(&mut cursor)
      .filter_map(|it| self.child(it, Child::Kind(&["type_identifier"])))
      .filter_map(|it| self.text(it))
      .collect()
  }

  /// The supertypes of the body of an enum entry, the current node: the path of the enum around it, its containers and
  /// its name; none outside an enum
  pub(crate) fn enum_supertypes(&self) -> Vec<Vec<u32>> {
    if self.holder() != Some(Holder::Enum) {
      return Vec::new();
    }
    let mut path = Vec::new();
    let mut current = self.container();
    while let Some(declaration) = current.and_then(|it| self.declarations.get(it as usize)) {
      path.push(declaration.name);
      current = declaration.container;
    }
    path.reverse();
    vec![path]
  }

  /// The last declaration among the earlier siblings of the current node
  pub(crate) fn previous_declaration(&self) -> Option<u32> {
    let parent = self.frames.len().checked_sub(2)?;
    self.frames[parent].last_decl
  }

  pub(crate) fn extend_declaration(&mut self, id: u32, node: Node<'t>) {
    if let Some(declaration) = self.declarations.get_mut(id as usize) {
      declaration.end_line = declaration.end_line.max(end_line_of(node));
    }
  }

  pub(crate) fn declaration_kind(&self, id: u32) -> Option<DeclKind> {
    self.declarations.get(id as usize).map(|it| it.kind)
  }

  /// Makes the current node extend the declaration below it, like a doc comment (a Rust attribute)
  pub(crate) fn extend_leading(&mut self, node: Node<'t>) {
    let row = row_line(node.start_position().row);
    let start = self.leading_now.map_or(row, |it| it.min(row));
    if let Some(parent) = self.frames.len().checked_sub(2) {
      self.frames[parent].leading = Some(self.frames[parent].leading.map_or(start, |it| it.min(start)));
    }
  }

  /// Gives the leading line of the current node to its first child declaration
  pub(crate) fn pass_leading(&mut self, node: Node<'t>) {
    let start = self.leading_now.unwrap_or_else(|| row_line(node.start_position().row));
    if let Some(frame) = self.frames.last_mut() {
      frame.leading = Some(start);
    }
  }

  pub(crate) fn start_tokens(&mut self) {
    if let Some(frame) = self.frames.last_mut() {
      frame.tokens = Some(Tokens::default());
    }
  }

  pub(crate) fn export_name(&mut self, name: &'t str) {
    let id = self.intern(name);
    self.exported.push(id);
  }
}

pub(crate) fn line_of(node: Node<'_>) -> u32 {
  row_line(node.start_position().row)
}

/// The last line of a node; a node that ends with its line break ends on the line before
fn end_line_of(node: Node<'_>) -> u32 {
  let end = node.end_position();
  if end.column == 0 && end.row > node.start_position().row {
    row_line(end.row - 1)
  } else {
    row_line(end.row)
  }
}

fn row_line(row: usize) -> u32 {
  u32::try_from(row + 1).unwrap_or(u32::MAX)
}
