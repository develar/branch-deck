//! The summary of a Rust file: one preorder walk over rust-analyzer's tree (`rust_syntax`), with an explicit frame
//! stack (rowan's preorder is an iterator, so the walk takes no stack of its own; the parse and the drop of the tree
//! take `rust_syntax::guarded`'s), on the parser-neutral `summary_builder`. The rules are those Rust had on tree-sitter,
//! read from rowan's kinds:
//!
//! - **`use` binds**: every name a `use` brings in (`pub use` and `pub(crate) use` included) is a `Binding`
//!   declaration of the file besides its `Import`, `{self}` the module before it, never `_`, `self`, `crate` or
//!   `super`; a `pub use`, `pub(crate) use` or `pub(super) use` passes the names on (`Import::exported`) and is no import
//!   statement (`FileSummary::import_lines`). A `use` in a function only binds its names there.
//! - **`impl` blocks declare nothing themselves**: their members carry the last name of the impl's type as `receiver`
//!   (none for a primitive, a reference or a tuple), and `summary_builder::finish` gives them to the declaration of
//!   that type when this file has it.
//! - **Locals**: a function, a closure, a block, an `if`, a `while`, a `for` and a match arm open a scope whose names
//!   (parameters, `let`, `if let`, `while let`, loop and arm patterns, type and const parameters, the items declared
//!   in it) are no references while it lasts; a declaration inside one is a local, not a declaration of the file. A
//!   `let`'s names are seen after it (`let x = x;`), a parameter's, a pattern's of a `for` or an arm right after the
//!   pattern. A capitalized name in a pattern is a variant or a constant (`None`, `MAX`), a reference.
//! - **Macros**: the identifiers among the tokens of a macro are `Value` references, `a::b::c` and `a.b` qualified by
//!   the names before them (`?.c` after a group, `x.f(…).c`), a `!` after one makes it a `Macro` reference; keywords,
//!   lifetimes, literals, primitive types and metavariables (`$x`) are none, nor are the patterns of `macro_rules!`.
//!   The body of a macro in item position (in a file, a module, an impl, a trait or an extern block, never in a
//!   function) declares each `pub` item at its top level by its tokens (`vocabulary! { pub enum E { A } }`,
//!   `lazy_static! { pub static ref X: T = … }`), with the variants of an enum (decision 17).
//! - **Attributes**: an attribute's path is a `Macro` reference unless it is built in, the names in `derive(…)` are
//!   `Type` references; the other arguments are configuration, not names. `#[test]`, `#[tokio::test]` and `#[rstest]`
//!   make the function a test. A declaration starts at its first doc comment or attribute, which rust-analyzer keeps
//!   in its node.
//! - **Fields**: a field of a struct expression or a struct pattern, a shorthand one too (`Stills { encoding }`), is a
//!   `Value` reference qualified by the struct.
//! - **The header** of a function, a field, a constant or a static (`Declaration::header`) is the tokens of its node
//!   before its body or its `=`, one space apart, without its attributes, doc comments, comments, visibility, the `;`
//!   that ends it and the `,` that ends a `where`; a lifetime is one token (`'a`).
//! - **Errors**: `error_bytes` counts the bytes under the outermost `ERROR` nodes and one per error of the parser that
//!   has no bytes (an expected token); a declaration is `in_error` when one lies between the start of its node, after
//!   its attributes, and its name.

use super::{CONTEXTUAL_KEYWORDS, Lines, PRIMITIVES, is_callee, is_outer_doc, leading_start, span};
use crate::summary::{DeclKind, Declaration, FileSummary, LineSpan, MemberKind, Reach, Role};
use crate::summary_builder::{Bindings, SummaryBuilder, blank_declaration, end_line};
use rust_syntax::ra_ap_syntax::{AstNode, NodeOrToken, SyntaxElement, SyntaxError, SyntaxKind as K, SyntaxNode, SyntaxToken, WalkEvent, ast};
use std::collections::HashSet;

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

/// The keyword of an item among the tokens of a macro: what it declares, and what it is to its members
const ITEM_KEYWORDS: &[(&str, DeclKind, Holder)] = &[
  ("enum", DeclKind::Type, Holder::Enum),
  ("struct", DeclKind::Type, Holder::Class),
  ("union", DeclKind::Type, Holder::Class),
  ("trait", DeclKind::Type, Holder::Trait),
  ("type", DeclKind::TypeAlias, Holder::Leaf),
  ("fn", DeclKind::Function, Holder::Leaf),
  ("const", DeclKind::Constant, Holder::Leaf),
  ("static", DeclKind::Value, Holder::Leaf),
  ("mod", DeclKind::Module, Holder::Module),
];

/// The tokens between the keywords of an item: `unsafe fn`, `async fn`, `extern "C" fn`
const ITEM_MODIFIERS: &[&str] = &["unsafe", "async", "extern", "mut"];

/// The punctuation rust-analyzer lexes a character at a time inside a token tree, glued back where its characters touch
const COMPOSITE_PUNCTUATION: &[&str] = &[
  "<<=", ">>=", "...", "..=", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "..", "+=", "-=", "*=", "/=", "%=", "^=", "&=", "|=", "<<", ">>",
];

/// Fills `summary` (`empty_summary` of the file) by one walk over rust-analyzer's tree of `source`; whether the file was
/// read: not when it is nested deeper than any code (`rust_syntax::guarded`)
pub(crate) fn summarize(source: &[u8], summary: &mut FileSummary) -> bool {
  rust_syntax::guarded(source, || {
    let text = rust_syntax::utf8_text(source);
    let (root, errors) = rust_syntax::parse_with_errors(source);
    let mut walk = Walk::new(source, &text, &errors);
    walk.run(&root);
    walk.finish(summary);
  })
  .is_some()
}

/// What a container is to the declarations in it, for their member kind and reach
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
  Class,
  Enum,
  Trait,
  Impl,
  TraitImpl,
  Module,
  /// A function, a field, a constant: nothing in it is a member
  Leaf,
}

/// The scope a declaration opens for its children
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opens {
  Nothing,
  /// Type parameters; the members are still declarations
  Members,
  /// A function: everything inside is local
  Body,
}

/// What a declaration is to the summary and to its members
#[derive(Debug, Clone, Copy)]
struct Traits {
  kind: DeclKind,
  reach: Reach,
  member: MemberKind,
  receiver: Option<u32>,
  holder: Holder,
}

struct Container {
  /// `None` for an `impl`, which declares nothing itself
  decl: Option<u32>,
  holder: Holder,
  receiver: Option<u32>,
}

struct Frame<'t> {
  /// The frame whose leaving binds the names of the pattern this node is in
  pattern: Option<usize>,
  scopes: u8,
  container: bool,
  error: bool,
  /// Inside a token tree: the path of the `a::b::` (or `a.b.`) tokens before the next one
  tokens: Option<Tokens<'t>>,
}

