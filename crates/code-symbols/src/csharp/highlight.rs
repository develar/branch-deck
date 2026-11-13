//! The highlighting of a C# file from Roslyn's syntax tree, as Rider colours it without resolving names, in the roles
//! the other languages share: the tokens from the leaves of the tree, the roles of names from the nodes around them,
//! painted by `highlight::paint::paint_leaf`.
//!
//! - **Tokens** are the lexer's, as the parser left them: every reserved keyword and every contextual one the parser
//!   made a keyword (`partial`, `async`, `await`, `get`, `set`, `where`, `record`, `when`, `with`, …), the `var` of a
//!   declaration and the `nameof` of a call, which Roslyn keeps as identifiers; the numbers; a string, a verbatim and a
//!   raw string, a UTF-8 one, a character literal, an interpolated string with its escapes (`\n`, `\x41`, `A`,
//!   `""` in a verbatim string, `{{` and `}}` in an interpolated one) and the code of its holes.
//! - **Comments**: a comment; a documentation comment (`///`, `/** */`) with its XML tags and the values of their
//!   attributes (`cref`, `name`); a branch of `#if` the file's preprocessor configuration does not take, as a comment.
//! - **Directives**: `#` and the directive's keywords (`if`, `region`, `pragma warning disable`, …).
//! - **An attribute** (`[Obsolete]`, `[a::B.C]`) in its colour, its arguments as code.
//! - **Declarations**: the name of a method, a local function, a constructor or a destructor (italic when it is
//!   static), of a field, a property, an event (italic when static or const), of an enum member, of a type parameter
//!   (and its uses as a type).
//! - **Uses**: a named argument (`x:`), a label, a member through `this` or `base` or a value (`this.count`,
//!   `item.Count`), and an unqualified name that is a field, a property or an event of a type around it with no local
//!   or parameter of that name in between (`highlight::scopes`); a name in a type position (`Color Color`) is a type.
//!
//! One pass over the nodes in preorder (their ids), with a stack of the open nodes that colour or scope their subtree:
//! no recursion, whatever the nesting.

use crate::csharp::{is_block_scope, is_code, is_type_declaration};
use crate::highlight::lines::{Builder, LineRuns};
use crate::highlight::paint::{Open, Paint, close_before, node_count, paint_escapes, paint_leaf};
use crate::highlight::scopes::Scopes;
use crate::highlight::{COMMENT, FIELD, ITALIC, Role, STATIC_FIELD};
use csharp_syntax::syntax_getters as sg;
use csharp_syntax::{NodeId, NodeKind, TokenKind, Tree, facts, read};
use std::collections::HashMap;

/// A static method or local function: the colour of a declaration, italic, as the other languages' static methods
const STATIC_METHOD: u32 = Role::FunctionDeclaration.style() | ITALIC;

/// The highlighting of a C# text (its size within `highlight::MAX_BYTES`): any bytes parse, so it always has one
pub(crate) fn highlight(text: &str) -> LineRuns {
  let tree = code_tokens::csharp::parse(text.as_bytes());
  let mut out = LineRuns::builder(text);
  Painter {
    tree: &tree,
    member_styles: HashMap::new(),
    scopes: Scopes::default(),
  }
  .paint(&mut out);
  out.finish()
}

/// What a composite the walk enters is to it
enum Entered {
  /// Nothing to remember: it binds a name or is plain
  Passed,
  /// It opened a scope, which ends with it
  Scope,
  /// Its subtree has this style
  Styled(u32),
  /// A structured trivia whose leaves are no names of the code, in this style
  Trivia(Trivia, u32),
}

/// A structured trivia: its tokens are not read as the code's
#[derive(Clone, Copy, PartialEq, Eq)]
enum Trivia {
  /// A documentation comment: its text, its XML tags and their values
  Doc,
  /// A directive (`#if DEBUG`) or tokens the parser skipped: their keywords are keywords, their names plain
  Directive,
}

struct Painter<'t> {
  tree: &'t Tree,
  /// The style of each member of the types the walk entered, by the node of its declaration (a `VariableDeclarator` of
  /// a field, a property, an event, an enum member)
  member_styles: HashMap<NodeId, u32>,
  scopes: Scopes,
}

