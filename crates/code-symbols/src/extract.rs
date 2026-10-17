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
//! - **Bounded**: files over `MAX_BYTES`, `MAX_LINES` or `MAX_AVERAGE_LINE` (minified) are not parsed, and the parse
//!   stops after `PARSE_BUDGET` progress callbacks, a count of parser operations, so the same bytes always give the same
//!   summary, unlike a wall-clock timeout. Either gives a `truncated` summary.

use crate::language::Language;
use crate::summary::{DeclKind, Declaration, FileSummary, Import, ImportTarget, MemberKind, Reach, Reference, Role};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::OnceLock;
use tree_sitter::{Node, ParseOptions, ParseState, Parser, Tree};

/// The version of the extraction rules; bump it on any rule change, so that cached summaries are made again
pub const RULES_VERSION: u32 = 1;
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
    names: Vec::new(),
    imports: Vec::new(),
    declarations: Vec::new(),
    references: Vec::new(),
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
  thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
  }
  PARSER.with_borrow_mut(|parser| {
    parser.set_language(&table.language).ok()?;
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
}

pub(crate) const SCOPE: Rule = Rule::Bind(Binder {
  scope: true,
  names: &[],
  late: false,
  implicit: &[],
  labels: &[],
});

pub(crate) const fn binder(scope: bool, names: &'static [Child]) -> Binder {
  Binder {
    scope,
    names,
    late: false,
    implicit: &[],
    labels: &[],
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
  /// The role a call, an annotation or a JSX tag gives a name or a member
  Role(Role),
  /// The root of a binding pattern; its names become visible when the frame at this index is left (`None`: the root)
  Pattern(Option<usize>),
  /// Leaves a pattern: a pattern's type, a default value, a member's object
  Expr,
  /// A body whose declarations are locals
  Scope,
  /// Walked without its rule: a Kotlin lambda that is the misread body of an object
  Plain,
}

struct Frame<'t> {
  node: Node<'t>,
  /// Names below bind, and become visible when the frame at this index is left
  pattern: Option<usize>,
  scopes: u8,
  container: bool,
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
}

struct Scope {
  bound_start: usize,
  body: bool,
}

/// The state of one walk; the language code reads and writes it through the methods below
pub(crate) struct Walk<'t> {
  spec: &'static Spec,
  table: &'static Table,
  source: &'t [u8],
  name_ids: HashMap<&'t str, u32>,
  names: Vec<&'t str>,
  package: Option<String>,
  imports: Vec<Import>,
  declarations: Vec<Declaration>,
  references: Vec<Reference>,
  frames: Vec<Frame<'t>>,
  /// (node id, mark, index of the frame that set it)
  marks: Vec<(usize, Mark, usize)>,
  scopes: Vec<Scope>,
  bound: Vec<u32>,
  bound_counts: HashMap<u32, u32>,
  body_scopes: u32,
  /// (frame index, name): bindings that become visible when that frame is left
  pending: Vec<(usize, u32)>,
  containers: Vec<Container>,
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
      imports: Vec::new(),
      declarations: Vec::new(),
      references: Vec::new(),
      frames: Vec::new(),
      marks: Vec::new(),
      scopes: Vec::new(),
      bound: Vec::new(),
      bound_counts: HashMap::new(),
      body_scopes: 0,
      pending: Vec::new(),
      containers: Vec::new(),
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
      error: false,
      leading: None,
      last_decl: None,
      tokens: None,
    };
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
          self.mark(callee, Mark::Role(role));
        }
        true
      }
      Some(Rule::Forward(child)) => {
        if let (Some(Mark::Role(role)), Some(child)) = (mark, self.child(node, child)) {
          self.mark(child, Mark::Role(role));
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
    while let Some(&(at, name)) = self.pending.last() {
      if at < index {
        break;
      }
      self.pending.pop();
      self.bind_id(name);
    }
    if frame.container {
      self.containers.pop();
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
      || (self.frames[frame].pattern.is_some() && self.table.is_binding(node.kind_id()) && (self.spec.binds)(text) && !matches!(mark, Some(Mark::Role(_))));
    if binds {
      let at = self.frames[frame].pattern.unwrap_or(frame);
      let id = self.intern(text);
      self.pending.push((at, id));
      return;
    }
    let role = match mark {
      Some(Mark::Role(role)) => role,
      _ => role,
    };
    let qualifier = self.token_qualifier(text);
    let before = self.references.len();
    self.reference(text, line_of(node), role, qualifier);
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
    let role = match mark {
      Some(Mark::Role(role)) => role,
      _ => role.unwrap_or_else(|| self.default_role(name_node)),
    };
    let qualifier = object.map(|object| self.qualifier_of(object));
    self.reference(text, line_of(name_node), role, qualifier);
  }

  fn path(&mut self, node: Node<'t>, mark: Option<Mark>) {
    let role = match mark {
      Some(Mark::Role(role)) => role,
      _ => Role::Type,
    };
    let mut segments: Vec<&'t str> = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
      if !matches!(self.table.rule(child.kind_id()), Some(Rule::Name(_))) {
        continue;
      }
      self.claim(child);
      let Some(text) = self.text(child) else { continue };
      let qualifier = if segments.is_empty() { None } else { Some(self.qualifier_ids(&segments)) };
      if !matches!(mark, Some(Mark::Claimed)) {
        self.reference(text, line_of(child), role, qualifier);
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
    if self.body_scopes > 0 {
      // a local declaration binds its name in the scope around it
      match rule.pattern.and_then(|it| self.child(node, it)) {
        Some(pattern) => self.mark(pattern, Mark::Pattern(rule.late.then_some(index))),
        None => {
          if let Some(name) = self.child(node, rule.name).and_then(|it| self.name_node(it)) {
            self.claim(name);
            if let Some(text) = self.text(name) {
              let id = self.intern(text);
              if rule.late { self.pending.push((index, id)) } else { self.bind_id(id) }
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
      let mut first = rule.default_name.filter(|_| names.is_empty()).map(|name| self.declare(node, node, name, &traits, leading));
      for name in names {
        if let Some(text) = self.text(name) {
          let id = self.declare(node, name, text, &traits, leading);
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

  fn declare(&mut self, node: Node<'t>, name_node: Node<'t>, name: &'t str, traits: &Traits, leading: Option<u32>) -> u32 {
    let name = self.intern(name);
    let start = row_line(node.start_position().row);
    let end_line = end_line_of(node);
    let declaration = Declaration {
      name,
      kind: traits.kind,
      container: self.container(),
      receiver: traits.receiver,
      reach: traits.reach,
      member: traits.member,
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

  fn reference(&mut self, name: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>) {
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
    for name in self.bound.drain(scope.bound_start..) {
      if let Some(count) = self.bound_counts.get_mut(&name) {
        *count -= 1;
        if *count == 0 {
          self.bound_counts.remove(&name);
        }
      }
    }
  }

  fn bind_id(&mut self, name: u32) {
    if self.scopes.is_empty() {
      return;
    }
    self.bound.push(name);
    *self.bound_counts.entry(name).or_insert(0) += 1;
  }

  fn is_bound(&self, name: u32) -> bool {
    self.bound_counts.contains_key(&name)
  }

  /// Makes the declaration (`None` for a Rust `impl`) the container of what is below the current node
  pub(crate) fn enter_container(&mut self, decl: Option<u32>, holder: Holder, receiver: Option<u32>) {
    self.containers.push(Container { decl, holder, receiver });
    if let Some(frame) = self.frames.last_mut() {
      frame.container = true;
    }
  }

  fn container(&self) -> Option<u32> {
    self.containers.iter().rev().find_map(|it| it.decl)
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

  fn is_doc_comment(&self, node: Node<'t>) -> bool {
    self
      .text(node)
      .is_some_and(|it| (it.starts_with("/**") && !it.starts_with("/**/")) || (it.starts_with("///") && !it.starts_with("////")))
  }

  fn finish(mut self, summary: &mut FileSummary) {
    self.resolve_impl_members();
    self.publish_exported();
    let mut order: Vec<u32> = (0..u32::try_from(self.names.len()).unwrap_or(u32::MAX)).collect();
    order.sort_unstable_by_key(|it| self.names[*it as usize]);
    let mut remap = vec![0u32; order.len()];
    for (new, old) in order.iter().enumerate() {
      remap[*old as usize] = u32::try_from(new).unwrap_or(u32::MAX);
    }
    let map = |id: &mut u32| *id = remap[*id as usize];
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
    }
    for reference in &mut self.references {
      map(&mut reference.name);
      reference.qualifier.iter_mut().flatten().for_each(map);
    }
    self.references.sort_unstable();
    self.references.dedup();
    summary.names = order.iter().map(|it| self.names[*it as usize].to_string()).collect();
    summary.package = self.package;
    summary.imports = self.imports;
    summary.declarations = self.declarations;
    summary.references = self.references;
    summary.error_bytes = self.error_bytes;
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
    self.mark(node, Mark::Role(role));
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
      self.bind_text(text);
    }
  }

  pub(crate) fn emit(&mut self, node: Node<'t>, role: Role, qualifier: Option<Vec<u32>>) {
    self.claim(node);
    if let Some(text) = self.text(node) {
      self.reference(text, line_of(node), role, qualifier);
    }
  }

  pub(crate) fn emit_text(&mut self, text: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>) {
    self.reference(text, line, role, qualifier);
  }

  pub(crate) fn ids(&mut self, path: &[&'t str]) -> Vec<u32> {
    path.iter().map(|it| self.intern(it)).collect()
  }

  pub(crate) fn set_package(&mut self, package: String) {
    self.package = Some(package);
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
    self.reference(name, name_line, Role::Import, (!path.is_empty()).then_some(path_ids));
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
    self.declare(node, name_node, name, &traits, None);
  }

  fn member_kind_at_top(&self) -> MemberKind {
    if self.container().is_some() { MemberKind::Static } else { MemberKind::TopLevel }
  }

  /// Declares a member from language code (a Kotlin `val` constructor parameter, a TypeScript parameter property)
  pub(crate) fn add_declaration(&mut self, node: Node<'t>, name_node: Node<'t>, traits: &Traits) -> Option<u32> {
    let name = self.text(name_node)?;
    self.claim(name_node);
    Some(self.declare(node, name_node, name, traits, None))
  }

  /// Declares a type the grammar misread as an expression, with the doc comments and annotations above it
  pub(crate) fn declare_misread(&mut self, node: Node<'t>, name_node: Node<'t>, name: &'t str, traits: &Traits) -> u32 {
    let leading = self.leading_now;
    self.declare(node, name_node, name, traits, leading)
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