#[derive(Default)]
struct Tokens<'t> {
  path: Vec<&'t str>,
  after_colons: bool,
  /// A group (`(…)`, `[…]`, `{…}`) came last: a name after `.` or `::` is a member of an expression
  after_group: bool,
  /// The reference of the last name, which a `!` makes a macro
  last: Option<usize>,
  /// The end of a `$`, whose name after it is a metavariable
  dollar: Option<usize>,
}

/// An item among the tokens of a macro: where it starts (its first doc comment or attribute), its name and its last
/// token or group
struct TokenItem<'t> {
  first: usize,
  name: SyntaxToken,
  name_text: &'t str,
  last: usize,
  header: Option<String>,
}

/// Where a declaration is: the bytes of its first line and its last, of its name, and where an error makes it `in_error`
struct Placed {
  start: usize,
  end: usize,
  name: usize,
  checked_from: usize,
}

struct Walk<'t> {
  source: &'t [u8],
  text: &'t str,
  lines: Lines,
  out: SummaryBuilder<'t>,
  frames: Vec<Frame<'t>>,
  bindings: Bindings,
  containers: Vec<Container>,
  error_depth: u32,
  /// The byte ranges of the errors: the outermost `ERROR` nodes and the errors of no bytes, at their offset
  errors: Vec<(usize, usize)>,
  /// (declaration, from, to): the declaration is `in_error` when an error lies in `from..=to`
  checks: Vec<(u32, usize, usize)>,
  /// The tokens a macro's item claimed as its name, by offset: no references
  claimed: HashSet<usize>,
  /// The walk reads an attribute of a declaration that is a container
  in_attribute_of_container: bool,
}

impl<'t> Walk<'t> {
  fn new(source: &'t [u8], text: &'t str, errors: &[SyntaxError]) -> Self {
    let mut out = SummaryBuilder::new();
    let points: Vec<(usize, usize)> = errors
      .iter()
      .filter(|it| it.range().is_empty())
      .map(|it| {
        let at = usize::from(it.range().start());
        (at, at)
      })
      .collect();
    out.error_bytes = u32::try_from(points.len()).unwrap_or(u32::MAX);
    Self {
      source,
      text,
      lines: Lines::new(source),
      out,
      frames: Vec::new(),
      bindings: Bindings::default(),
      containers: Vec::new(),
      error_depth: 0,
      errors: points,
      checks: Vec::new(),
      claimed: HashSet::new(),
      in_attribute_of_container: false,
    }
  }

  fn run(&mut self, root: &SyntaxNode) {
    let mut preorder = root.preorder_with_tokens();
    while let Some(event) = preorder.next() {
      match event {
        WalkEvent::Enter(NodeOrToken::Node(node)) => {
          if !self.enter(&node) {
            preorder.skip_subtree();
          }
        }
        WalkEvent::Enter(NodeOrToken::Token(token)) => self.token(&token),
        WalkEvent::Leave(NodeOrToken::Node(_)) => self.leave(),
        WalkEvent::Leave(NodeOrToken::Token(_)) => {}
      }
    }
  }

  fn finish(mut self, summary: &mut FileSummary) {
    self.errors.sort_unstable();
    // the furthest end of the errors up to each one, so that an error over `from..=to` is one binary search
    let mut furthest = Vec::with_capacity(self.errors.len());
    let mut end = 0;
    for (_, error_end) in &self.errors {
      end = end.max(*error_end);
      furthest.push(end);
    }
    for (declaration, from, to) in std::mem::take(&mut self.checks) {
      let before = self.errors.partition_point(|it| it.0 <= to);
      if before > 0 && furthest[before - 1] >= from {
        self.out.declarations[declaration as usize].in_error = true;
      }
    }
    self.out.finish(summary, None);
  }

