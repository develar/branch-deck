//! Go to Declaration and Find Usages of a Rust file (openspec/specs/commit/symbol-navigation/spec.md) from rust-analyzer's tree
//! (`rust_syntax`): the name at a place and the names of one spelling with their bindings (`navigate::Names`), of which
//! `navigate` makes the usages and the answers of `names_at` (openspec/specs/commit/name-bindings/spec.md).
//!
//! - **Names**: the identifiers of the code, a declaration's (`NAME`), a use's (`NAME_REF`) and the tokens of a
//!   macro; not a lifetime or a label, not a primitive type alone in a type (`u8`). A string among the tokens of a
//!   macro may be a format string, so a name it would take from the code around it (`"{total}"`, `"{:width$}"`) is a
//!   use of that name for the usages, though no place to go from.
//! - **Kinds**: the name of a declaration, a binding of a pattern (`let`, a parameter, `if let`, a `for`, an arm, a
//!   closure's parameter) and an alias (`as e`) is a `Declaration`; a capitalized name alone in a pattern (`None`) is
//!   a value; a field or a method after `.`, the name of a field before its colon (`Point { total: 1 }`) and a
//!   segment after `::` are members (`MemberCall` when called) with the text before them as their qualifier, but a
//!   segment of a type, which is a `Type` (`fmt::Debug`), as a name in a type, a struct expression or pattern and an
//!   associated type are; a callee is a `Call`.
//! - **Scopes**: a block, a function, a closure, an `impl`, a trait, a module, a match arm and a `for` declare the
//!   names of their own items, bindings and type parameters; an item that is a scope declares its own name in the scope
//!   around it. A name binds to the innermost scope around it that declares a name it sees: an item, a parameter and
//!   the binding of an arm are seen in all of their scope (the last before the name, else the first: an item after its
//!   use); the binding of a `let` after its statement (`let total = total + 1` reads the `total` before it), the one of
//!   an `if let` or a `while let` in the rest of the condition and the block it guards, the one of a `for` in its
//!   body; a binding shadows an item. A name sees the declarations of its namespace (`Space`): a type, and a segment
//!   before `::`, a type, a module, a type parameter, or a constant or a const parameter (the argument of a generic,
//!   `Array<N>`, is written as a type); the name of a macro call and of an attribute a macro; any other name
//!   everything but a macro. A member binds to nothing.
//! - **Locals**: a declaration in a function or a closure, which no other file sees.
//! - **Access**: a declaration, a call, a name in a `use` (`Import`), the end of the left side of an assignment
//!   (`Write`), else `Read`.
//! - **What a local is** (`LocalBinding`): the binding of a `let`, of an `if let`, a `while let` and an arm a variable,
//!   the one of a `for` a loop variable, of a parameter of a closure or of a function; an item and a type or const
//!   parameter inside a function are `Other`. No call names a parameter, but an attribute macro reads the names of
//!   the function it stands on (`#[tauri::command]`, `#[instrument]`): the parameter of a function under an
//!   attribute that is not one of the language's own says other code may name it. A local of a function or a closure
//!   with a `#[cfg]` inside it is read in part (`TreeNames::is_compiled_by_conditions`), as every local of a text
//!   with an error is.
//! - **Its site** (`Site`): `Point { total }` of a struct expression and of a pattern names the field too; a name
//!   among the tokens of a macro is the macro's to read; any other binding inside a pattern that takes a value apart
//!   is destructured.
//!
//! The parse, the walks and the drop of the tree run in `rust_syntax::guarded`; a file nested deeper than any code has
//! no answer.

use super::{CONTEXTUAL_KEYWORDS, PRIMITIVES, is_callee, span};
use crate::navigate::{Access, Bound, LocalBinding, LocalKind, Names, Position, ReferenceAt, ReferenceKind, Resolved, Site, Wanted, byte_offset, reference_answer, without_bodies};
use rust_syntax::ra_ap_syntax::ast::HasLoopBody;
use rust_syntax::ra_ap_syntax::{AstNode, NodeOrToken, SyntaxKind as K, SyntaxNode, SyntaxToken, TextRange, TextSize, TokenAtOffset, ast};
use std::cell::OnceCell;
use std::collections::HashMap;
use std::ops::Range;

/// The name at `position` of `text` and what its syntax says; `None` where no name is
pub(crate) fn reference_at(text: &str, position: Position) -> Option<ReferenceAt> {
  with_name_at(text, position, |token, name, scopes| {
    let (kind, qualifier) = kind_of(token, text);
    let local = match kind {
      ReferenceKind::Declaration => None,
      _ => binding_of(token, kind, scopes).map(|it| usize::from(it.text_range().start())),
    };
    reference_answer(text, usize::from(token.text_range().start()), name.to_string(), kind, qualifier, local)
  })
}

