//! The highlighting of a Rust file from rust-analyzer's tree (`rust_syntax`), after `RsColor` of `RustRover`: what its
//! lexer and its annotators colour that a syntax tree shows without resolving a name. One preorder walk over the tokens,
//! which tile the text (`paint::paint_leaf`); a node decides the colour of the names below it before the walk reaches
//! them (a call its callee, a declaration its name), and the scopes of the walk (`Scopes`) hold the locals, whose `mut`
//! ones are underlined, and the type parameters of a function.
//!
//! - **Tokens**: a keyword by its kind (`SyntaxKind::is_keyword` of the edition, the contextual ones the parser remaps
//!   included), `self` apart, a primitive type alone in a type as a keyword, `?`, a lifetime or a label, a number, a
//!   string with its escapes (`ast::IsString::escaped_char_ranges`), a comment, a doc comment with its tags.
//! - **Names**: a function's declaration and a call's callee (`f()`, `a.f()`, `T::f()`) as a function declaration, a
//!   macro's path and its `!`, a field, a constant (italic) and a static, a variant, a type parameter of a function, an
//!   `ALL_CAPS` name or a capitalized segment of a value path after a type (`Kind::Small`) as a constant.
//! - **Attributes**: the colour of an annotation, a literal inside painted over it.
//! - **Errors**: inside an `ERROR` node a keyword, a string and a comment keep their colours, the rest is plain.

use super::{PRIMITIVES, is_callee, span};
use crate::highlight::lines::{Builder, LineRuns};
use crate::highlight::paint::{Paint, comment_paint, paint_leaf};
use crate::highlight::scopes::Scopes;
use crate::highlight::{FIELD, ITALIC, Role, UNDERLINE, is_constant_name};
use rust_syntax::ra_ap_syntax::ast::HasName;
use rust_syntax::ra_ap_syntax::{AstNode, AstToken, NodeOrToken, SyntaxKind as K, SyntaxNode, SyntaxToken, WalkEvent, ast};
use std::collections::HashMap;

const CONSTANT: u32 = Role::Field.style() | ITALIC;
/// A `mut` binding, underlined as in `RustRover`
const MUTABLE: u32 = Role::Foreground.style() | UNDERLINE;

/// The highlighting of a Rust text (its size within `highlight::MAX_BYTES`); `None` for a file nested deeper than any
/// code (`rust_syntax::guarded`)
pub(crate) fn highlight(text: &str) -> Option<LineRuns> {
  rust_syntax::guarded(text.as_bytes(), || {
    let root = rust_syntax::parse(text.as_bytes());
    let mut walk = Walk {
      text,
      out: LineRuns::builder(text),
      scopes: Scopes::default(),
      open: Vec::new(),
      styles: Vec::new(),
      marks: HashMap::new(),
      nodes: 0,
      lets: Vec::new(),
      errors: 0,
      attributes: 0,
    };
    walk.run(&root);
    walk.out.finish()
  })
}

/// A node the walk is inside: its number (the scopes it opened are popped by it) and whether it gives its tokens a style
struct Open {
  id: usize,
  /// It pushed a style on `Walk::styles`
  styled: bool,
}

struct Walk<'a> {
  text: &'a str,
  out: Builder<'a>,
  scopes: Scopes,
  open: Vec<Open>,
  /// The styles of the nodes around the token, the innermost last
  styles: Vec<u32>,
  /// The styles parents gave the tokens below them, by their start
  marks: HashMap<usize, u32>,
  nodes: usize,
  /// The names of each `let` the walk is in, bound when it is left
  lets: Vec<Vec<(String, u32)>>,
  errors: u32,
  attributes: u32,
}

impl<'a> Walk<'a> {
  fn run(&mut self, root: &SyntaxNode) {
    for event in root.preorder_with_tokens() {
      match event {
        WalkEvent::Enter(NodeOrToken::Node(node)) => self.enter(&node),
        WalkEvent::Leave(NodeOrToken::Node(node)) => self.leave(&node),
        WalkEvent::Enter(NodeOrToken::Token(token)) => self.token(&token),
        WalkEvent::Leave(NodeOrToken::Token(_)) => {}
      }
    }
  }