  fn slice(&self, range: rust_syntax::ra_ap_syntax::TextRange) -> &'t str {
    self.text.get(span(range)).unwrap_or_default()
  }

  fn line(&self, offset: usize) -> u32 {
    self.lines.line(offset)
  }

  fn end_line(&self, start: usize, end: usize) -> u32 {
    end_line(|offset| self.lines.line(offset), self.source, start, end)
  }

  fn enter(&mut self, node: &SyntaxNode) -> bool {
    let index = self.frames.len();
    let kind = node.kind();
    let mut frame = Frame {
      pattern: self.frames.last().and_then(|it| it.pattern),
      scopes: 0,
      container: false,
      error: false,
      tokens: None,
    };
    if kind == K::ERROR {
      if self.error_depth == 0 {
        let range = span(node.text_range());
        self.out.error_bytes = self.out.error_bytes.saturating_add(u32::try_from(range.len()).unwrap_or(u32::MAX));
        self.errors.push((range.start, range.end));
      }
      self.error_depth += 1;
      frame.error = true;
    }
    // the pattern of a `let` binds when the `let` is left, the one of a parameter, a `for` or an arm when it is
    if ast::Pat::can_cast(kind)
      && let Some(parent) = node.parent()
    {
      match parent.kind() {
        K::LET_STMT | K::LET_EXPR => frame.pattern = Some(index - 1),
        K::PARAM | K::MATCH_ARM | K::FOR_EXPR => frame.pattern = Some(index),
        _ => {}
      }
    }
    self.frames.push(frame);
    match kind {
      K::ATTR => {
        self.in_attribute_of_container = index.checked_sub(1).is_some_and(|it| self.frames[it].container);
        self.attribute(node);
        self.in_attribute_of_container = false;
        false
      }
      K::DOC_COMMENT | K::VISIBILITY => false,
      K::USE | K::EXTERN_CRATE => {
        // the walk does not descend: the attributes of the item are read here
        for attribute in node.children().filter(|it| it.kind() == K::ATTR) {
          self.attribute(&attribute);
        }
        if kind == K::USE {
          self.use_item(node);
        } else {
          self.extern_crate(node);
        }
        false
      }
      K::MODULE | K::STRUCT | K::UNION | K::ENUM | K::TRAIT | K::TYPE_ALIAS | K::FN | K::CONST | K::STATIC | K::MACRO_RULES | K::MACRO_DEF | K::VARIANT | K::RECORD_FIELD => {
        self.declaration(node);
        true
      }
      K::IMPL => {
        self.impl_item(node);
        true
      }
      K::BLOCK_EXPR | K::CLOSURE_EXPR | K::IF_EXPR | K::WHILE_EXPR | K::MATCH_ARM | K::FOR_EXPR => {
        self.push_scope(true);
        true
      }
      K::TYPE_PARAM | K::CONST_PARAM => {
        if let Some(name) = name_token(node) {
          let id = self.out.intern(self.slice(name.text_range()));
          self.bindings.bind(id, None);
        }
        true
      }
      K::NAME => {
        self.name(node);
        false
      }
      K::NAME_REF => {
        self.name_ref(node);
        false
      }
      K::RECORD_EXPR_FIELD | K::RECORD_PAT_FIELD => {
        self.field(node);
        true
      }
      K::TOKEN_TREE => self.token_tree(node),
      _ => true,
    }
  }

  fn leave(&mut self) {
    let Some(frame) = self.frames.pop() else { return };
    let index = self.frames.len();
    self.bindings.leave_frame(index, frame.scopes);
    if frame.container {
      self.containers.pop();
    }
    if frame.error {
      self.error_depth -= 1;
    }
  }

  fn push_scope(&mut self, body: bool) {
    self.bindings.push(body);
    if let Some(frame) = self.frames.last_mut() {
      frame.scopes += 1;
    }
  }

  fn enter_container(&mut self, decl: Option<u32>, holder: Holder, receiver: Option<u32>) {
    self.containers.push(Container { decl, holder, receiver });
    if let Some(frame) = self.frames.last_mut() {
      frame.container = true;
    }
  }

  /// The innermost declaration around the node; for an attribute, the one around the item it is of, which rowan keeps
  /// in the item's node
  fn container(&self) -> Option<u32> {
    let skip = usize::from(self.in_attribute_of_container);
    self.containers.iter().rev().skip(skip).find_map(|it| it.decl)
  }

  fn holder(&self) -> Option<Holder> {
    self.containers.last().map(|it| it.holder)
  }

  fn container_receiver(&self) -> Option<u32> {
    self.containers.last().and_then(|it| it.receiver)
  }

  /// A reference of `name` on `line`; none for `Self`, nor for an unqualified name a scope binds. Its index
  fn reference(&mut self, name: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>) -> Option<usize> {
    if name.is_empty() || name == "Self" {
      return None;
    }
    let id = self.out.intern(name);
    if qualifier.is_none() && self.bindings.is_bound(id) {
      return None;
    }
    let container = self.container();
    Some(self.out.add_reference(id, line, role, qualifier, container, None, None))
  }

  /// The ids of a qualifier; empty when its first name is bound (a local's member)
  fn qualifier_ids(&mut self, path: &[&'t str]) -> Vec<u32> {
    let ids = self.out.ids(path);
    self.bindings.qualifier(ids)
  }

  /// The qualifier of a member of `object`: its path of names, or empty for an expression or a local
  fn qualifier_of(&mut self, object: Option<SyntaxNode>) -> Vec<u32> {
    match object.and_then(|it| self.path_of(&it)) {
      Some(path) => self.qualifier_ids(&path),
      None => Vec::new(),
    }
  }

  /// `a::b::c` as `[a, b, c]`: names and `self`, `super`, `crate`, `Self`; `None` for a segment of anything else
  /// (`<T as Trait>::`)
  fn path_segments(&self, path: &SyntaxNode) -> Option<Vec<&'t str>> {
    let mut reversed = Vec::new();
    let mut current = Some(path.clone());
    while let Some(path) = current {
      let segment = path.children().find(|it| it.kind() == K::PATH_SEGMENT)?;
      let name = segment.children().find(|it| it.kind() == K::NAME_REF)?.first_token()?;
      if !matches!(name.kind(), K::IDENT | K::SELF_KW | K::SUPER_KW | K::CRATE_KW | K::SELF_TYPE_KW) {
        return None;
      }
      reversed.push(self.slice(name.text_range()));
      current = path.children().find(|it| it.kind() == K::PATH);
    }
    reversed.reverse();
    Some(reversed)
  }

  /// `a.b.c` and `a::b` as `[a, b, c]`: a path, a field of one; else `None`
  fn path_of(&self, node: &SyntaxNode) -> Option<Vec<&'t str>> {
    match node.kind() {
      K::PATH => self.path_segments(node),
      K::PATH_EXPR | K::PATH_TYPE => self.path_segments(&node.children().find(|it| it.kind() == K::PATH)?),
      K::FIELD_EXPR => {
        let field = ast::FieldExpr::cast(node.clone())?;
        let mut path = self.path_of(field.expr()?.syntax())?;
        let name = field.name_ref()?.ident_token()?;
        path.push(self.slice(name.text_range()));
        Some(path)
      }
      _ => None,
    }
  }

  fn name(&mut self, node: &SyntaxNode) {
    if node.parent().is_none_or(|it| it.kind() != K::IDENT_PAT) {
      return;
    }
    let Some(token) = node.first_token().filter(|it| it.kind() == K::IDENT) else { return };
    let text = self.slice(token.text_range());
    let frame = self.frames.len() - 1;
    if !binds(text) {
      let line = self.line(usize::from(token.text_range().start()));
      self.reference(text, line, Role::Value, None);
      return;
    }
    let at = self.frames[frame].pattern.unwrap_or(frame);
    let id = self.out.intern(text);
    self.bindings.bind_later(at, id, None);
  }

  fn name_ref(&mut self, node: &SyntaxNode) {
    let Some(token) = node.first_token().filter(|it| it.kind() == K::IDENT) else { return };
    let Some(parent) = node.parent() else { return };
    let text = self.slice(token.text_range());
    let line = self.line(usize::from(token.text_range().start()));
    match parent.kind() {
      K::PATH_SEGMENT => self.path_reference(&parent, text, line),
      K::FIELD_EXPR => {
        let receiver = ast::FieldExpr::cast(parent.clone()).and_then(|it| it.expr()).map(|it| it.syntax().clone());
        let qualifier = self.qualifier_of(receiver);
        let role = if is_callee(&parent) { Role::Call } else { Role::Value };
        self.reference(text, line, role, Some(qualifier));
      }
      K::METHOD_CALL_EXPR => {
        let receiver = ast::MethodCallExpr::cast(parent).and_then(|it| it.receiver()).map(|it| it.syntax().clone());
        let qualifier = self.qualifier_of(receiver);
        self.reference(text, line, Role::Call, Some(qualifier));
      }
      // the field of a struct expression or pattern, which `field` reads
      K::RECORD_EXPR_FIELD | K::RECORD_PAT_FIELD => {}
      K::ASSOC_TYPE_ARG => {
        self.reference(text, line, Role::Type, None);
      }
      _ => {
        self.reference(text, line, Role::Value, None);
      }
    }
  }

  /// A segment of a path: qualified by the segments before it, the last one in the role of the path (a type, a callee,
  /// a macro, a struct), the others values
  fn path_reference(&mut self, segment: &SyntaxNode, text: &'t str, line: u32) {
    let Some(path) = segment.parent() else { return };
    let qualifier = path.children().find(|it| it.kind() == K::PATH).map(|it| match self.path_segments(&it) {
      Some(segments) => self.qualifier_ids(&segments),
      None => Vec::new(),
    });
    let role = match path.parent() {
      Some(outer) if outer.kind() != K::PATH => match outer.kind() {
        K::PATH_TYPE => {
          if qualifier.is_none() && PRIMITIVES.contains(&text) {
            return;
          }
          Role::Type
        }
        K::PATH_EXPR if is_callee(&outer) => Role::Call,
        K::MACRO_CALL => Role::Macro,
        K::RECORD_EXPR | K::RECORD_PAT => Role::Type,
        _ => Role::Value,
      },
      // a name with type arguments before a `::` is a type: `Vec` of `Vec::<u8>::new()`
      _ if qualifier.is_none() && segment.children().any(|it| it.kind() == K::GENERIC_ARG_LIST) => Role::Type,
      _ => Role::Value,
    };
    self.reference(text, line, role, qualifier);
  }

  /// `S { field: value }`, `S { field }` and `let S { field: pattern } = …`: the field is a member of the struct, never a
  /// local; a shorthand names a local too, which the walk of its name reads
  fn field(&mut self, node: &SyntaxNode) {
    let name = node.children().find(|it| it.kind() == K::NAME_REF).or_else(|| {
      // a shorthand: the path of one name, or the binding of one
      let inner = node.children().next()?;
      match inner.kind() {
        K::PATH_EXPR => {
          let path = inner.children().find(|it| it.kind() == K::PATH)?;
          if path.children().any(|it| it.kind() == K::PATH) {
            return None;
          }
          path.children().find(|it| it.kind() == K::PATH_SEGMENT)?.children().find(|it| it.kind() == K::NAME_REF)
        }
        K::IDENT_PAT => inner.children().find(|it| it.kind() == K::NAME),
        _ => None,
      }
    });
    let Some(token) = name.and_then(|it| it.first_token()).filter(|it| it.kind() == K::IDENT) else {
      return;
    };
    let structure = node
      .parent()
      .and_then(|list| list.parent())
      .and_then(|it| it.children().find(|child| child.kind() == K::PATH));
    let qualifier = self.qualifier_of(structure);
    let text = self.slice(token.text_range());
    let line = self.line(usize::from(token.text_range().start()));
    self.reference(text, line, Role::Value, Some(qualifier));
  }

  fn declaration(&mut self, node: &SyntaxNode) {
    let (kind, opens) = match node.kind() {
      K::MODULE => (DeclKind::Module, Opens::Nothing),
      K::STRUCT | K::UNION | K::ENUM | K::TRAIT => (DeclKind::Type, Opens::Members),
      K::TYPE_ALIAS => (DeclKind::TypeAlias, Opens::Members),
      K::FN => (DeclKind::Function, Opens::Body),
      K::CONST => (DeclKind::Constant, Opens::Nothing),
      K::MACRO_RULES | K::MACRO_DEF => (DeclKind::Macro, Opens::Nothing),
      K::VARIANT => (DeclKind::EnumEntry, Opens::Nothing),
      _ => (DeclKind::Value, Opens::Nothing),
    };
    let name = name_token(node);
    if self.bindings.is_local() {
      // a local declaration binds its name in the scope around it
      if let Some(name) = &name {
        let id = self.out.intern(self.slice(name.text_range()));
        self.bindings.bind(id, None);
      }
    } else if let Some(name) = name {
      let traits = self.traits(node, kind);
      let text = self.slice(name.text_range());
      let range = span(node.text_range());
      let start = leading_start(node);
      let name_start = usize::from(name.text_range().start());
      let mut declaration = blank_declaration(self.out.intern(text), traits.kind, traits.reach, traits.member, self.container());
      declaration.receiver = traits.receiver;
      declaration.test = kind == DeclKind::Function && is_test(node);
      declaration.header = matches!(kind, DeclKind::Function | DeclKind::Value | DeclKind::Constant).then(|| self.header(node));
      let at = Placed {
        start,
        end: range.end,
        name: name_start,
        checked_from: rust_syntax::start_after_attributes(node),
      };
      let id = self.add_declaration(declaration, &at);
      self.enter_container(Some(id), traits.holder, traits.receiver);
    }
    if opens != Opens::Nothing {
      self.push_scope(opens == Opens::Body);
    }
  }

  fn traits(&self, node: &SyntaxNode, kind: DeclKind) -> Traits {
    let holder = self.holder();
    let own = match node.kind() {
      K::STRUCT | K::UNION | K::VARIANT => Holder::Class,
      K::ENUM => Holder::Enum,
      K::TRAIT => Holder::Trait,
      K::MODULE => Holder::Module,
      _ => Holder::Leaf,
    };
    let has_self = ast::Fn::cast(node.clone()).and_then(|it| it.param_list()).and_then(|it| it.self_param()).is_some();
    let member = match holder {
      None | Some(Holder::Module) => MemberKind::TopLevel,
      Some(Holder::Class) => MemberKind::Instance,
      Some(Holder::Impl | Holder::TraitImpl | Holder::Trait) if has_self => MemberKind::Instance,
      Some(_) => MemberKind::Static,
    };
    // a variant, a trait item and an item of a trait impl are as visible as their trait or type
    let in_variant = node.kind() == K::RECORD_FIELD && node.parent().and_then(|it| it.parent()).is_some_and(|it| it.kind() == K::VARIANT);
    let reach = match holder {
      Some(Holder::Enum | Holder::Trait | Holder::TraitImpl) => Reach::Public,
      Some(Holder::Class) if in_variant => Reach::Public,
      _ => self.reach(node),
    };
    let receiver = if matches!(holder, Some(Holder::Impl | Holder::TraitImpl)) {
      self.container_receiver()
    } else {
      None
    };
    Traits {
      kind,
      reach,
      member,
      receiver,
      holder: own,
    }
  }

  fn reach(&self, node: &SyntaxNode) -> Reach {
    match node.children().find(|it| it.kind() == K::VISIBILITY) {
      Some(visibility) => self.visibility_reach(self.slice(visibility.text_range())),
      None => self.private_reach(),
    }
  }

  /// The reach of `pub`, `pub(crate)`, `pub(super)`, `pub(self)` and `pub(in …)`, or of the `(…)` after a `pub` token
  fn visibility_reach(&self, text: &str) -> Reach {
    if text == "pub" {
      Reach::Public
    } else if text.contains("super") {
      Reach::Parent
    } else if text.contains("self") {
      self.private_reach()
    } else {
      Reach::Module
    }
  }

  fn private_reach(&self) -> Reach {
    if self.containers.iter().any(|it| it.holder == Holder::Module) {
      Reach::Container
    } else {
      Reach::File
    }
  }

  /// The header of the function, field, constant or static `node` (`Declaration::header`)
  fn header(&self, node: &SyntaxNode) -> String {
    let mut tokens: Vec<&'t str> = Vec::new();
    for element in node.children_with_tokens() {
      match element {
        NodeOrToken::Token(token) => match token.kind() {
          K::EQ => break,
          K::SEMICOLON | K::WHITESPACE | K::COMMENT => {}
          _ => tokens.push(self.slice(token.text_range())),
        },
        NodeOrToken::Node(child) => match child.kind() {
          K::ATTR | K::DOC_COMMENT | K::VISIBILITY => {}
          K::BLOCK_EXPR => break,
          _ => self.header_tokens(&child, &mut tokens),
        },
      }
    }
    tokens.join(" ")
  }

  /// The tokens of `node` and below in a header: no attributes, comments, nor the `,` that ends a `where`
  fn header_tokens(&self, node: &SyntaxNode, tokens: &mut Vec<&'t str>) {
    let mut preorder = node.preorder_with_tokens();
    while let Some(event) = preorder.next() {
      match event {
        WalkEvent::Enter(NodeOrToken::Node(child)) if matches!(child.kind(), K::ATTR | K::DOC_COMMENT) => preorder.skip_subtree(),
        WalkEvent::Enter(NodeOrToken::Token(token)) if !is_trivia(token.kind()) => {
          let ends_where = token.kind() == K::COMMA && token.parent().is_some_and(|it| it.kind() == K::WHERE_CLAUSE) && next_significant(&token).is_none();
          if !ends_where {
            tokens.push(self.slice(token.text_range()));
          }
        }
        _ => {}
      }
    }
  }

  /// `#[…]`: its path is a macro unless built in, `derive(…)` names traits; an inner attribute (`#![…]`) none
  fn attribute(&mut self, node: &SyntaxNode) {
    let Some(attribute) = ast::Attr::cast(node.clone()) else { return };
    if attribute.excl_token().is_some() {
      return;
    }
    let Some(meta) = attribute.meta().map(|it| it.syntax().clone()) else { return };
    if !matches!(meta.kind(), K::TOKEN_TREE_META | K::PATH_META | K::KEY_VALUE_META) {
      return;
    }
    let Some(path) = meta.children().find(|it| it.kind() == K::PATH) else { return };
    let Some(segments) = self.path_segments(&path) else { return };
    let Some((name, before)) = segments.split_last() else { return };
    if *name == "derive" && before.is_empty() {
      if let Some(arguments) = meta.children().find(|it| it.kind() == K::TOKEN_TREE) {
        self.derives(&arguments);
      }
    } else if !before.is_empty() || !BUILTIN_ATTRIBUTES.contains(name) {
      let qualifier = (!before.is_empty()).then(|| self.out.ids(before));
      let line = self.line(usize::from(path.text_range().start()));
      self.reference(name, line, Role::Macro, qualifier);
    }
  }

  /// The traits of `derive(Debug, serde::Serialize)`
  fn derives(&mut self, arguments: &SyntaxNode) {
    let mut path: Vec<&'t str> = Vec::new();
    for token in arguments.children_with_tokens().filter_map(NodeOrToken::into_token) {
      match token.kind() {
        K::IDENT => {
          let text = self.slice(token.text_range());
          let qualifier = (!path.is_empty()).then(|| self.out.ids(&path));
          let line = self.line(usize::from(token.text_range().start()));
          self.reference(text, line, Role::Type, qualifier);
          path.push(text);
        }
        K::COLON | K::WHITESPACE | K::COMMENT => {}
        _ => path.clear(),
      }
    }
  }

  /// An `impl` declares nothing; its members name its type as their receiver
  fn impl_item(&mut self, node: &SyntaxNode) {
    let item = ast::Impl::cast(node.clone());
    let receiver = item
      .as_ref()
      .and_then(ast::Impl::self_ty)
      .and_then(|it| self.type_name(it.syntax()))
      .map(|it| self.out.intern(it));
    let holder = if item.and_then(|it| it.trait_()).is_some() { Holder::TraitImpl } else { Holder::Impl };
    if !self.bindings.is_local() {
      self.enter_container(None, holder, receiver);
    }
    self.push_scope(false);
  }

  /// The last name of an impl's type: `Foo` of `a::Foo<T>`; none for a primitive, a reference, a tuple
  fn type_name(&self, node: &SyntaxNode) -> Option<&'t str> {
    if node.kind() != K::PATH_TYPE {
      return None;
    }
    let path = node.children().find(|it| it.kind() == K::PATH)?;
    let name = path
      .children()
      .find(|it| it.kind() == K::PATH_SEGMENT)?
      .children()
      .find(|it| it.kind() == K::NAME_REF)?
      .first_token()
      .filter(|it| it.kind() == K::IDENT)?;
    let text = self.slice(name.text_range());
    let qualified = path.children().any(|it| it.kind() == K::PATH);
    (qualified || !PRIMITIVES.contains(&text)).then_some(text)
  }

  /// `use a::{b, c::d as e, f::*}`, walked with an explicit stack of (tree, path before it)
  fn use_item(&mut self, node: &SyntaxNode) {
    let reach = self.reach(node);
    // a `pub use` passes the names on: no import statement alone
    if !is_exported(reach) {
      let start = rust_syntax::start_after_attributes(node);
      let end = usize::from(node.text_range().end());
      let span = LineSpan {
        start: self.line(start),
        end: self.end_line(start, end),
      };
      self.out.import_lines.push(span);
    }
    let Some(tree) = ast::Use::cast(node.clone()).and_then(|it| it.use_tree()) else {
      return;
    };
    let mut stack: Vec<(ast::UseTree, Vec<&'t str>)> = vec![(tree, Vec::new())];
    while let Some((tree, prefix)) = stack.pop() {
      let mut path = prefix;
      let last = tree.path().and_then(|own| {
        if let Some(segments) = self.path_segments(own.syntax()) {
          path.extend(segments);
        }
        own.segment().map(|it| it.syntax().text_range())
      });
      if let Some(list) = tree.use_tree_list() {
        let start = stack.len();
        stack.extend(list.use_trees().map(|it| (it, path.clone())));
        stack[start..].reverse();
      } else if tree.star_token().is_some() {
        let line = self.line(usize::from(tree.syntax().text_range().start()));
        self.out.add_import_all(&path, is_exported(reach), line);
      } else if let Some(last) = last {
        let alias = tree.rename().map(|it| it.syntax().clone());
        self.use_name(node, path, last, alias.as_ref(), reach);
      }
    }
  }

  fn extern_crate(&mut self, node: &SyntaxNode) {
    let Some(item) = ast::ExternCrate::cast(node.clone()) else { return };
    let Some(name) = item.name_ref().and_then(|it| it.syntax().first_token()) else { return };
    let reach = self.reach(node);
    let alias = item.rename().map(|it| it.syntax().clone());
    let text = self.slice(name.text_range());
    self.use_name(node, vec![text], name.text_range(), alias.as_ref(), reach);
  }

  /// One name a `use` or `extern crate` brings in, its last segment at `last`: its import, its reference and its
  /// binding
  fn use_name(&mut self, node: &SyntaxNode, mut segments: Vec<&'t str>, last: rust_syntax::ra_ap_syntax::TextRange, alias: Option<&SyntaxNode>, reach: Reach) {
    // `use a::b::{self}` brings in `b`
    if segments.len() > 1 && segments.last() == Some(&"self") {
      segments.pop();
    }
    let Some((name, before)) = segments.split_last() else { return };
    let line = self.line(usize::from(last.start()));
    // the name or the `_` of `as …`
    let alias_token = alias.and_then(|it| {
      it.descendants_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .find(|token| matches!(token.kind(), K::IDENT | K::UNDERSCORE))
    });
    let alias_text = alias_token.as_ref().map(|it| self.slice(it.text_range()));
    self.out.add_import_name(before, name, alias_text, is_exported(reach), line);
    let path_ids = self.out.ids(before);
    self.reference(name, line, Role::Import, (!before.is_empty()).then_some(path_ids));
    let bound = alias_text.unwrap_or(name);
    if bound == "_" || ["self", "crate", "super"].contains(&bound) {
      return;
    }
    if self.bindings.is_local() {
      let id = self.out.intern(bound);
      self.bindings.bind(id, None);
      return;
    }
    let name_start = alias_token.map_or_else(|| usize::from(last.start()), |it| usize::from(it.text_range().start()));
    let start = rust_syntax::start_after_attributes(node);
    let member = if self.container().is_some() { MemberKind::Static } else { MemberKind::TopLevel };
    let declaration = blank_declaration(self.out.intern(bound), DeclKind::Binding, reach, member, self.container());
    let at = Placed {
      start,
      end: usize::from(node.text_range().end()),
      name: name_start,
      checked_from: start,
    };
    self.add_declaration(declaration, &at);
  }

  /// Adds a declaration at its place: its lines and the column of its name, `in_error` inside an `ERROR` node (or
  /// after the walk, by an error between `checked_from` and its name)
  fn add_declaration(&mut self, mut declaration: Declaration, at: &Placed) -> u32 {
    declaration.start_line = self.line(at.start);
    declaration.end_line = self.end_line(at.start, at.end);
    declaration.name_line = self.line(at.name);
    declaration.name_column = psi_builder::document::column(self.source, at.name);
    declaration.in_error = self.error_depth > 0;
    let id = self.out.add_declaration(declaration);
    self.checks.push((id, at.checked_from, at.name));
    id
  }

  /// A token tree: its tokens are read by `token`; the pattern of a `macro_rules!` rule and the arguments of a
  /// `macro` are skipped, and the body of a macro in item position declares its `pub` items
  fn token_tree(&mut self, node: &SyntaxNode) -> bool {
    if is_rule_pattern(node) {
      return false;
    }
    let parent = node.parent();
    let grandparent = parent.as_ref().and_then(SyntaxNode::parent);
    // a group ends the path before it: a name after it is a member of an expression
    let index = self.frames.len() - 1;
    if let Some(around) = index.checked_sub(1).and_then(|it| self.frames[it].tokens.as_mut()) {
      around.group();
    }
    self.frames[index].tokens = Some(Tokens::default());
    let in_item_position = parent.as_ref().is_some_and(|it| it.kind() == K::MACRO_CALL)
      && grandparent
        .as_ref()
        .is_some_and(|it| matches!(it.kind(), K::SOURCE_FILE | K::ITEM_LIST | K::ASSOC_ITEM_LIST | K::EXTERN_ITEM_LIST));
    if in_item_position && !self.bindings.is_local() {
      self.macro_items(node);
    }
    true
  }

  /// A token of the tree being walked; only the tokens of a token tree say anything here
  fn token(&mut self, token: &SyntaxToken) {
    let Some(mut tokens) = self.frames.last_mut().and_then(|it| it.tokens.take()) else {
      return;
    };
    self.tree_token(&mut tokens, token);
    if let Some(frame) = self.frames.last_mut() {
      frame.tokens = Some(tokens);
    }
  }

  fn tree_token(&mut self, tokens: &mut Tokens<'t>, token: &SyntaxToken) {
    let kind = token.kind();
    if is_trivia(kind) {
      return;
    }
    let range = span(token.text_range());
    if tokens.dollar.take() == Some(range.start) && (kind == K::IDENT || kind.is_keyword(rust_syntax::EDITION)) {
      return;
    }
    let text = self.slice(token.text_range());
    match kind {
      K::DOLLAR => tokens.dollar = Some(range.end),
      K::IDENT if self.claimed.contains(&range.start) || PRIMITIVES.contains(&text) => {}
      // a macro in a pattern (`id!(Some(a))`): its lowercase names bind as the pattern's
      K::IDENT
        if binds(text)
          && let Some(at) = self.frames.last().and_then(|it| it.pattern) =>
      {
        let id = self.out.intern(text);
        self.bindings.bind_later(at, id, None);
        tokens.reset();
      }
      K::IDENT if !CONTEXTUAL_KEYWORDS.contains(&text) => {
        // after a group, the names of a chain are members of an expression (`f(…).a.b`)
        let qualifier = if !tokens.after_colons {
          tokens.path.clear();
          tokens.after_group = false;
          None
        } else if tokens.after_group {
          Some(Vec::new())
        } else {
          (!tokens.path.is_empty()).then(|| tokens.path.clone())
        };
        if !tokens.after_group {
          tokens.path.push(text);
        }
        tokens.after_colons = false;
        let qualifier = qualifier.map(|it| self.qualifier_ids(&it));
        let line = self.line(range.start);
        tokens.last = self.reference(text, line, Role::Value, qualifier);
      }
      K::SELF_KW | K::SUPER_KW | K::CRATE_KW | K::SELF_TYPE_KW => {
        if !tokens.after_colons {
          tokens.path.clear();
        }
        tokens.path.push(text);
        tokens.after_colons = false;
        tokens.after_group = false;
        tokens.last = None;
      }
      // `mut` leaves the path as it is; a literal is an expression, whose members are of no path (`"a".len()`)
      K::MUT_KW => {}
      K::TRUE_KW | K::FALSE_KW => tokens.group(),
      _ if kind.is_literal() => tokens.group(),
      K::COLON if touches_next(token, K::COLON) => {}
      K::COLON if touches_previous(token, K::COLON) => {
        tokens.after_colons = true;
        tokens.last = None;
      }
      K::DOT if !touches_next(token, K::DOT) && !touches_previous(token, K::DOT) => {
        tokens.after_colons = true;
        tokens.last = None;
      }
      K::BANG if !touches_next(token, K::EQ) => {
        if let Some(last) = tokens.last {
          self.out.references[last].role = Role::Macro;
        }
        tokens.reset();
      }
      _ => tokens.reset(),
    }
  }

  /// The `pub` items at the top level of the token tree of a macro, each from its doc comments and attributes on; a
  /// token that starts no item starts the search again
  fn macro_items(&mut self, tree: &SyntaxNode) {
    let children = token_children(tree);
    let mut leading: Option<usize> = None;
    let mut index = 0;
    while let Some(child) = children.get(index) {
      index += 1;
      if is_comment(child.kind()) {
        if self.is_doc(child) {
          leading.get_or_insert(index - 1);
        }
        continue;
      }
      if child.kind() == K::POUND && children.get(index).is_some_and(|it| it.kind() == K::TOKEN_TREE) {
        leading.get_or_insert(index - 1);
        index += 1;
        continue;
      }
      if child.kind() == K::PUB_KW
        && let Some(end) = self.macro_item(&children, index - 1, leading)
      {
        index = end;
      }
      leading = None;
    }
  }

  fn is_doc(&self, element: &SyntaxElement) -> bool {
    is_outer_doc(self.slice(element.text_range()))
  }

  /// Whether the element is a token of this text that is a word: a keyword or an identifier, not a literal
  fn is_word(&self, element: &SyntaxElement, word: &str) -> bool {
    element.as_token().is_some_and(|it| !it.kind().is_literal() && self.slice(it.text_range()) == word)
  }

  /// The item that starts with the `pub` at `children[start]`: `pub(crate)`, the keywords and modifiers, the name, up to
  /// its `;` or its `{…}` body (a value's initializer may hold braces); the index after it
  fn macro_item(&mut self, children: &[SyntaxElement], start: usize, leading: Option<usize>) -> Option<usize> {
    let mut index = start + 1;
    let reach = match children.get(index).filter(|it| delimited_by(it, K::L_PAREN)) {
      Some(visibility) => {
        index += 1;
        self.visibility_reach(self.slice(visibility.text_range()))
      }
      None => Reach::Public,
    };
    let header_start = index;
    let mut item: Option<(DeclKind, Holder)> = None;
    let name = loop {
      let child = children.get(index)?;
      index += 1;
      if let Some((_, kind, holder)) = ITEM_KEYWORDS.iter().find(|(keyword, ..)| self.is_word(child, keyword)) {
        item = Some((*kind, *holder));
        continue;
      }
      // `static ref` of `lazy_static!`
      let by_reference = self.is_word(child, "ref") && item.is_some_and(|(kind, _)| kind == DeclKind::Value);
      if !by_reference && let Some(token) = child.as_token().filter(|it| it.kind() == K::IDENT) {
        break token.clone();
      }
      let modifier = ITEM_MODIFIERS.iter().any(|it| self.is_word(child, it)) || child.kind().is_literal();
      if !by_reference && !modifier {
        return None;
      }
    };
    let (kind, holder) = item?;
    let braced = !matches!(kind, DeclKind::Value | DeclKind::Constant | DeclKind::TypeAlias);
    let mut last = index - 1;
    let mut body = None;
    while let Some(child) = children.get(index) {
      if child.kind() == K::PUB_KW {
        break;
      }
      index += 1;
      last = index - 1;
      if child.kind() == K::SEMICOLON {
        break;
      }
      if braced && delimited_by(child, K::L_CURLY) {
        body = child.as_node().cloned();
        break;
      }
    }
    let header_end = if body.is_some() { index - 1 } else { index };
    let parameters = children[header_start..header_end].iter().find(|it| delimited_by(it, K::L_PAREN));
    let has_self = parameters
      .and_then(SyntaxElement::as_node)
      .is_some_and(|it| it.children_with_tokens().any(|child| child.kind() == K::SELF_KW));
    let (member, receiver) = match self.holder() {
      None | Some(Holder::Module) => (MemberKind::TopLevel, None),
      Some(container) => (
        if kind == DeclKind::Function && has_self {
          MemberKind::Instance
        } else {
          MemberKind::Static
        },
        if matches!(container, Holder::Impl | Holder::TraitImpl) {
          self.container_receiver()
        } else {
          None
        },
      ),
    };
    let traits = Traits {
      kind,
      reach,
      member,
      receiver,
      holder,
    };
    let header = matches!(kind, DeclKind::Function | DeclKind::Value | DeclKind::Constant).then(|| self.tokens_header(&children[header_start..header_end]));
    let first = usize::from(children[leading.unwrap_or(start)].text_range().start());
    let name_text = self.slice(name.text_range());
    let item = TokenItem {
      first,
      name,
      name_text,
      last: usize::from(children[last].text_range().end()),
      header,
    };
    let container = self.container();
    let declared = self.add_token_declaration(container, item, &traits);
    if let (Some(body), Holder::Enum) = (body, holder) {
      self.enum_variants(declared, &body);
    }
    Some(index)
  }

  /// The variants of an enum among the tokens of a macro: the name at the start of the body and after each `,`, with the
  /// doc comments and attributes above it
  fn enum_variants(&mut self, declared: u32, body: &SyntaxNode) {
    let mut leading: Option<usize> = None;
    let mut expects = true;
    // (first, name, last end)
    let mut variant: Option<(usize, SyntaxToken, usize)> = None;
    let children = token_children(body);
    let mut index = 0;
    while let Some(child) = children.get(index) {
      index += 1;
      let range = span(child.text_range());
      if is_comment(child.kind()) {
        if expects && self.is_doc(child) {
          leading.get_or_insert(range.start);
        }
        continue;
      }
      if child.kind() == K::COMMA {
        if let Some(found) = variant.take() {
          self.add_variant(declared, found);
        }
        (expects, leading) = (true, None);
      } else if expects && child.kind() == K::POUND && children.get(index).is_some_and(|it| it.kind() == K::TOKEN_TREE) {
        leading.get_or_insert(range.start);
        index += 1;
      } else if let Some(token) = child.as_token().filter(|it| expects && it.kind() == K::IDENT) {
        variant = Some((leading.unwrap_or(range.start), token.clone(), range.end));
        expects = false;
      } else if let Some((_, _, last)) = variant.as_mut() {
        *last = range.end;
      } else {
        expects = false;
      }
    }
    if let Some(found) = variant {
      self.add_variant(declared, found);
    }
  }

  fn add_variant(&mut self, declared: u32, (first, name, last): (usize, SyntaxToken, usize)) {
    let traits = Traits {
      kind: DeclKind::EnumEntry,
      reach: Reach::Public,
      member: MemberKind::Static,
      receiver: None,
      holder: Holder::Class,
    };
    let name_text = self.slice(name.text_range());
    let item = TokenItem {
      first,
      name,
      name_text,
      last,
      header: None,
    };
    self.add_token_declaration(Some(declared), item, &traits);
  }

  /// Declares an item found among the tokens of a macro: from the line of its first token to the end of its last, in
  /// `container`
  fn add_token_declaration(&mut self, container: Option<u32>, item: TokenItem<'t>, traits: &Traits) -> u32 {
    let name_range = span(item.name.text_range());
    self.claimed.insert(name_range.start);
    let mut declaration = blank_declaration(self.out.intern(item.name_text), traits.kind, traits.reach, traits.member, container);
    declaration.receiver = traits.receiver;
    declaration.header = item.header;
    declaration.start_line = self.line(item.first).min(self.line(name_range.start));
    declaration.end_line = self.end_line(name_range.start, name_range.end).max(self.end_line(item.first, item.last));
    declaration.name_line = self.line(name_range.start);
    declaration.name_column = psi_builder::document::column(self.source, name_range.start);
    declaration.in_error = self.error_depth > 0;
    let id = self.out.add_declaration(declaration);
    self.checks.push((id, item.first, name_range.start));
    id
  }

  /// The header (`Declaration::header`) of an item among the tokens of a macro: the tokens of `elements` in order,
  /// the punctuation whose characters touch glued (`->`, `::`), until an `=` ends it, without the `;` that ends the item
  fn tokens_header(&self, elements: &[SyntaxElement]) -> String {
    let mut leaves: Vec<SyntaxToken> = Vec::new();
    for element in elements {
      match element {
        NodeOrToken::Token(token) => leaves.push(token.clone()),
        NodeOrToken::Node(node) => leaves.extend(node.descendants_with_tokens().filter_map(NodeOrToken::into_token)),
      }
    }
    leaves.retain(|it| !is_trivia(it.kind()));
    let mut tokens: Vec<&'t str> = Vec::new();
    let mut index = 0;
    while index < leaves.len() {
      let (text, next) = self.glued(&leaves, index);
      if text == "=" {
        break;
      }
      tokens.push(text);
      index = next;
    }
    if tokens.last() == Some(&";") {
      tokens.pop();
    }
    tokens.join(" ")
  }

  /// The text of the punctuation that starts at `leaves[index]`, glued to the characters that touch it as rustc lexes
  /// them, and the index after it
  fn glued(&self, leaves: &[SyntaxToken], index: usize) -> (&'t str, usize) {
    let first = &leaves[index];
    let start = usize::from(first.text_range().start());
    if first.kind().is_punct() {
      for composite in COMPOSITE_PUNCTUATION {
        let count = composite.len();
        let Some(run) = leaves.get(index..index + count) else { continue };
        let touching = run
          .windows(2)
          .all(|pair| pair[0].text_range().end() == pair[1].text_range().start() && pair[1].kind().is_punct());
        let end = usize::from(run[count - 1].text_range().end());
        if touching && self.text.get(start..end) == Some(composite) {
          return (self.text.get(start..end).unwrap_or_default(), index + count);
        }
      }
    }
    (self.slice(first.text_range()), index + 1)
  }
}