/// The answer of `answer` for the name at `position` of a parse of `text`, its text and the declarations of that text,
/// all in `rust_syntax::guarded`
fn with_name_at<T: Send>(text: &str, position: Position, answer: impl FnOnce(&SyntaxToken, &str, &Scopes) -> T + Send) -> Option<T> {
  let offset = byte_offset(text, position)?;
  rust_syntax::guarded(text.as_bytes(), || {
    let root = rust_syntax::parse(text.as_bytes());
    let token = name_at(&root, text, offset)?;
    let name = text.get(span(token.text_range()))?;
    Some(answer(&token, name, &Scopes::of(&name_tokens(&root), &Wanted::named(name))))
  })
  .flatten()
}

/// The answer of `read` for the names of a parse of `text` (`navigate::names_at`, `usages_at`, `usages_of`), all in
/// `rust_syntax::guarded`; `None` for a text nested deeper than any code
pub(crate) fn with_names<R: Send>(text: &str, read: impl FnOnce(&dyn Names) -> R + Send) -> Option<R> {
  rust_syntax::guarded(text.as_bytes(), || {
    // with the errors, as the tokens of the text are read: what a text with one is, the two must say alike
    let (root, errors) = rust_syntax::parse_with_errors(text.as_bytes());
    let tokens = name_tokens(&root);
    read(&TreeNames {
      root,
      tokens,
      text,
      broken: !errors.is_empty(),
      conditions: OnceCell::new(),
    })
  })
}

/// A tree and its text, for the names of `navigate::Names`
struct TreeNames<'t> {
  root: SyntaxNode,
  /// The tokens every resolution reads (`name_tokens`)
  tokens: Vec<SyntaxToken>,
  text: &'t str,
  /// The lexer, the parser or the validation reported an error
  broken: bool,
  /// The attributes of the tree that compile what they stand on by a condition (`conditions`), read once
  conditions: OnceCell<Vec<Condition>>,
}

/// An attribute that compiles what it stands on by a condition (`#[cfg(unix)]`): its range and the range of what it
/// stands on
struct Condition {
  attribute: TextRange,
  on: TextRange,
}

/// The attributes of a tree with a `cfg` in them (`#[cfg(…)]`, `#[cfg_attr(…, cfg(…))]`), in the order of the text
fn conditions(root: &SyntaxNode, text: &str) -> Vec<Condition> {
  if !text.contains("cfg") {
    return Vec::new();
  }
  let attributes = root.descendants().filter(|it| it.kind() == K::ATTR && it.descendants().any(|it| it.kind() == K::CFG_META));
  let condition = |attribute: SyntaxNode| {
    Some(Condition {
      attribute: attribute.text_range(),
      on: attribute.parent()?.text_range(),
    })
  };
  attributes.filter_map(condition).collect()
}

impl TreeNames<'_> {
  /// Whether code around the declaration of a local is compiled by a condition: a `#[cfg]` stands inside the
  /// outermost function or closure around it, on a statement, an expression, a parameter or an arm (on the function
  /// itself it compiles all of it or nothing). The scopes read both of `#[cfg(unix)] let total = 1;` and
  /// `#[cfg(windows)] let total = 2;` as declarations of one block, the second hiding the first, where each build has
  /// one of them: the names they bind to a local of such a body are those of no build
  fn is_compiled_by_conditions(&self, declared: &SyntaxToken) -> bool {
    let Some(body) = declared.parent_ancestors().filter(|it| matches!(it.kind(), K::FN | K::CLOSURE_EXPR)).last() else {
      return false;
    };
    let body = body.text_range();
    let conditions = self.conditions.get_or_init(|| conditions(&self.root, self.text));
    let from = conditions.partition_point(|it| it.attribute.start() < body.start());
    conditions[from..].iter().take_while(|it| it.attribute.start() < body.end()).any(|it| it.on != body)
  }
}

/// The tokens a resolution reads, in the order of the text: the identifiers of the tree and the strings among the
/// tokens of a macro, which may be format strings. They are collected once for all the resolutions of a parse: a walk
/// of rust-analyzer's tree costs several times what a resolution does with its tokens
fn name_tokens(root: &SyntaxNode) -> Vec<SyntaxToken> {
  let tokens = root.descendants_with_tokens().filter_map(NodeOrToken::into_token);
  let reads = |token: &SyntaxToken| match token.kind() {
    K::IDENT => true,
    K::STRING => token.parent().is_some_and(|it| it.kind() == K::TOKEN_TREE),
    _ => false,
  };
  tokens.filter(reads).collect()
}