impl Painter<'_> {
  fn paint(&mut self, out: &mut Builder<'_>) {
    let tree = self.tree;
    let count = node_count(tree);
    // the open nodes, each with the structured trivia it is
    let mut open: Vec<Open<Option<Trivia>>> = Vec::new();
    let mut id = 0;
    while id < count {
      let node = NodeId(id);
      let around = close_before(&mut open, id, &mut self.scopes);
      let in_trivia = open.iter().rev().find_map(|it| it.extra);
      if tree.is_leaf(node) {
        if !tree.range(node).is_empty() {
          self.leaf(out, node, around, in_trivia);
        }
      } else {
        let (style, trivia) = match self.enter(node, in_trivia) {
          Entered::Passed => {
            id += 1;
            continue;
          }
          Entered::Scope => (None, None),
          Entered::Styled(style) => (Some(style), None),
          Entered::Trivia(trivia, style) => (Some(style), Some(trivia)),
        };
        open.push(Open {
          end: tree.subtree_end(node).0,
          node,
          style,
          extra: trivia,
        });
      }
      id += 1;
    }
  }

  /// Opens the scope of a composite and binds what it declares
  fn enter(&mut self, node: NodeId, in_trivia: Option<Trivia>) -> Entered {
    let tree = self.tree;
    let Some(kind) = tree.node_kind(node) else { return Entered::Passed };
    match in_trivia {
      Some(Trivia::Doc) => return doc_style(kind).map_or(Entered::Passed, Entered::Styled),
      Some(Trivia::Directive) => return Entered::Passed,
      None => {}
    }
    match kind {
      NodeKind::SingleLineDocumentationCommentTrivia | NodeKind::MultiLineDocumentationCommentTrivia => Entered::Trivia(Trivia::Doc, Role::DocComment.style()),
      // a directive, or tokens the parser skipped: what their leaves are, no names
      kind if facts::is_preprocessor_directive(kind.syntax_kind()) || kind == NodeKind::SkippedTokensTrivia => Entered::Trivia(Trivia::Directive, Role::Foreground.style()),
      NodeKind::Attribute => Entered::Styled(Role::Annotation.style()),
      // the arguments of an attribute are code, the code of a hole of an interpolated string too (its alignment
      // included); its format is the string's
      NodeKind::AttributeArgumentList | NodeKind::Interpolation => Entered::Styled(Role::Foreground.style()),
      NodeKind::InterpolatedStringExpression | NodeKind::InterpolationFormatClause => Entered::Styled(Role::String.style()),
      kind if is_type_declaration(kind) => {
        let members = self.members(node);
        self.scopes.push_class(node.0 as usize, members);
        Entered::Scope
      }
      kind if is_code(kind) || is_block_scope(kind) => {
        self.scopes.push_local(node.0 as usize);
        if is_value_accessor(kind) {
          self.scopes.bind("value");
        }
        // the variable of a `foreach` is seen in its body only
        if kind == NodeKind::ForEachStatement
          && let Some(name) = identifier_text(tree, node)
        {
          self.scopes.bind(name);
        }
        Entered::Scope
      }
      NodeKind::TypeParameter => {
        if let Some(name) = identifier_text(tree, node) {
          self.scopes.bind_type_parameter(name);
        }
        Entered::Passed
      }
      kind if binds_local(tree, node, kind) => {
        if let Some(name) = identifier_text(tree, node) {
          self.scopes.bind(name);
        }
        Entered::Passed
      }
      _ => Entered::Passed,
    }
  }

  fn leaf(&self, out: &mut Builder<'_>, leaf: NodeId, around: u32, in_trivia: Option<Trivia>) {
    let tree = self.tree;
    let start = tree.range(leaf).start;
    let text = text_of(tree, leaf);
    let Some(token) = tree.token_kind(leaf) else { return };
    let paint = match (token, in_trivia) {
      (TokenKind::SingleLineCommentTrivia | TokenKind::MultiLineCommentTrivia | TokenKind::DisabledTextTrivia, _) => Some(Paint::Leaf(COMMENT)),
      (TokenKind::DocumentationCommentExteriorTrivia, _) => Some(Paint::Leaf(Role::DocComment.style())),
      // the value of a tag's attribute (`name="x"`), its quotes too; any other text of the comment is in the style around
      (TokenKind::XmlTextLiteralToken | TokenKind::DoubleQuoteToken | TokenKind::SingleQuoteToken, Some(Trivia::Doc)) if is_attribute_value(tree, leaf) => {
        Some(Paint::Leaf(Role::DocTagValue.style()))
      }
      (_, Some(Trivia::Doc)) => Some(Paint::Inherit),
      // `#`, `if`, `region`, `pragma warning disable`, `nullable enable`; `#region Name`, a symbol, the text of `#error`
      // are plain
      (token, Some(Trivia::Directive)) => {
        let keyword = token == TokenKind::HashToken || facts::is_preprocessor_keyword(token.syntax_kind()) || facts::is_keyword_kind(token.syntax_kind());
        Some(if keyword { Paint::Leaf(Role::Keyword.style()) } else { Paint::Inherit })
      }
      (token, None) => self.code_paint(out, leaf, token, start, text, around),
    };
    if let Some(paint) = paint {
      paint_leaf(out, start, text, paint, around);
    }
  }

  /// The paint of a token of the code; `None` when it painted itself (a string with its escapes)
  fn code_paint(&self, out: &mut Builder<'_>, leaf: NodeId, token: TokenKind, start: usize, text: &str, around: u32) -> Option<Paint> {
    let tree = self.tree;
    Some(match token {
      TokenKind::StringLiteralToken | TokenKind::Utf8StringLiteralToken | TokenKind::CharacterLiteralToken => {
        let escapes = if text.starts_with('@') { Escapes::Verbatim } else { Escapes::Backslash };
        string(out, start, text, escapes, around);
        return None;
      }
      TokenKind::SingleLineRawStringLiteralToken
      | TokenKind::MultiLineRawStringLiteralToken
      | TokenKind::Utf8SingleLineRawStringLiteralToken
      | TokenKind::Utf8MultiLineRawStringLiteralToken
      | TokenKind::InterpolatedStringStartToken
      | TokenKind::InterpolatedVerbatimStringStartToken
      | TokenKind::InterpolatedSingleLineRawStringStartToken
      | TokenKind::InterpolatedMultiLineRawStringStartToken
      | TokenKind::InterpolatedStringEndToken
      | TokenKind::InterpolatedRawStringEndToken => Paint::Leaf(Role::String.style()),
      TokenKind::InterpolatedStringTextToken => {
        string(out, start, text, interpolated_escapes(tree, leaf), around);
        return None;
      }
      TokenKind::NumericLiteralToken => Paint::Leaf(Role::Number.style()),
      // `@count` is the name `count`
      TokenKind::IdentifierToken => self.name_style(leaf, tree.value_text(leaf).unwrap_or(text)).map_or(Paint::Inherit, Paint::Leaf),
      token if facts::is_keyword_kind(token.syntax_kind()) => Paint::Leaf(Role::Keyword.style()),
      _ => Paint::Inherit,
    })
  }

  /// The style of a name by the node it names or is a part of; `None` leaves it the style around it
  fn name_style(&self, leaf: NodeId, name: &str) -> Option<u32> {
    let tree = self.tree;
    let (parent, kind) = crate::psi_navigate::parent_kind(tree, leaf)?;
    match kind {
      NodeKind::MethodDeclaration | NodeKind::LocalFunctionStatement | NodeKind::ConstructorDeclaration | NodeKind::DestructorDeclaration => {
        Some(if has_modifier(tree, parent, TokenKind::StaticKeyword) {
          STATIC_METHOD
        } else {
          Role::FunctionDeclaration.style()
        })
      }
      NodeKind::VariableDeclarator | NodeKind::PropertyDeclaration | NodeKind::EventDeclaration | NodeKind::EnumMemberDeclaration | NodeKind::Parameter => {
        self.member_styles.get(&parent).copied()
      }
      NodeKind::TypeParameter => Some(Role::TypeParameter.style()),
      NodeKind::LabeledStatement => Some(Role::Label.style()),
      NodeKind::IdentifierName => self.use_style(parent, name),
      _ => None,
    }
  }

  /// The style of an `IdentifierName`: the keyword `var` or `nameof`, a named argument, a label, a type parameter as a
  /// type, a member after a value or of a type around it
  fn use_style(&self, name_node: NodeId, name: &str) -> Option<u32> {
    let tree = self.tree;
    let owner = tree.parent(name_node)?;
    let owner_kind = tree.node_kind(owner)?;
    if sg::is_in_type_slot(tree, name_node) {
      return if self.scopes.is_type_parameter(name) {
        Some(Role::TypeParameter.style())
      } else if name == "var" && is_declared_type(owner_kind) {
        Some(Role::Keyword.style())
      } else {
        None
      };
    }
    match owner_kind {
      // the dotted name of a namespace or a `using` and the qualifier of a qualified name: no member
      NodeKind::NamespaceDeclaration | NodeKind::FileScopedNamespaceDeclaration | NodeKind::UsingDirective | NodeKind::QualifiedName | NodeKind::AliasQualifiedName => None,
      NodeKind::NameColon => Some(Role::NamedArgument.style()),
      NodeKind::GotoStatement => Some(Role::Label.style()),
      NodeKind::InvocationExpression if name == "nameof" && is_callee(tree, owner, name_node) => Some(Role::Keyword.style()),
      NodeKind::InvocationExpression if is_callee(tree, owner, name_node) => None,
      NodeKind::SimpleMemberAccessExpression | NodeKind::PointerMemberAccessExpression if member_access_name(tree, owner) == Some(name_node) => {
        // `x.F(…)`: a call, which only resolving tells a method of; `T.Count`: a type's static member, a namespace or a
        // property of that name, which only resolving tells apart
        let callee = tree
          .parent(owner)
          .is_some_and(|call| tree.node_kind(call) == Some(NodeKind::InvocationExpression) && is_callee(tree, call, owner));
        let object = member_access_expression(tree, owner)?;
        (!callee && is_value(tree, object)).then_some(FIELD)
      }
      NodeKind::MemberBindingExpression => {
        // `x?.Count`: a member of a value, unless it is called
        let call = tree.parent(owner)?;
        let callee = tree.node_kind(call) == Some(NodeKind::InvocationExpression) && is_callee(tree, call, owner);
        (!callee).then_some(FIELD)
      }
      _ => self.scopes.member(name),
    }
  }

  /// The fields, properties, events and enum members of a type with their styles, the properties a record's
  /// parameters declare (`record Point(int X, int Y)`); keeps the style of each for its declaration
  fn members(&mut self, type_node: NodeId) -> Vec<(String, u32)> {
    let tree = self.tree;
    let mut names = Vec::new();
    let record = matches!(tree.node_kind(type_node), Some(NodeKind::RecordDeclaration | NodeKind::RecordStructDeclaration));
    for member in tree.children(type_node) {
      match tree.node_kind(member) {
        Some(NodeKind::ParameterList) if record => {
          for parameter in tree.children(member).filter(|&it| tree.node_kind(it) == Some(NodeKind::Parameter)) {
            self.member_styles.insert(parameter, FIELD);
            names.extend(identifier_text(tree, parameter).map(|name| (name.to_string(), FIELD)));
          }
        }
        Some(NodeKind::FieldDeclaration | NodeKind::EventFieldDeclaration) => {
          let shared = has_modifier(tree, member, TokenKind::StaticKeyword) || has_modifier(tree, member, TokenKind::ConstKeyword);
          let style = if shared { STATIC_FIELD } else { FIELD };
          let Some(declaration) = tree.children(member).find(|&it| tree.node_kind(it) == Some(NodeKind::VariableDeclaration)) else {
            continue;
          };
          for declarator in tree.children(declaration).filter(|&it| tree.node_kind(it) == Some(NodeKind::VariableDeclarator)) {
            self.member_styles.insert(declarator, style);
            names.extend(identifier_text(tree, declarator).map(|name| (name.to_string(), style)));
          }
        }
        Some(NodeKind::PropertyDeclaration | NodeKind::EventDeclaration) => {
          let style = if has_modifier(tree, member, TokenKind::StaticKeyword) { STATIC_FIELD } else { FIELD };
          self.member_styles.insert(member, style);
          names.extend(identifier_text(tree, member).map(|name| (name.to_string(), style)));
        }
        Some(NodeKind::EnumMemberDeclaration) => {
          self.member_styles.insert(member, STATIC_FIELD);
          names.extend(identifier_text(tree, member).map(|name| (name.to_string(), STATIC_FIELD)));
        }
        _ => {}
      }
    }
    names
  }
}