impl Tokens<'_> {
  /// A group or a literal came: a name after `.` or `::` is a member of an expression
  fn group(&mut self) {
    self.reset();
    self.after_group = true;
  }

  fn reset(&mut self) {
    self.path.clear();
    self.after_colons = false;
    self.after_group = false;
    self.last = None;
  }
}

/// `pub use` passes names on, and so do `pub(crate) use` and `pub(super) use`
const fn is_exported(reach: Reach) -> bool {
  matches!(reach, Reach::Public | Reach::Module | Reach::Parent)
}

const fn is_trivia(kind: K) -> bool {
  matches!(kind, K::WHITESPACE | K::COMMENT | K::OUTER_DOC_COMMENT | K::INNER_DOC_COMMENT)
}

const fn is_comment(kind: K) -> bool {
  matches!(kind, K::COMMENT | K::OUTER_DOC_COMMENT | K::INNER_DOC_COMMENT)
}

/// Whether a token tree is the pattern of a `macro_rules!` rule (the group before a `=>`) or the arguments of a
/// `macro`, whose names are the macro's own (`$x:expr`), not the code's
pub(crate) fn is_rule_pattern(tree: &SyntaxNode) -> bool {
  let parent = tree.parent();
  let rule = parent.as_ref().is_some_and(|it| it.kind() == K::TOKEN_TREE)
    && parent
      .as_ref()
      .and_then(SyntaxNode::parent)
      .is_some_and(|it| matches!(it.kind(), K::MACRO_RULES | K::MACRO_DEF))
    && next_significant_of(tree).is_some_and(|it| it.kind() == K::EQ);
  let arguments = parent.as_ref().is_some_and(|it| it.kind() == K::MACRO_DEF) && tree.next_sibling().is_some_and(|it| it.kind() == K::TOKEN_TREE);
  rule || arguments
}

