//! Go to Declaration and Find Usages of a Rust file (openspec/specs/commit/symbol-navigation/spec.md) from rust-analyzer's tree
//! (`rust_syntax`), with the answers of `navigate` (`reference_answer`, `usage_answers`).
//!
//! - **Names**: the identifiers of the code, a declaration's (`NAME`), a use's (`NAME_REF`) and the tokens of a
//!   macro; not a lifetime or a label, not a primitive type alone in a type (`u8`).
//! - **Kinds**: the name of a declaration, a binding of a pattern (`let`, a parameter, `if let`, a `for`, an arm, a
//!   closure's parameter) and an alias (`as e`) is a `Declaration`; a capitalized name alone in a pattern (`None`) is
//!   a value; a field or a method after `.` and a segment after `::` are members (`MemberCall` when called) with the
//!   text before them as their qualifier, but a segment of a type, which is a `Type` (`fmt::Debug`), as a name in a type,
//!   a struct expression or pattern and an associated type are; a callee is a `Call`.
//! - **Scopes**: a block, a function, a closure, an `impl`, a trait, a module, a match arm and a `for` declare the
//!   names of their own items, bindings and type parameters; an item that is a scope declares its own name in the scope
//!   around it. A name binds to the innermost scope around it that declares it: the last declaration before it there,
//!   else the first (an item after its use). A member binds to nothing.
//! - **Locals**: a declaration in a function or a closure, which no other file sees.
//! - **Access**: a declaration, a call, a name in a `use` (`Import`), the end of the left side of an assignment
//!   (`Write`), else `Read`.
//!
//! The parse, the walks and the drop of the tree run in `rust_syntax::guarded`; a file nested deeper than any code has
//! no answer.

use super::{CONTEXTUAL_KEYWORDS, PRIMITIVES, is_callee, span};
use crate::navigate::{Access, Position, ReferenceAt, ReferenceKind, Usage, UsagesAt, byte_offset, reference_answer, usage_answers};
use rust_syntax::ra_ap_syntax::{AstNode, NodeOrToken, SyntaxKind as K, SyntaxNode, SyntaxToken, TextSize, TokenAtOffset, WalkEvent, ast};
use std::collections::HashMap;

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

/// The usages of the name at `position`: those of its declaration for a local, else those of every name of that text
/// that no local binds; `None` where no name is
pub(crate) fn usages_at(text: &str, position: Position) -> Option<UsagesAt> {
  with_name_at(text, position, |token, name, scopes| {
    let (kind, _) = kind_of(token, text);
    let local = binding_of(token, kind, scopes).filter(is_local);
    let root = token.parent_ancestors().last();
    let usages = root.map_or_else(Vec::new, |root| {
      names_in(&root, text, name, scopes, |binding| match (&local, binding) {
        (Some(local), Some(binding)) => binding == local,
        (Some(_), None) => false,
        (None, binding) => binding.is_none_or(|it| !is_local(it)),
      })
    });
    UsagesAt {
      name: name.to_string(),
      local: local.is_some(),
      usages,
    }
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
    Some(answer(&token, name, &Scopes::of(&root, name)))
  })
  .flatten()
}

/// The usages of `name` in a text that is not the one of the name: every name of that text that no local binds
pub(crate) fn usages_of(text: &str, name: &str) -> Vec<Usage> {
  // a cheap test first: most files of a review do not have the word
  if !text.contains(name) {
    return Vec::new();
  }
  rust_syntax::guarded(text.as_bytes(), || {
    let root = rust_syntax::parse(text.as_bytes());
    let scopes = Scopes::of(&root, name);
    names_in(&root, text, name, &scopes, |binding| binding.is_none_or(|it| !is_local(it)))
  })
  .unwrap_or_default()
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

/// The declarations of one name by the scope that declares them, each in the order of the text
struct Scopes(HashMap<SyntaxNode, Vec<SyntaxToken>>);

impl Scopes {
  /// The declarations of `name` in the tree: each declaring node's name in the innermost scope around the node, its
  /// own scope excluded (a function declares its name in the scope around it)
  fn of(root: &SyntaxNode, name: &str) -> Self {
    let mut scopes: HashMap<SyntaxNode, Vec<SyntaxToken>> = HashMap::new();
    for element in root.descendants_with_tokens() {
      let NodeOrToken::Token(token) = element else { continue };
      if token.kind() != K::IDENT || token.text() != name {
        continue;
      }
      let Some(declaration) = declaring(&token) else { continue };
      if let Some(scope) = declaration.ancestors().skip(1).find(|it| is_scope(it.kind())) {
        scopes.entry(scope).or_default().push(token);
      }
    }
    Self(scopes)
  }

  /// The declaration a name binds to: in the innermost scope around it that declares its name, the last declaration
  /// before it, else the first
  fn declaration_of(&self, token: &SyntaxToken) -> Option<SyntaxToken> {
    let start = token.text_range().start();
    token.parent_ancestors().filter(|it| is_scope(it.kind())).find_map(|scope| {
      let declared = self.0.get(&scope)?;
      declared.iter().rev().find(|it| it.text_range().start() <= start).or_else(|| declared.first()).cloned()
    })
  }
}

/// The declaration a name is bound to: the name itself for a declaration's, none for a member
fn binding_of(token: &SyntaxToken, kind: ReferenceKind, scopes: &Scopes) -> Option<SyntaxToken> {
  match kind {
    ReferenceKind::Declaration => Some(token.clone()),
    ReferenceKind::Value | ReferenceKind::Call | ReferenceKind::Type => scopes.declaration_of(token),
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

/// The names `name` of the tree whose binding `keep` takes, with how each uses it, in the order of the text
fn names_in(root: &SyntaxNode, text: &str, name: &str, scopes: &Scopes, keep: impl Fn(Option<&SyntaxToken>) -> bool) -> Vec<Usage> {
  let mut usages = Vec::new();
  for event in root.preorder_with_tokens() {
    let WalkEvent::Enter(NodeOrToken::Token(token)) = event else { continue };
    if token.text() != name || !is_name(&token, text) {
      continue;
    }
    let (kind, _) = kind_of(&token, text);
    if keep(binding_of(&token, kind, scopes).as_ref()) {
      usages.push((usize::from(token.text_range().start()), access_of(&token, kind)));
    }
  }
  usage_answers(text, name, usages)
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