  fn text_of(&self, token: &SyntaxToken) -> &'a str {
    self.text.get(span(token.text_range())).unwrap_or_default()
  }

  fn mark(&mut self, token: Option<SyntaxToken>, style: u32) {
    if let Some(token) = token {
      self.marks.insert(usize::from(token.text_range().start()), style);
    }
  }

  fn enter(&mut self, node: &SyntaxNode) {
    self.nodes += 1;
    let id = self.nodes;
    let kind = node.kind();
    let mut style = None;
    match kind {
      K::ERROR => self.errors += 1,
      K::ATTR => {
        self.attributes += 1;
        style = Some(Role::Annotation.style());
      }
      K::FN => {
        self.scopes.push_local(id);
        self.mark(name_of(node), Role::FunctionDeclaration.style());
      }
      K::CLOSURE_EXPR | K::BLOCK_EXPR | K::FOR_EXPR | K::MATCH_ARM | K::IF_EXPR | K::WHILE_EXPR => self.scopes.push_local(id),
      K::MACRO_RULES | K::MACRO_DEF => self.mark(name_of(node), Role::Macro.style()),
      // a constant is italic, a static not, as in RustRover
      K::CONST => self.mark(name_of(node), CONSTANT),
      K::STATIC | K::RECORD_FIELD => self.mark(name_of(node), FIELD),
      K::VARIANT => self.mark(name_of(node), Role::EnumVariant.style()),
      // RustRover colours a call as the function it calls: `f()`, `item.clone()`, `Vec::new()`
      K::CALL_EXPR => {
        let callee = ast::CallExpr::cast(node.clone()).and_then(|it| it.expr());
        if let Some(ast::Expr::PathExpr(path)) = callee {
          let name = path.path().and_then(|it| it.segment()).and_then(|it| it.name_ref()).and_then(|it| it.ident_token());
          self.mark(name, Role::FunctionDeclaration.style());
        }
      }
      K::METHOD_CALL_EXPR => {
        let name = ast::MethodCallExpr::cast(node.clone()).and_then(|it| it.name_ref()).and_then(|it| it.ident_token());
        self.mark(name, Role::FunctionDeclaration.style());
      }
      K::MACRO_CALL => {
        let call = ast::MacroCall::cast(node.clone());
        let path = call.as_ref().and_then(ast::MacroCall::path);
        for token in path.iter().flat_map(|it| it.syntax().descendants_with_tokens()).filter_map(NodeOrToken::into_token) {
          self.mark(Some(token), Role::Macro.style());
        }
        self.mark(call.and_then(|it| it.excl_token()), Role::Macro.style());
      }
      K::GENERIC_PARAM_LIST => {
        for parameter in node.children().filter(|it| it.kind() == K::TYPE_PARAM) {
          if let Some(name) = name_of(&parameter) {
            let name = self.text_of(&name).to_string();
            self.scopes.bind_type_parameter(&name);
          }
        }
      }
      K::PARAM => {
        for (name, style) in self.pattern_names(node) {
          self.scopes.bind_styled(&name, style);
        }
      }
      // a `let`'s names are seen after it: its initializer sees the bindings it shadows (`let x = x.trim();`)
      K::LET_STMT => {
        let names = self.pattern_names(node);
        self.lets.push(names);
      }
      _ => {}
    }
    if let Some(style) = style {
      self.styles.push(style);
    }
    self.open.push(Open { id, styled: style.is_some() });
  }

  fn leave(&mut self, node: &SyntaxNode) {
    if let Some(open) = self.open.pop() {
      self.scopes.pop(open.id);
      if open.styled {
        self.styles.pop();
      }
    }
    match node.kind() {
      K::ERROR => self.errors -= 1,
      K::ATTR => self.attributes -= 1,
      K::LET_STMT => {
        for (name, style) in self.lets.pop().unwrap_or_default() {
          self.scopes.bind_styled(&name, style);
        }
      }
      _ => {}
    }
  }

  /// The names the pattern of a parameter or a `let` binds, with their style: a `mut` one is underlined wherever it is
  /// used, its declaration too
  fn pattern_names(&mut self, node: &SyntaxNode) -> Vec<(String, u32)> {
    let Some(pattern) = node.children().find(|it| ast::Pat::can_cast(it.kind())) else {
      return Vec::new();
    };
    let mut names = Vec::new();
    for binding in std::iter::once(pattern.clone()).chain(pattern.descendants()).filter_map(ast::IdentPat::cast) {
      let Some(name) = binding.name().and_then(|it| it.ident_token()) else { continue };
      let text = self.text_of(&name).to_string();
      let style = if binding.mut_token().is_some() { MUTABLE } else { 0 };
      if style != 0 {
        self.mark(Some(name), style);
      }
      names.push((text, style));
    }
    names
  }

  fn token(&mut self, token: &SyntaxToken) {
    let around = self.styles.last().copied().unwrap_or(0);
    let start = usize::from(token.text_range().start());
    let text = self.text_of(token);
    let kind = token.kind();
    if matches!(kind, K::STRING | K::BYTE_STRING | K::C_STRING) {
      self.string(token, start, around);
      return;
    }
    let paint = match self.marks.remove(&start) {
      Some(style) => Paint::Leaf(style),
      None => self.paint(token, kind, text),
    };
    paint_leaf(&mut self.out, start, text, paint, around);
  }

  fn paint(&self, token: &SyntaxToken, kind: K, text: &str) -> Paint {
    let style = match kind {
      K::WHITESPACE => return Paint::Inherit,
      K::COMMENT | K::OUTER_DOC_COMMENT | K::INNER_DOC_COMMENT => return comment_paint(text),
      K::CHAR | K::BYTE => Role::String.style(),
      _ if self.errors > 0 => return if is_keyword(kind) { Paint::Leaf(Role::Keyword.style()) } else { Paint::Inherit },
      K::INT_NUMBER | K::FLOAT_NUMBER => Role::Number.style(),
      // an attribute is one colour, its literals painted over it
      _ if self.attributes > 0 => return Paint::Inherit,
      // RustRover's lexer colours a label (`'outer`) as a lifetime (`QUOTE_IDENTIFIER`)
      K::LIFETIME_IDENT => Role::Lifetime.style(),
      K::QUESTION => Role::QuestionOperator.style(),
      K::SELF_KW => Role::SelfValue.style(),
      // a metavariable of a macro (`$x`, `$crate`, `$type`) is the macro's
      _ if token.prev_sibling_or_token().is_some_and(|it| it.kind() == K::DOLLAR) => return Paint::Opaque,
      K::IDENT => return self.name(token, text),
      _ if is_keyword(kind) => Role::Keyword.style(),
      _ => return Paint::Inherit,
    };
    Paint::Leaf(style)
  }

  /// A string literal: the colour of a string, its escapes in theirs (`ast::IsString::escaped_char_ranges`, and the `\`
  /// that continues a string on the next line); a raw string has none
  fn string(&mut self, token: &SyntaxToken, start: usize, around: u32) {
    let base = Role::String.style();
    self.out.push(start, around);
    let text = self.text_of(token);
    let mut escapes: Vec<std::ops::Range<usize>> = Vec::new();
    let raw = text.trim_start_matches(['b', 'c']).starts_with('r');
    if !raw {
      let mut add = |range: rust_syntax::ra_ap_syntax::TextRange, _: Result<char, _>| {
        let range = span(range);
        if text.get(range.clone()).is_some_and(|it| it.starts_with('\\')) {
          escapes.push(range);
        }
      };
      match token.kind() {
        K::STRING => ast::String::cast(token.clone()).iter().for_each(|it| ast::IsString::escaped_char_ranges(it, &mut add)),
        K::BYTE_STRING => ast::ByteString::cast(token.clone()).iter().for_each(|it| ast::IsString::escaped_char_ranges(it, &mut add)),
        _ => ast::CString::cast(token.clone()).iter().for_each(|it| ast::IsString::escaped_char_ranges(it, &mut add)),
      }
      let bytes = text.as_bytes();
      let mut index = 0;
      while index + 1 < bytes.len() {
        if bytes[index] == b'\\' {
          if matches!(bytes[index + 1], b'\n' | b'\r') {
            escapes.push(index..index + 1);
          }
          index += 2;
        } else {
          index += 1;
        }
      }
      escapes.sort_by_key(|it| it.start);
    }
    for escape in escapes {
      self.out.push(start + escape.start, base);
      self.out.push(start + escape.end, Role::Escape.style());
    }
    self.out.push(start + text.len(), base);
  }

  /// The colour of an identifier by the node it names in
  fn name(&self, token: &SyntaxToken, text: &str) -> Paint {
    let Some(parent) = token.parent() else { return Paint::Opaque };
    match parent.kind() {
      K::NAME => match parent.parent().map(|it| it.kind()) {
        Some(K::TYPE_PARAM) => self.type_parameter(text),
        // the shorthand field of a struct pattern
        Some(K::IDENT_PAT)
          if parent
            .parent()
            .and_then(|it| it.parent())
            .is_some_and(|it| it.kind() == K::RECORD_PAT_FIELD && !it.children().any(|child| child.kind() == K::NAME_REF)) =>
        {
          Paint::Leaf(FIELD)
        }
        Some(K::IDENT_PAT) => self.name_style(text, true),
        // the name of a type, a module, a trait: no colour of its own
        _ => Paint::Opaque,
      },
      K::NAME_REF => {
        let Some(context) = parent.parent() else { return Paint::Opaque };
        match context.kind() {
          K::FIELD_EXPR | K::RECORD_EXPR_FIELD | K::RECORD_PAT_FIELD => Paint::Leaf(FIELD),
          K::PATH_SEGMENT => self.segment(&context, token, text),
          _ => self.type_parameter(text),
        }
      }
      K::TOKEN_TREE => {
        // a fragment of a macro's pattern (`$e:expr`) and a primitive type are keywords, as tree-sitter's grammar lexed them
        if is_fragment(token) || PRIMITIVES.contains(&text) {
          Paint::Leaf(Role::Keyword.style())
        } else {
          self.name_style(text, true)
        }
      }
      _ => Paint::Opaque,
    }
  }

  /// A segment of a path: a type in a type (a primitive one a keyword, a type parameter of a function in its colour),
  /// else a value
  fn segment(&self, segment: &SyntaxNode, token: &SyntaxToken, text: &str) -> Paint {
    let Some(path) = segment.parent() else { return Paint::Opaque };
    let qualified = path.children().any(|it| it.kind() == K::PATH);
    let outer = path.parent();
    if token.parent_ancestors().any(|it| it.kind() == K::USE) {
      return Paint::Opaque;
    }
    if outer.as_ref().is_some_and(|it| it.kind() == K::PATH_TYPE) {
      if !qualified && PRIMITIVES.contains(&text) {
        return Paint::Leaf(Role::Keyword.style());
      }
      return self.type_parameter(text);
    }
    if let Some(style) = self.scopes.local(text) {
      return if style == 0 { Paint::Opaque } else { Paint::Leaf(style) };
    }
    let is_qualifier = outer.as_ref().is_some_and(|it| it.kind() == K::PATH);
    // the last segment of `Kind::Small` in an expression or a pattern: an enum variant or an associated constant
    let segment_of_value = qualified
      && !is_qualifier
      && text.starts_with(|it: char| it.is_ascii_uppercase())
      && path
        .children()
        .find(|it| it.kind() == K::PATH)
        .and_then(|it| it.children().find(|child| child.kind() == K::PATH_SEGMENT))
        .and_then(|it| it.children().find(|child| child.kind() == K::NAME_REF))
        .and_then(|it| it.first_token())
        .is_some_and(|it| it.text().starts_with(|first: char| first.is_ascii_uppercase()))
      && outer
        .as_ref()
        .is_some_and(|it| (it.kind() == K::PATH_EXPR && !is_callee(it)) || matches!(it.kind(), K::PATH_PAT | K::TUPLE_STRUCT_PAT));
    if segment_of_value {
      return Paint::Leaf(CONSTANT);
    }
    unbound(text, !qualified && !is_qualifier)
  }

  /// An unqualified name: a `mut` binding is underlined, a name in the style of a constant is one
  fn name_style(&self, text: &str, alone: bool) -> Paint {
    match self.scopes.local(text) {
      Some(0) => Paint::Opaque,
      Some(style) => Paint::Leaf(style),
      None => unbound(text, alone),
    }
  }

  fn type_parameter(&self, text: &str) -> Paint {
    if self.scopes.is_type_parameter(text) {
      Paint::Leaf(Role::TypeParameter.style())
    } else {
      Paint::Opaque
    }
  }
}