/// The name of a declaration: its `NAME`, or the `_` of `const _`
fn name_token(node: &SyntaxNode) -> Option<SyntaxToken> {
  if let Some(name) = node.children().find(|it| it.kind() == K::NAME) {
    return name.first_token().filter(|it| it.kind() == K::IDENT);
  }
  (node.kind() == K::CONST)
    .then(|| node.children_with_tokens().filter_map(NodeOrToken::into_token).find(|it| it.kind() == K::UNDERSCORE))
    .flatten()
}

/// Whether a name in a pattern binds: a capitalized one is a variant or a constant (`None`, `MAX`)
fn binds(name: &str) -> bool {
  !name.starts_with(|it: char| it.is_ascii_uppercase())
}

/// Whether a function is a test: an attribute `test` (`#[test]`, `#[tokio::test]`, `#[test_log::test]`) or `rstest`
fn is_test(node: &SyntaxNode) -> bool {
  node.children().filter_map(ast::Attr::cast).filter(|it| it.excl_token().is_none()).any(|attribute| {
    attribute.path().and_then(|path| path.segment()).and_then(|segment| segment.name_ref()).is_some_and(|name| {
      let text = name.text();
      text == "test" || text == "rstest"
    })
  })
}

/// The children of a token tree without its delimiters and whitespace
fn token_children(tree: &SyntaxNode) -> Vec<SyntaxElement> {
  let mut children: Vec<SyntaxElement> = tree.children_with_tokens().filter(|it| it.kind() != K::WHITESPACE).collect();
  if children.first().is_some_and(|it| matches!(it.kind(), K::L_PAREN | K::L_CURLY | K::L_BRACK)) {
    children.remove(0);
  }
  if children.last().is_some_and(|it| matches!(it.kind(), K::R_PAREN | K::R_CURLY | K::R_BRACK)) {
    children.pop();
  }
  children
}

