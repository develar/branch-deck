//! The tokens of a TypeScript, TSX or JavaScript file from oxc's tokens and tree (`tokens`): the tokens are the
//! parse's (a `/` a division or a regular expression, a template's text with its `${` and `}`, the text of a JSX
//! element), the comments the program's, and the shape of a token the nodes of the tree whose spans open before it and
//! close after it.
//!
//! - **The nodes** are every node the visitor enters (`Visit::enter_node`), by their spans: sorted by their start, the
//!   longer first, a node inside the one before it is its child. So a line break that ends a statement
//!   (`return⏎foo()`: a `ReturnStatement` without an argument and an `ExpressionStatement`) is in the shapes, as is
//!   what a word is where the lexer gives it one kind (`type`, `async`, `get`: a name under an identifier node).
//! - **A name** is a token an identifier node starts at, or an identifier token no node took.
//! - **The text of a JSX element** compares as JSX reads it (`jsx_text`), and is no token when it reads as nothing: the
//!   whitespace with a line break between two elements. Its node is not in the shapes, the token stands for it.
//! - **The errors** oxc reported are a list beside the tree (`ParserReturn::diagnostics`), and some change no node: a
//!   line break before the `=>` of an arrow function or after a `throw` is a syntax error of the same tree. Each is an
//!   event before the token its first label stands at or before, by its message, so such a text has other tokens.
//! - **Two blanks are errors oxc does not report** (`is_irregular`): U+0085 and U+200B between two tokens, its
//!   "irregular whitespace". Only the scanner of TypeScript reads them as whitespace; ECMAScript does not, so an engine
//!   that runs the file, esbuild and Babel (the readers of a TypeScript file beside `tsc`) call them a syntax error.
//!   In every dialect such a blank is an error at its place (`Builder::irregular`): a text with it is another
//!   program than the text without it to at least one of its readers.
//! - **No tokens** for a file oxc read only after a recovery (`Parsed::recovered`): the regions it blanked are not in
//!   the tokens; nor for one it could not read at all or that nests deeper than `js_syntax` parses.

use crate::tokens::build::{Builder, Kinds, hash_bytes, mix};
use crate::tokens::{TokenClass, Tokens};
use js_syntax::Dialect;
use js_syntax::oxc_ast::{AstKind, AstType};
use js_syntax::oxc_ast_visit::Visit;
use js_syntax::oxc_parser::Kind;
use js_syntax::oxc_span::GetSpan;
use std::collections::HashSet;

/// The tokens of `text` read as `dialect`; `None` when the tree does not hold every token (the module's "No tokens")
pub(crate) fn tokens(dialect: Dialect, text: &str) -> Option<Tokens> {
  super::with_program(dialect, text.as_bytes(), |parsed| {
    if parsed.recovered {
      return None;
    }
    let program = parsed.program()?;
    let mut nodes = Nodes::default();
    nodes.visit_program(program);
    // by their start, the longer first; the order of the visit among the nodes of one span, a parent before its child
    nodes.spans.sort_by(|left, right| left.start.cmp(&right.start).then(right.end.cmp(&left.end)));

    let mut out = Builder::new(text).irregular(is_irregular);
    let mut token_kinds = Kinds::default();
    let mut open = Open::default();
    let mut spans = nodes.spans.iter().peekable();
    let hashbang = program.hashbang.as_ref().map(|it| it.span);
    let mut comments = hashbang.into_iter().chain(program.comments.iter().map(|it| it.span)).peekable();
    // where each error stands, by its first label (one without a label before the first token), and its message
    let mut errors: Vec<(usize, u64)> = parsed
      .diagnostics
      .iter()
      .map(|error| {
        let at = error.labels.iter().map(|label| label.offset() as usize).min().unwrap_or(0);
        (at, hash_bytes(error.message.as_bytes()))
      })
      .collect();
    errors.sort_by_key(|(at, _)| *at);
    let mut errors = errors.into_iter().peekable();
    for token in parsed.tokens.iter().filter(|it| it.kind() != Kind::Eof) {
      let (start, end, kind) = (token.start(), token.end(), token.kind());
      while let Some(comment) = comments.next_if(|it| it.start < start) {
        out.comment(comment.start as usize..comment.end as usize);
      }
      let range = start as usize..end as usize;
      let jsx = (kind == Kind::JSXText).then(|| jsx_text(text.as_bytes().get(range.clone()).unwrap_or_default()));
      if jsx == Some(None) {
        continue;
      }
      while let Some(span) = spans.next_if(|it| it.start <= start) {
        open.open(&mut out, span);
      }
      open.close_until(&mut out, start);
      while let Some((_, message)) = errors.next_if(|(at, _)| *at < end as usize) {
        out.error(message);
      }
      let hash = token_kinds.of(kind as usize, || format!("{kind:?}"));
      let class = class(kind, nodes.names.contains(&start));
      match jsx.flatten() {
        Some(read) => out.code_as(class, hash, range, read),
        None => out.code(class, hash, range),
      }
    }
    for comment in comments {
      out.comment(comment.start as usize..comment.end as usize);
    }
    for (_, message) in errors {
      out.error(message);
    }
    for span in spans {
      open.open(&mut out, span);
    }
    open.close_until(&mut out, u32::MAX);
    out.finish()
  })
  .flatten()
}