impl Names for TreeNames<'_> {
  fn name_at(&self, offset: usize) -> Option<(usize, &str)> {
    let range = span(name_at(&self.root, self.text, offset)?.text_range());
    Some((range.start, self.text.get(range)?))
  }

  fn resolve(&self, wanted: &Wanted<'_>) -> Vec<Resolved> {
    let scopes = Scopes::of(&self.tokens, wanted);
    let mut resolved = Vec::new();
    let bound = |declared: SyntaxToken| Bound {
      start: usize::from(declared.text_range().start()),
      local: local_of(&declared).map(|it| LocalBinding {
        read_in_part: self.is_compiled_by_conditions(&declared),
        ..it
      }),
    };
    for token in &self.tokens {
      let start = usize::from(token.text_range().start());
      if token.kind() == K::STRING {
        for capture in captures(token, wanted) {
          resolved.push(Resolved {
            start: capture,
            access: Access::Read,
            site: Site::Macro,
            binding: scopes
              .declaration_at(token, TextSize::new(u32::try_from(capture).unwrap_or(u32::MAX)), Space::Value)
              .map(bound),
          });
        }
        continue;
      }
      if !wanted.takes(token.text(), start) || !is_name(token, self.text) {
        continue;
      }
      let (kind, _) = kind_of(token, self.text);
      let binding = binding_of(token, kind, &scopes).map(bound);
      resolved.push(Resolved {
        start,
        access: access_of(token, kind),
        site: site_of(token),
        binding,
      });
    }
    resolved
  }

  /// `self` and `Self` are keywords, no names
  fn is_implicit(&self, _name: &str) -> bool {
    false
  }

  /// A text with an error: what stands where the parser gave up is no name of the tree
  fn reads_in_part(&self) -> bool {
    self.broken
  }

  fn statements_of(&self, starts: &[usize], local: bool) -> Vec<Vec<Range<usize>>> {
    // the token that holds the byte: a name, or the string of a macro that names one
    let token_at = |start: usize| {
      let after = self.tokens.partition_point(|token| usize::from(token.text_range().start()) <= start);
      after
        .checked_sub(1)
        .map(|at| &self.tokens[at])
        .filter(|token| start < usize::from(token.text_range().end()))
    };
    let statement = if local { member_of } else { statement_of };
    starts.iter().map(|&start| token_at(start).map_or_default(statement)).collect()
  }
}

/// The statement of the declared name `token`, which is not local (`navigate`, "The statements of a name"): the
/// innermost node around it that `is_statement` (the file where none is), from its first token that is code to its
/// last one, without the items of the nodes from the name up to it (those of a module, an `impl` or a trait)
fn statement_of(token: &SyntaxToken) -> Vec<Range<usize>> {
  let mut items = Vec::new();
  let mut statement = None;
  for node in token.parent_ancestors() {
    items.extend(
      node
        .children()
        .filter(|child| matches!(child.kind(), K::ITEM_LIST | K::ASSOC_ITEM_LIST))
        .map(|child| own_range(&child)),
    );
    let ends = is_statement(&node);
    statement = Some(node);
    if ends {
      break;
    }
  }
  statement.map_or_default(|statement| without_bodies(own_range(&statement), items))
}

/// The member around the name `token` of a local (`navigate`, "The statements of a name"): the outermost node
/// around it that `is_statement` and holds no items of its own (a module, an `impl`, a trait), from its first token
/// that is code to its last one; none where no such node is around the name
fn member_of(token: &SyntaxToken) -> Vec<Range<usize>> {
  let is_type = |node: &SyntaxNode| matches!(node.kind(), K::MODULE | K::IMPL | K::TRAIT | K::EXTERN_BLOCK);
  let member = token
    .parent_ancestors()
    .filter(|node| node.parent().is_some() && is_statement(node) && !is_type(node))
    .last();
  member.map_or_default(|member| vec![own_range(&member)])
}

/// The bytes of a node from its first token that is code to its last one: without the comments and the whitespace
/// at its ends (the doc comment of an item is a part of its node)
fn own_range(node: &SyntaxNode) -> Range<usize> {
  let range = node.text_range();
  let within = |token: &SyntaxToken| range.contains_range(token.text_range());
  // a doc comment is a node of its own in this tree
  let is_code = |token: &SyntaxToken| !token.kind().is_trivia() && token.parent().is_none_or(|parent| parent.kind() != K::DOC_COMMENT);
  let first = std::iter::successors(node.first_token(), SyntaxToken::next_token).take_while(within).find(is_code);
  let last = std::iter::successors(node.last_token(), SyntaxToken::prev_token).take_while(within).find(is_code);
  match (first, last) {
    (Some(first), Some(last)) => usize::from(first.text_range().start())..usize::from(last.text_range().end()),
    _ => span(range),
  }
}

/// Whether a node is a statement or an item (`navigate`, "The statements of a name"): what stands in a block, in a
/// file, in a module or in an `impl` or a trait (the value a block ends with too), and the value of an arm of a
/// `match`, a branch that is no block
fn is_statement(node: &SyntaxNode) -> bool {
  let Some(parent) = node.parent() else { return true };
  matches!(parent.kind(), K::STMT_LIST | K::SOURCE_FILE | K::ITEM_LIST | K::ASSOC_ITEM_LIST) || is_arm_value(node, &parent)
}