/// A name no local binds: an `ALL_CAPS` one alone (not a segment of a path) is a constant
fn unbound(text: &str, alone: bool) -> Paint {
  if alone && is_constant_name(text) { Paint::Leaf(CONSTANT) } else { Paint::Opaque }
}

/// A keyword of the edition, the contextual ones the parser remaps included, but `macro_rules`, plain as tree-sitter's
/// grammar left `macro_rules!`
fn is_keyword(kind: K) -> bool {
  kind.is_keyword(rust_syntax::EDITION) && !matches!(kind, K::MACRO_RULES_KW)
}

/// The fragment of a metavariable in a macro's pattern: `expr` of `$e:expr`
fn is_fragment(token: &SyntaxToken) -> bool {
  let colon = token.prev_sibling_or_token().filter(|it| it.kind() == K::COLON);
  let name = colon
    .and_then(|it| it.prev_sibling_or_token())
    .filter(|it| it.kind() == K::IDENT || it.kind().is_keyword(rust_syntax::EDITION));
  name.and_then(|it| it.prev_sibling_or_token()).is_some_and(|it| it.kind() == K::DOLLAR)
}

/// The identifier of a declaration's name
fn name_of(node: &SyntaxNode) -> Option<SyntaxToken> {
  node.children().find(|it| it.kind() == K::NAME)?.first_token().filter(|it| it.kind() == K::IDENT)
}