/// The style of a node inside a documentation comment: its tags (`<see`, `/>`) and their attributes' names in the colour
/// of a tag, the values of the attributes (a `cref`'s type, a `name`'s parameter, a quoted text) in the colour of a
/// tag's value; `None` for the text around them
const fn doc_style(kind: NodeKind) -> Option<u32> {
  match kind {
    NodeKind::XmlElementStartTag
    | NodeKind::XmlElementEndTag
    | NodeKind::XmlEmptyElement
    | NodeKind::XmlTextAttribute
    | NodeKind::XmlCrefAttribute
    | NodeKind::XmlNameAttribute => Some(Role::DocTag.style()),
    NodeKind::TypeCref
    | NodeKind::QualifiedCref
    | NodeKind::NameMemberCref
    | NodeKind::IndexerMemberCref
    | NodeKind::OperatorMemberCref
    | NodeKind::ConversionOperatorMemberCref
    | NodeKind::ExtensionMemberCref
    | NodeKind::IdentifierName => Some(Role::DocTagValue.style()),
    _ => None,
  }
}

/// How the escapes of a string literal read
#[derive(Clone, Copy, PartialEq, Eq)]
enum Escapes {
  /// A regular string or character: `\n`, `\x41`, `A`, `\U00000041`
  Backslash,
  /// A verbatim string (`@"…"`): `""`
  Verbatim,
  /// The text of a regular interpolated string: the backslash escapes and `{{`, `}}`
  InterpolatedBackslash,
  /// The text of a verbatim interpolated string: `""`, `{{`, `}}`
  InterpolatedVerbatim,
  /// The text of a raw interpolated string: none
  None,
}