/// Whether `node` is the value of the arm `parent` of a `match`
fn is_arm_value(node: &SyntaxNode, parent: &SyntaxNode) -> bool {
  ast::MatchArm::cast(parent.clone()).and_then(|arm| arm.expr()).is_some_and(|value| value.syntax() == node)
}

/// The name token at `offset`: the character there, else the one before it, as the end of a name is still the name
/// (`foo|`)
fn name_at(root: &SyntaxNode, text: &str, offset: usize) -> Option<SyntaxToken> {
  let at = TextSize::new(u32::try_from(offset).ok()?);
  let (after, before) = match root.token_at_offset(at) {
    TokenAtOffset::None => return None,
    TokenAtOffset::Single(token) => {
      let before = (token.text_range().start() == at).then(|| token.prev_token()).flatten();
      (Some(token), before)
    }
    TokenAtOffset::Between(left, right) => (Some(right), Some(left)),
  };
  after.into_iter().chain(before).find(|it| is_name(it, text))
}

/// Whether a token is a name of the code: an identifier of a declaration, of a use or among the tokens of a macro
fn is_name(token: &SyntaxToken, text: &str) -> bool {
  if token.kind() != K::IDENT {
    return false;
  }
  let Some(parent) = token.parent() else { return false };
  match parent.kind() {
    K::NAME => true,
    K::NAME_REF => !is_primitive_type(&parent, token, text),
    K::TOKEN_TREE => {
      let word = text.get(span(token.text_range())).unwrap_or_default();
      !PRIMITIVES.contains(&word) && !CONTEXTUAL_KEYWORDS.contains(&word) && !is_metavariable(token) && !in_rule_pattern(&parent)
    }
    _ => false,
  }
}

/// The bytes where a string among the tokens of a macro names what a resolution looks for as a format string would take
/// it from the code around the macro: a word right after the `{` of a placeholder (`{total}`, `{total:>8}`) or before
/// a `$` (`{:width$}`, and `{:0width$}` after a flag: a digit starts no word). Whether the macro formats is its own:
/// a string that only looks so costs a use too many, never one too few
fn captures(token: &SyntaxToken, wanted: &Wanted<'_>) -> Vec<usize> {
  let text = token.text();
  if wanted.also.is_empty() && !text.contains(wanted.name) {
    return Vec::new();
  }
  let offset = usize::from(token.text_range().start());
  let is_word = |it: char| it == '_' || it.is_alphanumeric();
  let mut found = Vec::new();
  let mut characters = text.char_indices().peekable();
  while let Some((start, character)) = characters.next() {
    // a word starts at a letter or `_` and takes every letter and digit after it, so the character before it is none
    // of its own
    if !(character == '_' || character.is_alphabetic()) {
      continue;
    }
    let mut end = start + character.len_utf8();
    while let Some(&(next, it)) = characters.peek().filter(|it| is_word(it.1)) {
      end = next + it.len_utf8();
      characters.next();
    }
    let before = &text[..start];
    // `{{` writes a brace, and the braces of `\u{…}` hold a code point
    let opens = before.len() - before.trim_end_matches('{').len();
    let placeholder = opens % 2 == 1 && !before.trim_end_matches('{').ends_with("\\u");
    if (placeholder || text[end..].starts_with('$')) && wanted.takes(&text[start..end], offset + start) {
      found.push(offset + start);
    }
  }
  found
}

/// A name after `$` among the tokens of a macro (`$x`, `$crate`): the macro's, not the code's
fn is_metavariable(token: &SyntaxToken) -> bool {
  token.prev_sibling_or_token().is_some_and(|it| it.kind() == K::DOLLAR)
}

/// Whether a token tree is in the pattern of a `macro_rules!` rule or the arguments of a `macro`, whose names are the
/// macro's own (`$x:ident`)
fn in_rule_pattern(tree: &SyntaxNode) -> bool {
  std::iter::successors(Some(tree.clone()), SyntaxNode::parent)
    .take_while(|it| it.kind() == K::TOKEN_TREE)
    .any(|it| super::summary::is_rule_pattern(&it))
}

/// A primitive type alone in a type (`u8`): no name, as tree-sitter's grammar read it
fn is_primitive_type(name_ref: &SyntaxNode, token: &SyntaxToken, text: &str) -> bool {
  let Some(path) = name_ref.parent().and_then(|segment| segment.parent()) else {
    return false;
  };
  path.parent().is_some_and(|it| it.kind() == K::PATH_TYPE)
    && !path.children().any(|it| it.kind() == K::PATH)
    && PRIMITIVES.contains(&text.get(span(token.text_range())).unwrap_or_default())
}