/// Whether oxc skips `character` between two tokens though ECMAScript calls it no whitespace (`WhiteSpace` is the
/// tab, U+000B, U+000C, the byte order mark and the class `Zs`): U+0085 (`Cc`) and U+200B (`Cf` since Unicode 4.0.1)
const fn is_irregular(character: char) -> bool {
  matches!(character, '\u{85}' | '\u{200b}')
}

/// The open nodes of the walk, the end and the kind of each, innermost last
#[derive(Default)]
struct Open {
  nodes: Vec<(u32, u64)>,
  kinds: Kinds,
}

impl Open {
  /// Opens the node at `span`, once the nodes it is not inside are closed
  fn open(&mut self, out: &mut Builder<'_>, span: &NodeSpan) {
    while let Some((_, closed)) = self.nodes.pop_if(|it| span.end > it.0) {
      out.close(closed);
    }
    let kind = self.kinds.of(span.kind as usize, || format!("{:?}", span.kind));
    out.open(kind);
    self.nodes.push((span.end, kind));
  }

  /// Closes the nodes that end at `offset` or before it
  fn close_until(&mut self, out: &mut Builder<'_>, offset: u32) {
    while let Some((_, closed)) = self.nodes.pop_if(|it| it.0 <= offset) {
      out.close(closed);
    }
  }
}

/// A node of the tree by its span
struct NodeSpan {
  start: u32,
  end: u32,
  kind: AstType,
}

/// The nodes of a program in the order of the visit, and the starts of its names
#[derive(Default)]
struct Nodes {
  spans: Vec<NodeSpan>,
  names: HashSet<u32>,
}

impl<'a> Visit<'a> for Nodes {
  fn enter_node(&mut self, kind: AstKind<'a>) {
    let (span, kind) = (kind.span(), kind.ty());
    match kind {
      // the token of its text stands for it (`jsx_text`)
      AstType::JSXText => return,
      AstType::IdentifierName | AstType::IdentifierReference | AstType::BindingIdentifier | AstType::LabelIdentifier | AstType::PrivateIdentifier | AstType::JSXIdentifier => {
        self.names.insert(span.start);
      }
      _ => {}
    }
    self.spans.push(NodeSpan {
      start: span.start,
      end: span.end,
      kind,
    });
  }
}

const fn class(kind: Kind, named: bool) -> TokenClass {
  match kind {
    Kind::Str | Kind::RegExp | Kind::NoSubstitutionTemplate | Kind::TemplateHead | Kind::TemplateMiddle | Kind::TemplateTail | Kind::JSXText => TokenClass::Literal,
    _ if named => TokenClass::Name,
    Kind::Ident | Kind::PrivateIdentifier => TokenClass::Name,
    Kind::True | Kind::False | Kind::Null | Kind::This | Kind::Super => TokenClass::Keyword,
    kind if kind.is_any_keyword() => TokenClass::Keyword,
    _ => TokenClass::Other,
  }
}

/// The text of a JSX element as JSX reads it, hashed; `None` when it reads as nothing. Every line but the first loses
/// the spaces and tabs it starts with, every line but the last those it ends with, a line left empty is dropped, and
/// the rest is joined by one space: what Babel's `cleanJSXElementLiteralChild` and TypeScript's
/// `fixupWhitespaceAndDecodeEntities` both do. The spaces inside a line stay, JSX keeps them. Where the two differ, the
/// lines stay apart, so two texts equal here are equal to both: TypeScript also trims the other whitespace of Unicode
/// from a line and ends a line at U+2028 and U+2029, Babel does not
pub(crate) fn jsx_text(text: &[u8]) -> Option<u64> {
  let blank = |byte: &u8| matches!(byte, b' ' | b'\t');
  // a `\r\n` gives an empty line between its two bytes, which is dropped as any other
  let lines: Vec<&[u8]> = text.split(|byte| matches!(byte, b'\n' | b'\r')).collect();
  let last = lines.len() - 1;
  // whether both readers trim the same of each line and split the text at the same places
  let separators = |text: &str| text.contains(['\u{2028}', '\u{2029}']);
  let mut alike = std::str::from_utf8(text).is_ok_and(|text| !separators(text));
  let mut read: Vec<&[u8]> = Vec::with_capacity(lines.len());
  for (index, mut line) in lines.into_iter().enumerate() {
    let edge = |line: &[u8], end: bool| {
      let line = std::str::from_utf8(line).unwrap_or("\u{a0}");
      if end { line.chars().next_back() } else { line.chars().next() }.is_some_and(char::is_whitespace)
    };
    if index != 0 {
      line = &line[line.iter().take_while(|byte| blank(byte)).count()..];
      alike &= !edge(line, false);
    }
    if index != last {
      line = &line[..line.len() - line.iter().rev().take_while(|byte| blank(byte)).count()];
      alike &= !edge(line, true);
    }
    if !line.is_empty() {
      read.push(line);
    }
  }
  if read.is_empty() {
    None
  } else if alike {
    Some(hash_bytes(&read.join(&b' ')))
  } else {
    Some(read.into_iter().fold(1, |hash, line| mix(hash, hash_bytes(line))))
  }
}