/// The escapes of an `InterpolatedStringTextToken` (a child of an `InterpolatedStringText`) by the token its string
/// starts with (`$"`, `$@"`, `$"""`)
fn interpolated_escapes(tree: &Tree, leaf: NodeId) -> Escapes {
  let string = tree
    .parent(leaf)
    .and_then(|text| tree.parent(text))
    .filter(|&it| tree.node_kind(it) == Some(NodeKind::InterpolatedStringExpression));
  let start = string.and_then(|it| tree.children(it).find_map(|child| (!tree.is_trivia(child)).then(|| tree.token_kind(child)).flatten()));
  match start {
    Some(TokenKind::InterpolatedVerbatimStringStartToken) => Escapes::InterpolatedVerbatim,
    Some(TokenKind::InterpolatedSingleLineRawStringStartToken | TokenKind::InterpolatedMultiLineRawStringStartToken) => Escapes::None,
    _ => Escapes::InterpolatedBackslash,
  }
}

/// Paints a string literal: the literal in the colour of a string, its escapes in theirs
fn string(out: &mut Builder<'_>, start: usize, text: &str, escapes: Escapes, around: u32) {
  let bytes = text.as_bytes();
  // a verbatim literal's escapes are between its `@"` and its closing quote (before a `u8` suffix): the quotes that
  // open and close it are no doubled quote
  let scan = if escapes == Escapes::Verbatim {
    2.min(bytes.len())..bytes.iter().rposition(|&it| it == b'"').filter(|&it| it >= 2).unwrap_or(bytes.len())
  } else {
    0..bytes.len()
  };
  paint_escapes(out, start, text, scan, around, |bytes| escape_length(bytes, escapes));
}