/// The node a name declares when the name is a declaration's: an item, a field, a variant, a type or const parameter,
/// a binding of a pattern, an alias
fn declaring(token: &SyntaxToken) -> Option<SyntaxNode> {
  let name = token.parent().filter(|it| it.kind() == K::NAME)?;
  let declaration = name.parent()?;
  let declares = match declaration.kind() {
    K::FN
    | K::STRUCT
    | K::UNION
    | K::ENUM
    | K::VARIANT
    | K::RECORD_FIELD
    | K::TRAIT
    | K::TYPE_ALIAS
    | K::CONST
    | K::STATIC
    | K::MODULE
    | K::MACRO_RULES
    | K::MACRO_DEF
    | K::TYPE_PARAM
    | K::CONST_PARAM
    | K::RENAME => true,
    K::IDENT_PAT => !token.text().starts_with(|it: char| it.is_ascii_uppercase()),
    _ => false,
  };
  declares.then_some(declaration)
}

/// Whether a node opens a scope: the names declared inside are not seen outside
const fn is_scope(kind: K) -> bool {
  matches!(
    kind,
    K::SOURCE_FILE | K::BLOCK_EXPR | K::FN | K::CLOSURE_EXPR | K::IMPL | K::TRAIT | K::MODULE | K::MATCH_ARM | K::FOR_EXPR
  )
}

/// What a name may be, as Rust keeps its namespaces apart
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
  /// A type, a module, a type parameter, and the constant or the const parameter a generic takes as an argument: a
  /// name in a type, and a segment of a path before `::`
  Type,
  /// A macro: the name of a macro call and of an attribute
  Macro,
  /// Anything but a macro: a value, and what the syntax does not tell (a token of a macro)
  Value,
}

impl Space {
  /// The namespace a name reads by its place
  fn of(token: &SyntaxToken, kind: ReferenceKind) -> Self {
    let path = token
      .parent()
      .filter(|it| it.kind() == K::NAME_REF)
      .and_then(|it| it.parent())
      .filter(|it| it.kind() == K::PATH_SEGMENT)
      .and_then(|it| it.parent());
    match path.as_ref().and_then(SyntaxNode::parent).map(|it| it.kind()) {
      _ if kind == ReferenceKind::Type => Self::Type,
      // `total::run`: only a module or a type has a path after it
      Some(K::PATH) => Self::Type,
      Some(K::MACRO_CALL | K::PATH_META | K::KEY_VALUE_META | K::TOKEN_TREE_META) => Self::Macro,
      _ => Self::Value,
    }
  }

  /// Whether a name of the namespace may be what the node `declaration` declares
  fn holds(self, declaration: K) -> bool {
    let is_macro = matches!(declaration, K::MACRO_RULES | K::MACRO_DEF);
    match self {
      // an argument of a generic that is one name is parsed as a type, and may be a constant: `Array<N>`, `Array<CAP>`
      Self::Type => matches!(
        declaration,
        K::STRUCT | K::UNION | K::ENUM | K::VARIANT | K::TRAIT | K::TYPE_ALIAS | K::MODULE | K::TYPE_PARAM | K::CONST_PARAM | K::CONST | K::RENAME
      ),
      Self::Macro => is_macro || declaration == K::RENAME,
      Self::Value => !is_macro,
    }
  }
}

/// An item whose code sees the items around it, never their bindings
const fn is_item(kind: K) -> bool {
  matches!(
    kind,
    K::FN | K::CONST | K::STATIC | K::IMPL | K::TRAIT | K::MODULE | K::STRUCT | K::ENUM | K::UNION | K::TYPE_ALIAS
  )
}

/// The declarations of the names a resolution looks for by the scope that declares them, each in the order of the text
/// with where its scope sees it and the kind of the node that declares it
struct Scopes(HashMap<SyntaxNode, Vec<Declaration>>);

/// A declaration of a scope: its name, where the names of the scope see it, and the kind of the node that declares it
struct Declaration {
  name: SyntaxToken,
  seen: TextRange,
  kind: K,
}

impl Scopes {
  /// The declarations among the names a resolution looks for, of the tokens of a tree (`name_tokens`): each declaring
  /// node's name in the innermost scope around the node, its own scope excluded (a function declares its name in the
  /// scope around it)
  fn of(tokens: &[SyntaxToken], wanted: &Wanted<'_>) -> Self {
    let mut scopes: HashMap<SyntaxNode, Vec<Declaration>> = HashMap::new();
    for token in tokens {
      if token.kind() != K::IDENT || !wanted.takes(token.text(), usize::from(token.text_range().start())) {
        continue;
      }
      let Some(declaration) = declaring(token) else { continue };
      if let Some(scope) = declaration.ancestors().skip(1).find(|it| is_scope(it.kind())) {
        let declared = Declaration {
          name: token.clone(),
          seen: seen_in(&declaration, &scope),
          kind: declaration.kind(),
        };
        scopes.entry(scope).or_default().push(declared);
      }
    }
    Self(scopes)
  }