/// Whether the element is a token tree delimited by this token: `(`, `{`
fn delimited_by(element: &SyntaxElement, delimiter: K) -> bool {
  element
    .as_node()
    .is_some_and(|it| it.kind() == K::TOKEN_TREE && it.first_token().is_some_and(|token| token.kind() == delimiter))
}

/// The next sibling of a token that is no trivia
fn next_significant(token: &SyntaxToken) -> Option<SyntaxElement> {
  std::iter::successors(token.next_sibling_or_token(), SyntaxElement::next_sibling_or_token).find(|it| !is_trivia(it.kind()))
}

/// The next sibling of a node that is no trivia
fn next_significant_of(node: &SyntaxNode) -> Option<SyntaxElement> {
  std::iter::successors(node.next_sibling_or_token(), SyntaxElement::next_sibling_or_token).find(|it| !is_trivia(it.kind()))
}

/// Whether the token right after this one, with nothing between, is of `kind` (`:` of `::`)
fn touches_next(token: &SyntaxToken, kind: K) -> bool {
  token.next_sibling_or_token().is_some_and(|it| it.kind() == kind)
}

/// Whether the token right before this one, with nothing between, is of `kind`
fn touches_previous(token: &SyntaxToken, kind: K) -> bool {
  token.prev_sibling_or_token().is_some_and(|it| it.kind() == kind)
}