/// The length of the escape at the start of `bytes`, 0 where none starts, as the C# lexer reads it: `\x` with one to four
/// hex digits, `\u` with four, `\U` with eight, else a backslash and the character after it; `""` in a verbatim string,
/// `{{` and `}}` in the text of an interpolated one
fn escape_length(bytes: &[u8], escapes: Escapes) -> usize {
  let backslash = matches!(escapes, Escapes::Backslash | Escapes::InterpolatedBackslash);
  let doubled_quote = matches!(escapes, Escapes::Verbatim | Escapes::InterpolatedVerbatim);
  let braces = matches!(escapes, Escapes::InterpolatedBackslash | Escapes::InterpolatedVerbatim);
  match bytes {
    [b'\\', kind, rest @ ..] if backslash => {
      let hex = |most: usize| rest.iter().take(most).take_while(|it| it.is_ascii_hexdigit()).count();
      match kind {
        b'x' | b'u' => 2 + hex(4),
        b'U' => 2 + hex(8),
        _ => 2,
      }
    }
    [b'"', b'"', ..] if doubled_quote => 2,
    [b'{', b'{', ..] | [b'}', b'}', ..] if braces => 2,
    _ => 0,
  }
}

/// An accessor whose body sees the implicit parameter `value`
const fn is_value_accessor(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::SetAccessorDeclaration | NodeKind::InitAccessorDeclaration | NodeKind::AddAccessorDeclaration | NodeKind::RemoveAccessorDeclaration
  )
}