  /// The declaration a name of the namespace `space` at the byte `start` of a token binds to (the token itself, or a
  /// name inside a string): in the innermost scope around it that declares a name of the namespace it sees, the
  /// binding that shadows last (the one seen from the latest place), and among the declarations seen from one place
  /// (the items of the scope) the last before the name, else the first. An item sees no binding of the code around
  /// it: a `fn` inside a function reads the items alone
  fn declaration_at(&self, token: &SyntaxToken, start: TextSize, space: Space) -> Option<SyntaxToken> {
    let mut in_item = false;
    for scope in token.parent_ancestors() {
      if let Some(declared) = self.0.get(&scope).filter(|_| is_scope(scope.kind())) {
        let sees = |it: &&Declaration| it.seen.contains(start) && space.holds(it.kind) && !(in_item && it.kind == K::IDENT_PAT);
        if let Some(from) = declared.iter().filter(sees).map(|it| it.seen.start()).max() {
          let seen = || declared.iter().filter(sees).filter(|it| it.seen.start() == from).map(|it| &it.name);
          let before = seen().rev().find(|it| it.text_range().start() <= start);
          return before.or_else(|| seen().next()).cloned();
        }
      }
      in_item |= is_item(scope.kind());
    }
    None
  }
}

/// Where the names of its scope see a declaration: all of the scope for an item, a parameter and the binding of an arm;
/// after its statement for the binding of a `let`, whose value reads the names before it; the rest of the condition
/// and the block it guards for the binding of an `if let` or a `while let`; its body for the binding of a `for`
fn seen_in(declaration: &SyntaxNode, scope: &SyntaxNode) -> TextRange {
  let whole = scope.text_range();
  let holder = Some(declaration)
    .filter(|it| it.kind() == K::IDENT_PAT)
    .and_then(|it| it.ancestors().skip(1).find(|it| !is_pattern_part(it.kind())));
  let Some(holder) = holder else { return whole };
  let after = |node: &SyntaxNode, end: Option<TextSize>| TextRange::new(node.text_range().end(), end.unwrap_or_else(|| whole.end()).max(node.text_range().end()));
  match holder.kind() {
    K::LET_STMT => after(&holder, None),
    K::LET_EXPR => {
      let guarded = holder.ancestors().skip(1).take_while(|it| it != scope).find_map(|it| match it.kind() {
        K::IF_EXPR => ast::IfExpr::cast(it).and_then(|it| it.then_branch()).map(|it| it.syntax().text_range().end()),
        K::WHILE_EXPR => ast::WhileExpr::cast(it).and_then(|it| it.loop_body()).map(|it| it.syntax().text_range().end()),
        _ => None,
      });
      after(&holder, guarded)
    }
    K::FOR_EXPR => ast::ForExpr::cast(holder).and_then(|it| it.loop_body()).map_or(whole, |it| it.syntax().text_range()),
    _ => whole,
  }
}

/// The declaration a name is bound to: the name itself for a declaration's, none for a member
fn binding_of(token: &SyntaxToken, kind: ReferenceKind, scopes: &Scopes) -> Option<SyntaxToken> {
  match kind {
    ReferenceKind::Declaration => Some(token.clone()),
    ReferenceKind::Value | ReferenceKind::Call | ReferenceKind::Type => scopes.declaration_at(token, token.text_range().start(), Space::of(token, kind)),
    ReferenceKind::Member | ReferenceKind::MemberCall => None,
  }
}

/// Whether the name of a declaration is a local: a function or a closure around what it declares
fn is_local(declared: &SyntaxToken) -> bool {
  let Some(declaration) = declared.parent().and_then(|it| it.parent()) else {
    return false;
  };
  declaration.ancestors().skip(1).any(|it| matches!(it.kind(), K::FN | K::CLOSURE_EXPR))
}

/// A node of a pattern between a binding and what the pattern is of: a pattern, or a field of a record pattern
fn is_pattern_part(kind: K) -> bool {
  ast::Pat::can_cast(kind) || matches!(kind, K::RECORD_PAT_FIELD | K::RECORD_PAT_FIELD_LIST)
}

