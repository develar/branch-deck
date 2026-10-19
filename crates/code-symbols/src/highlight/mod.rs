//! The syntax highlighting of one file, as the IntelliJ editor colours it without resolving names
//! (docs/design/diff-view.md, "Syntax highlighting"): the role of each piece of text, from a tree-sitter syntax tree.
//!
//! - **One walk, painted spans**: a preorder walk asks the rules of the grammar for the style of each node; a node with a
//!   style paints its text, an inner node paints over it (an escape in a string, the code of a template), and a node
//!   without one takes the style of the node around it. So the spans come out in the order of the text, with no sort.
//! - **The format of the view**: the lines are split as `buildLineTable` of the view splits them (`\n`, `\r\n`, a lone
//!   `\r`), each line has the runs of its styles, a pair (UTF-16 offset in the line, style) where the style changes, none
//!   while it is the default foreground, as the Shiki worker sends them (`src/lib/diff/highlight/protocol.ts`).
//! - **A style** is a `Role` in its low 6 bits and the font style above them: italic 1, bold 2, underline 4.
//! - **Bounded**: over `MAX_BYTES` or after the parse budget there is no highlighting; the view falls back to Shiki for
//!   the file. A parse error colours what its leaves are (a keyword, a literal), the rest stays plain.
//!
//! A pure function of the text, like the summary (`extract.rs`), but with its own parse: the summary is cached by blob,
//! and TypeScript files are read by the TypeScript grammar here, not by TSX.

mod lex;
pub mod lines;
mod paint;
mod scopes;

#[cfg(feature = "java")]
mod java;
#[cfg(feature = "kotlin")]
mod kotlin;
#[cfg(feature = "rust")]
mod rust_lang;
#[cfg(feature = "typescript")]
mod typescript;

use lines::LineRuns;

/// The version of the highlighting rules; bump it on any rule change, so that cached highlightings are made again
pub const RULES_VERSION: u32 = 3;
/// Files above this size are not highlighted: `MAX_BLOB_BYTES` of the texts the view reads
pub const MAX_BYTES: usize = 8 << 20;
/// The progress callbacks of a parse per KiB of text; the densest files of the IntelliJ monorepo take 19
const PARSE_BUDGET_PER_KIB: usize = 50;
/// The budget of a small file: error recovery may take many operations on few bytes
const MIN_PARSE_BUDGET: usize = 50_000;

/// The bits of the role in a style
pub const ROLE_BITS: u32 = 6;
pub const ITALIC: u32 = 1 << ROLE_BITS;
pub const BOLD: u32 = 2 << ROLE_BITS;
/// A mutable binding (a Kotlin `var`, a Rust `mut`): underlined, as IntelliJ's mutable variables
pub const UNDERLINE: u32 = 4 << ROLE_BITS;

/// A comment; the view gives it the font style of the scheme (italic in the light one)
pub(crate) const COMMENT: u32 = Role::Comment.style();
/// An instance field or property
pub(crate) const FIELD: u32 = Role::Field.style();
/// A static field, a constant, an enum entry: italic
pub(crate) const STATIC_FIELD: u32 = Role::Field.style() | ITALIC;

/// The roles of the highlighting, by their ids in the view: `SYNTAX_ROLES` of `src/lib/diff/islands-themes.ts` is this
/// order after the default foreground (`roles_match_the_view` checks it)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Role {
  Foreground = 0,
  Keyword,
  String,
  Escape,
  Number,
  /// A line or block comment: italic in the light scheme only, as IntelliJ's
  Comment,
  FunctionDeclaration,
  Field,
  Annotation,
  Tag,
  Attribute,
  Label,
  DocComment,
  DocTag,
  DocTagValue,
  NamedArgument,
  Macro,
  Lifetime,
  TypeParameter,
  /// Rust's `self`, coloured apart from the keywords in `RustRover`
  SelfValue,
  EnumVariant,
  /// Rust's `?`
  QuestionOperator,
}

impl Role {
  /// The names of the view, in the order of the ids after the default foreground
  pub const NAMES: [&'static str; 21] = [
    "keyword",
    "string",
    "escape",
    "number",
    "comment",
    "functionDeclaration",
    "field",
    "annotation",
    "tag",
    "attribute",
    "label",
    "docComment",
    "docTag",
    "docTagValue",
    "namedArgument",
    "macro",
    "lifetime",
    "typeParameter",
    "selfValue",
    "enumVariant",
    "questionOperator",
  ];

  pub const fn style(self) -> u32 {
    self as u32
  }
}

/// The grammar a file is highlighted with
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Grammar {
  Kotlin,
  Java,
  Rust,
  TypeScript,
  /// TSX: `.tsx` and JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`), JSX included
  Tsx,
}