/// A node that binds a local of the scope it is in by its identifier: a parameter, a local's declarator, the variable of
/// a `foreach` or a `catch`, a designation (`out var x`, `is string s`), a range variable of a query
fn binds_local(tree: &Tree, node: NodeId, kind: NodeKind) -> bool {
  match kind {
    // a parameter of a type (a primary constructor's, a record's) is seen in the type's body: a record's is a member
    // (`members`), a class's a parameter its members capture, which no local scope holds
    NodeKind::Parameter => !tree
      .parent(node)
      .and_then(|list| tree.parent(list))
      .and_then(|owner| tree.node_kind(owner))
      .is_some_and(is_type_declaration),
    NodeKind::FunctionPointerParameter
    | NodeKind::ForEachStatement
    | NodeKind::CatchDeclaration
    | NodeKind::SingleVariableDesignation
    | NodeKind::FromClause
    | NodeKind::LetClause
    | NodeKind::JoinClause
    | NodeKind::JoinIntoClause
    | NodeKind::QueryContinuation => true,
    // a field's declarator is a member (`members`), any other a local
    NodeKind::VariableDeclarator => !tree
      .parent(node)
      .and_then(|declaration| tree.parent(declaration))
      .and_then(|owner| tree.node_kind(owner))
      .is_some_and(|owner| matches!(owner, NodeKind::FieldDeclaration | NodeKind::EventFieldDeclaration)),
    _ => false,
  }
}

/// A node whose type is a declared one, which `var` may stand for: a local, a `foreach` variable, an `out var`
const fn is_declared_type(kind: NodeKind) -> bool {
  matches!(kind, NodeKind::VariableDeclaration | NodeKind::ForEachStatement | NodeKind::DeclarationExpression)
}

/// Whether `expression` is what `call` calls: its first child, the expression before the argument list
fn is_callee(tree: &Tree, call: NodeId, expression: NodeId) -> bool {
  tree.children(call).find(|&it| !tree.is_leaf(it)) == Some(expression)
}

/// The expression of a member access (`a` of `a.b`): its first composite child
fn member_access_expression(tree: &Tree, access: NodeId) -> Option<NodeId> {
  tree.children(access).find(|&it| !tree.is_leaf(it))
}

/// The name of a member access (`b` of `a.b`): its last composite child
fn member_access_name(tree: &Tree, access: NodeId) -> Option<NodeId> {
  tree.children(access).filter(|&it| !tree.is_leaf(it)).last()
}

/// Whether an expression reads as a value, not as a type or a namespace: `this`, `base`, a literal, a call, an element
/// access, a name not in the style of a type (`item`, not `Item`), a parenthesized one, a member of one of these
fn is_value(tree: &Tree, expression: NodeId) -> bool {
  match tree.node_kind(expression) {
    Some(NodeKind::IdentifierName) => !text_of(tree, expression).trim_start().trim_start_matches('@').starts_with(char::is_uppercase),
    Some(NodeKind::SimpleMemberAccessExpression) => member_access_expression(tree, expression).is_some_and(|it| is_value(tree, it)),
    Some(NodeKind::GenericName | NodeKind::QualifiedName | NodeKind::AliasQualifiedName | NodeKind::PredefinedType) | None => false,
    Some(_) => true,
  }
}

/// Whether a declaration has the modifier as a keyword: one of its child tokens, before its type and name
fn has_modifier(tree: &Tree, declaration: NodeId, modifier: TokenKind) -> bool {
  tree
    .children(declaration)
    .take_while(|&it| tree.is_leaf(it) || tree.node_kind(it) == Some(NodeKind::AttributeList))
    .any(|it| tree.token_kind(it) == Some(modifier))
}

/// Whether a leaf of a documentation comment is the value of a tag's attribute: a child of `name="x"`
fn is_attribute_value(tree: &Tree, leaf: NodeId) -> bool {
  tree
    .parent(leaf)
    .and_then(|it| tree.node_kind(it))
    .is_some_and(|kind| matches!(kind, NodeKind::XmlTextAttribute | NodeKind::XmlCrefAttribute | NodeKind::XmlNameAttribute))
}

/// The text of a node, empty when it is not UTF-8
fn text_of(tree: &Tree, node: NodeId) -> &str {
  tree.utf8_text(node).unwrap_or_default()
}

/// The name a declaration binds: the `ValueText` of its child `IdentifierToken` (`count` of `@count`)
fn identifier_text(tree: &Tree, declaration: NodeId) -> Option<&str> {
  tree
    .children(declaration)
    .find(|&it| tree.token_kind(it) == Some(TokenKind::IdentifierToken))
    .and_then(|it| read::word(tree, it))
}

#[cfg(test)]
#[path = "highlight_test.rs"]
mod tests;