/// What the name of a declaration is when a function or a closure declares it: a binding by what its pattern is of,
/// else `Other`. No call names a parameter; an attribute macro on its function may (`is_read_by_attribute`)
fn local_of(declared: &SyntaxToken) -> Option<LocalBinding> {
  if !is_local(declared) {
    return None;
  }
  let binding = declared.parent().and_then(|it| it.parent()).filter(|it| it.kind() == K::IDENT_PAT);
  let holder = binding.and_then(|it| it.ancestors().skip(1).find(|it| !is_pattern_part(it.kind())));
  let kind = match holder.as_ref().map(SyntaxNode::kind) {
    Some(K::LET_STMT | K::LET_EXPR | K::MATCH_ARM) => LocalKind::Variable,
    Some(K::FOR_EXPR) => LocalKind::LoopVariable,
    Some(K::PARAM) => match holder.as_ref().and_then(SyntaxNode::parent).and_then(|it| it.parent()).map(|it| it.kind()) {
      Some(K::CLOSURE_EXPR) => LocalKind::LambdaParameter,
      Some(K::FN) => LocalKind::Parameter,
      _ => LocalKind::Other,
    },
    _ => LocalKind::Other,
  };
  Some(LocalBinding {
    named_by_calls: kind == LocalKind::Parameter && holder.is_some_and(|it| is_read_by_attribute(&it)),
    ..LocalBinding::of(kind)
  })
}

/// The attributes of the language and of its tools, which read no name of what they stand on
const INERT_ATTRIBUTES: [&str; 53] = [
  "allow",
  "automatically_derived",
  "cfg",
  "cold",
  "collapse_debuginfo",
  "coverage",
  "crate_name",
  "crate_type",
  "debugger_visualizer",
  "deny",
  "deprecated",
  "derive",
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
  "link_ordinal",
  "link_section",
  "macro_export",
  "macro_use",
  "must_use",
  "naked",
  "no_builtins",
  "no_implicit_prelude",
  "no_link",
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
  "should_panic",
  "target_feature",
  "test",
  "track_caller",
  "type_length_limit",
  "used",
  "warn",
  "windows_subsystem",
  "bench",
];

/// The tools whose attributes the compiler knows (`#[rustfmt::skip]`, `#[clippy::…]`)
const ATTRIBUTE_TOOLS: [&str; 5] = ["clippy", "diagnostic", "miri", "rust_analyzer", "rustfmt"];

/// Whether an attribute macro may read the name of a parameter: its function, an item around it or the parameter
/// itself stands under an attribute that is not one of the language's own (`#[tauri::command]` and `#[instrument]`
/// name the parameters in what they generate), which gets the tokens of all it stands on
fn is_read_by_attribute(parameter: &SyntaxNode) -> bool {
  let attributes = |node: SyntaxNode| node.children().filter(|it| it.kind() == K::ATTR).flat_map(|it| it.children());
  parameter.ancestors().flat_map(attributes).any(|it| !is_inert_meta(&it))
}

/// Whether what an attribute holds is of the language or of its tools: a `cfg`, a path of theirs, or a `cfg_attr` or
/// an `unsafe(…)` of such alone
fn is_inert_meta(meta: &SyntaxNode) -> bool {
  match meta.kind() {
    K::CFG_META => true,
    K::CFG_ATTR_META | K::UNSAFE_META => meta
      .children()
      .filter(|it| {
        matches!(
          it.kind(),
          K::PATH_META | K::KEY_VALUE_META | K::TOKEN_TREE_META | K::CFG_ATTR_META | K::CFG_META | K::UNSAFE_META
        )
      })
      .all(|it| is_inert_meta(&it)),
    _ => meta.children().find(|it| it.kind() == K::PATH).is_some_and(|it| {
      let path: String = it.text().to_string().split_whitespace().collect();
      is_inert_path(&path)
    }),
  }
}

/// Whether the path of an attribute names one of the language or of its tools
fn is_inert_path(path: &str) -> bool {
  match path.split_once("::") {
    Some((tool, _)) => ATTRIBUTE_TOOLS.contains(&tool),
    None => INERT_ATTRIBUTES.contains(&path),
  }
}

/// What the site of a name makes of it beside its binding (`Site`)
fn site_of(token: &SyntaxToken) -> Site {
  let Some(parent) = token.parent() else { return Site::Plain };
  match parent.kind() {
    K::TOKEN_TREE => Site::Macro,
    K::NAME => pattern_site(&parent),
    K::NAME_REF => use_site(&parent),
    _ => Site::Plain,
  }
}

/// The site of the name of a binding: `Point { total }` names the field too, any other binding inside a pattern that
/// takes a value apart is destructured
fn pattern_site(name: &SyntaxNode) -> Site {
  let Some(binding) = name.parent().filter(|it| it.kind() == K::IDENT_PAT) else {
    return Site::Plain;
  };
  let mut parts = binding.ancestors().skip(1).take_while(|it| is_pattern_part(it.kind()));
  match parts.next() {
    // without a field's name before a colon, the binding is the field's name
    Some(field) if field.kind() == K::RECORD_PAT_FIELD && !field.children().any(|it| it.kind() == K::NAME_REF) => Site::Shorthand,
    Some(_) => Site::Destructured,
    None => Site::Plain,
  }
}