impl Grammar {
  /// A stable number for compact keys
  pub const fn id(self) -> u8 {
    match self {
      Self::Kotlin => 0,
      Self::Java => 1,
      Self::Rust => 2,
      Self::TypeScript => 3,
      Self::Tsx => 4,
    }
  }

  fn language(self) -> Option<tree_sitter::Language> {
    match self {
      #[cfg(feature = "kotlin")]
      Self::Kotlin => Some(tree_sitter_kotlin_sg::LANGUAGE.into()),
      #[cfg(feature = "java")]
      Self::Java => Some(tree_sitter_java::LANGUAGE.into()),
      #[cfg(feature = "rust")]
      Self::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
      #[cfg(feature = "typescript")]
      Self::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
      #[cfg(feature = "typescript")]
      Self::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
      #[cfg_attr(
        all(feature = "kotlin", feature = "java", feature = "rust", feature = "typescript"),
        expect(unreachable_patterns, reason = "every arm above is behind a feature")
      )]
      _ => None,
    }
  }
}

/// The grammar of a file by its extension, if this build can highlight it
pub fn grammar_of(path: &str) -> Option<Grammar> {
  let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
  let (_, extension) = file_name.rsplit_once('.')?;
  let grammar = match extension {
    "kt" | "kts" => Grammar::Kotlin,
    "java" => Grammar::Java,
    "rs" => Grammar::Rust,
    "ts" | "mts" | "cts" => Grammar::TypeScript,
    "tsx" | "js" | "jsx" | "mjs" | "cjs" => Grammar::Tsx,
    _ => return None,
  };
  grammar.language().map(|_| grammar)
}

/// A stable hash of the rules and the grammars of this build: a cached highlighting is valid only for the same value
pub fn highlighter_version() -> u32 {
  crate::extract::extractor_version() ^ RULES_VERSION.wrapping_mul(0x9e37_79b9)
}

/// A name in the style of a constant or an enum entry: upper case, digits and underscores
pub(crate) fn is_constant_name(name: &str) -> bool {
  name.len() > 1 && name.bytes().next().is_some_and(|it| it.is_ascii_uppercase()) && name.bytes().all(|it| it.is_ascii_uppercase() || it.is_ascii_digit() || it == b'_')
}

/// A name in the style of a type: `Foo`, not `FOO` or `foo`
pub(crate) fn is_type_name(name: &str) -> bool {
  name.bytes().next().is_some_and(|it| it.is_ascii_uppercase()) && name.bytes().any(|it| it.is_ascii_lowercase())
}

/// The first named child of a kind
pub(crate) fn named_child<'tree>(node: tree_sitter::Node<'tree>, kind: &str) -> Option<tree_sitter::Node<'tree>> {
  let mut cursor = node.walk();
  node.named_children(&mut cursor).find(|it| it.kind() == kind)
}

/// The tree of a text by its grammar, within the parse budget of its size
pub(crate) fn parse_text(grammar: Grammar, text: &str) -> Option<tree_sitter::Tree> {
  let language = grammar.language()?;
  let budget = (text.len() / 1024).saturating_mul(PARSE_BUDGET_PER_KIB).max(MIN_PARSE_BUDGET);
  crate::extract::parse_language(&language, text.as_bytes(), u32::try_from(budget).unwrap_or(u32::MAX))
}

/// The highlighting of a text; `None` when it is over `MAX_BYTES` or its parse ran out of budget
pub fn highlight(grammar: Grammar, text: &str) -> Option<LineRuns> {
  if text.len() > MAX_BYTES {
    return None;
  }
  let tree = parse_text(grammar, text)?;
  let mut runs = LineRuns::builder(text);
  match grammar {
    #[cfg(feature = "kotlin")]
    Grammar::Kotlin => paint::paint(&tree, text, &mut kotlin::Kotlin, &mut runs),
    #[cfg(feature = "java")]
    Grammar::Java => paint::paint(&tree, text, &mut java::Java, &mut runs),
    #[cfg(feature = "rust")]
    Grammar::Rust => paint::paint(&tree, text, &mut rust_lang::Rust, &mut runs),
    #[cfg(feature = "typescript")]
    Grammar::TypeScript | Grammar::Tsx => paint::paint(&tree, text, &mut typescript::TypeScript, &mut runs),
    #[cfg_attr(
      all(feature = "kotlin", feature = "java", feature = "rust", feature = "typescript"),
      expect(unreachable_patterns, reason = "every arm above is behind a feature")
    )]
    _ => return None,
  }
  Some(runs.finish())
}