/// The site of a name that declares nothing: the field a struct expression names by its value (`Point { total }`)
fn use_site(name_ref: &SyntaxNode) -> Site {
  let path = name_ref
    .parent()
    .filter(|it| it.kind() == K::PATH_SEGMENT)
    .and_then(|it| it.parent())
    .filter(|it| !it.children().any(|it| it.kind() == K::PATH));
  let field = path
    .and_then(|it| it.parent())
    .filter(|it| it.kind() == K::PATH_EXPR)
    .and_then(|it| it.parent())
    .filter(|it| it.kind() == K::RECORD_EXPR_FIELD);
  // without a field's name before a colon, the value is the field's name
  if field.is_some_and(|it| !it.children().any(|it| it.kind() == K::NAME_REF)) {
    Site::Shorthand
  } else {
    Site::Plain
  }
}

/// How a name is used: declared, imported, called, written (the end of the left side of an assignment), else read
fn access_of(token: &SyntaxToken, kind: ReferenceKind) -> Access {
  match kind {
    ReferenceKind::Declaration => return Access::Declaration,
    ReferenceKind::Call | ReferenceKind::MemberCall => return Access::Call,
    _ => {}
  }
  let range = token.text_range();
  for ancestor in token.parent_ancestors() {
    match ancestor.kind() {
      K::USE => return Access::Import,
      K::BIN_EXPR => {
        let Some(assignment) = ast::BinExpr::cast(ancestor).filter(|it| matches!(it.op_kind(), Some(ast::BinaryOp::Assignment { .. }))) else {
          continue;
        };
        let left = assignment.lhs().map(|it| it.syntax().text_range());
        return if left.is_some_and(|it| it.end() == range.end() && it.start() <= range.start()) {
          Access::Write
        } else {
          Access::Read
        };
      }
      kind if is_scope(kind) => return Access::Read,
      _ => {}
    }
  }
  Access::Read
}

/// The kind of a name by its place, and the text of the qualifier before it
fn kind_of(token: &SyntaxToken, text: &str) -> (ReferenceKind, Option<String>) {
  let text_of = |node: &SyntaxNode| text.get(span(node.text_range())).unwrap_or_default().to_string();
  if declaring(token).is_some() {
    return (ReferenceKind::Declaration, None);
  }
  let Some(parent) = token.parent() else { return (ReferenceKind::Value, None) };
  if parent.kind() != K::NAME_REF {
    return (ReferenceKind::Value, None);
  }
  let Some(context) = parent.parent() else { return (ReferenceKind::Value, None) };
  match context.kind() {
    K::FIELD_EXPR => {
      let kind = if is_callee(&context) { ReferenceKind::MemberCall } else { ReferenceKind::Member };
      (kind, ast::FieldExpr::cast(context).and_then(|it| it.expr()).map(|it| text_of(it.syntax())))
    }
    K::METHOD_CALL_EXPR => (
      ReferenceKind::MemberCall,
      ast::MethodCallExpr::cast(context).and_then(|it| it.receiver()).map(|it| text_of(it.syntax())),
    ),
    K::ASSOC_TYPE_ARG => (ReferenceKind::Type, None),
    // `Point { total: 1 }`, `Point { total: read }`: the field before the colon, which no scope binds
    K::RECORD_EXPR_FIELD | K::RECORD_PAT_FIELD => (ReferenceKind::Member, None),
    K::PATH_SEGMENT => path_kind(&context, &text_of),
    _ => (ReferenceKind::Value, None),
  }
}

/// The kind of a segment of a path and the text of the path before it: a type in a type, a struct expression or pattern
/// (and a name with type arguments before `::`), a member after `::`, a callee a call
fn path_kind(segment: &SyntaxNode, text_of: &impl Fn(&SyntaxNode) -> String) -> (ReferenceKind, Option<String>) {
  let Some(path) = segment.parent() else { return (ReferenceKind::Value, None) };
  let qualifier = path.children().find(|it| it.kind() == K::PATH);
  let outer = path.parent();
  let typed = outer.as_ref().is_some_and(|it| matches!(it.kind(), K::PATH_TYPE | K::RECORD_EXPR | K::RECORD_PAT));
  let callee = outer.as_ref().is_some_and(|it| it.kind() == K::PATH_EXPR && is_callee(it));
  match qualifier {
    Some(qualifier) => {
      let kind = if typed {
        ReferenceKind::Type
      } else if callee {
        ReferenceKind::MemberCall
      } else {
        ReferenceKind::Member
      };
      (kind, Some(text_of(&qualifier)))
    }
    None if typed => (ReferenceKind::Type, None),
    None if outer.as_ref().is_some_and(|it| it.kind() == K::PATH) && segment.children().any(|it| it.kind() == K::GENERIC_ARG_LIST) => (ReferenceKind::Type, None),
    None if callee => (ReferenceKind::Call, None),
    None => (ReferenceKind::Value, None),
  }
}
